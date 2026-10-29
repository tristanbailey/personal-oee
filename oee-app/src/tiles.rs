//! The four metric tiles on the Today screen (D-20).

use eframe::egui::{self, RichText, Stroke};

use crate::theme;
use oee_core::{DayInputs, DayResult, DayStatus, Metrics, WeekReport};

/// FR-040: percentages to one decimal place.
pub fn pct(v: f64) -> String {
    format!("{:.1}%", v * 100.0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    pub title: &'static str,
    pub value: String,
    pub caption: String,
    /// The OEE tile is drawn dark.
    pub inverted: bool,
}

/// Availability, Efficiency, Quality and OEE. `ended` switches the OEE caption
/// from "So far today" to the formula.
pub fn tiles(inputs: &DayInputs, m: &Metrics, ended: bool) -> [Tile; 4] {
    build(inputs, m, if ended { "A × E × Q" } else { "So far today" })
}

/// D-35: the same four tiles for a pooled week (FR-041, FR-053).
pub fn week_tiles(report: &WeekReport) -> [Tile; 4] {
    build(&report.totals, &report.metrics, "Week, pooled")
}

fn build(inputs: &DayInputs, m: &Metrics, oee_caption: &str) -> [Tile; 4] {
    let available = inputs.day_minutes.saturating_sub(inputs.planned_minutes);
    [
        Tile {
            title: "Availability",
            value: pct(m.availability),
            caption: format!("{} min lost of {available}", inputs.downtime_minutes()),
            inverted: false,
        },
        Tile {
            title: "Efficiency",
            value: pct(m.efficiency),
            caption: format!("{} of {} points", inputs.work_points, inputs.target_points),
            inverted: false,
        },
        Tile {
            title: "Quality",
            value: pct(m.quality),
            caption: format!("{} rework", inputs.rework_points),
            inverted: false,
        },
        Tile {
            title: "OEE",
            value: pct(m.oee),
            caption: oee_caption.into(),
            inverted: true,
        },
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerTone {
    /// Over capacity: worth a look.
    Warn,
    /// 0% day.
    Bad,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Banner {
    pub text: &'static str,
    pub tone: BannerTone,
}

/// D-21: FR-048 over-capacity flag, and FR-045a 0% day once the day has ended.
pub fn banners(result: &DayResult, ended: bool) -> Vec<Banner> {
    let mut out = Vec::new();
    if result.metrics.over_capacity {
        out.push(Banner {
            text: "Over capacity: more Work points than the target. Check the blocks are right.",
            tone: BannerTone::Warn,
        });
    }
    if ended && result.status == DayStatus::ZeroWork {
        out.push(Banner {
            text: "0% day: time was logged but there are no Work points. It still counts in the week.",
            tone: BannerTone::Bad,
        });
    }
    out
}

pub fn show_banner(ui: &mut egui::Ui, banner: &Banner) {
    let p = theme::of(ui);
    let (fill, edge) = match banner.tone {
        BannerTone::Warn => (p.change_soft, p.change),
        BannerTone::Bad => (p.down_soft, p.down),
    };
    egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, edge))
        .rounding(6.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(banner.text).color(p.ink).strong());
        });
}

pub fn show(ui: &mut egui::Ui, tiles: &[Tile; 4]) {
    ui.columns(4, |cols| {
        for (ui, tile) in cols.iter_mut().zip(tiles) {
            let p = theme::of(ui);
            // The OEE tile is inverted: ink for the fill, the page colour for text.
            let (fill, ink) = if tile.inverted { (p.ink, p.bg) } else { (p.surface, p.ink) };
            egui::Frame::none()
                .fill(fill)
                .stroke(Stroke::new(1.0_f32, p.line))
                .rounding(8.0)
                .inner_margin(10.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(tile.title).color(ink).small());
                    ui.label(RichText::new(&tile.value).color(ink).size(30.0).strong());
                    ui.label(RichText::new(&tile.caption).color(ink).small());
                });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::{hm, DayRecord, Settings};

    #[test]
    fn captions_match_the_prd_examples() {
        // 09:00–17:40 with lunch (60 min planned), a 25-min Idle stop and a rework block.
        let lunch = oee_core::PlannedWindow::new(
            hm(12, 0),
            hm(13, 0),
            oee_core::PlannedKind::Break,
            "Lunch",
        );
        let mut r = DayRecord::new(hm(9, 0), hm(17, 40), vec![lunch]);
        r.add_downtime(hm(10, 0), hm(10, 25));
        r.categorise(hm(10, 0), oee_core::LostCategory::Idle).unwrap();
        r.set_rework(hm(9, 0), true).unwrap();
        let i = r.inputs(&Settings::default());
        let m = Metrics::from_inputs(&i);
        let t = tiles(&i, &m, false);
        assert_eq!(t[0].caption, "20 min lost of 460");
        assert_eq!(t[1].caption, "22 of 20 points");
        assert_eq!(t[2].caption, "1 rework");
        assert_eq!(t[3].caption, "So far today");
        assert!(t[3].inverted && !t[0].inverted);
        assert_eq!(tiles(&i, &m, true)[3].caption, "A × E × Q");
    }

    fn result(work: u32, target: u32) -> DayResult {
        let i = DayInputs {
            day_minutes: 520,
            work_points: work,
            target_points: target,
            ..Default::default()
        };
        oee_core::day_result(Some(&i))
    }

    #[test]
    fn over_capacity_banner_shows_above_100_percent() {
        assert!(banners(&result(20, 20), false).is_empty());
        let b = banners(&result(21, 20), false);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].tone, BannerTone::Warn);
    }

    #[test]
    fn zero_day_banner_only_once_the_day_has_ended() {
        assert!(banners(&result(0, 20), false).is_empty());
        let b = banners(&result(0, 20), true);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].tone, BannerTone::Bad);
        assert!(banners(&result(5, 20), true).is_empty());
    }

    #[test]
    fn values_have_one_decimal_place() {
        assert_eq!(pct(0.584), "58.4%");
        assert_eq!(pct(1.0), "100.0%");
        assert_eq!(pct(0.0), "0.0%");
    }
}

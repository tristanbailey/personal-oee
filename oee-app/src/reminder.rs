//! Categorisation banners and the Finish day button (D-28, FR-026, FR-027).

use eframe::egui::{self, Color32, RichText};
use oee_core::{Mode, Tracker};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoriseBanner {
    pub text: String,
    /// Shown at the end of the day: finalises the day when pressed.
    pub finish_button: bool,
}

pub fn blocks_need(n: usize) -> String {
    if n == 1 {
        "1 lost block needs a category.".into()
    } else {
        format!("{n} lost blocks need a category.")
    }
}

/// What to say about uncategorised lost blocks, if anything.
pub fn categorise_banner(t: &Tracker) -> Option<CategoriseBanner> {
    let n = t.record.uncategorised().len();
    match t.mode() {
        Mode::Planned { .. } if n > 0 => Some(CategoriseBanner {
            text: format!("{} Click the striped blocks below.", blocks_need(n)),
            finish_button: false,
        }),
        Mode::Ended if !t.record.finalised => Some(CategoriseBanner {
            text: if n > 0 {
                format!(
                    "{} Anything left becomes System Down when you finish.",
                    blocks_need(n)
                )
            } else {
                "Day ended. Everything is categorised.".into()
            },
            finish_button: true,
        }),
        _ => None,
    }
}

/// Draw the banner. Returns true if Finish day was pressed.
pub fn show(ui: &mut egui::Ui, banner: &CategoriseBanner) -> bool {
    let mut finish = false;
    egui::Frame::none()
        .fill(Color32::from_rgb(0x3e, 0x2e, 0x10))
        .rounding(6.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&banner.text).color(Color32::from_rgb(0xf0, 0xa6, 0x2a)));
                if banner.finish_button {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        finish = ui.button("Finish day").clicked();
                    });
                }
            });
        });
    finish
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::{hm, PlannedKind, PlannedWindow, Settings};

    fn lunch() -> PlannedWindow {
        PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch")
    }

    fn tracker() -> Tracker {
        Tracker::start_day(hm(9, 0), vec![lunch()], Settings::default())
    }

    #[test]
    fn no_banner_while_working_or_in_downtime() {
        let mut t = tracker();
        assert_eq!(categorise_banner(&t), None);
        t.toggle_downtime(hm(10, 0));
        t.toggle_downtime(hm(10, 25));
        assert_eq!(categorise_banner(&t), None);
    }

    #[test]
    fn planned_downtime_prompts_when_blocks_need_a_category() {
        let mut t = tracker();
        t.toggle_downtime(hm(10, 0));
        t.toggle_downtime(hm(10, 25));
        t.tick(hm(12, 5));
        let b = categorise_banner(&t).unwrap();
        assert_eq!(
            b.text,
            "1 lost block needs a category. Click the striped blocks below."
        );
        assert!(!b.finish_button);
    }

    #[test]
    fn planned_downtime_with_nothing_to_categorise_shows_nothing() {
        let mut t = tracker();
        t.tick(hm(12, 5));
        assert_eq!(categorise_banner(&t), None);
    }

    #[test]
    fn end_of_day_warns_then_offers_finish() {
        let mut t = tracker();
        t.toggle_downtime(hm(10, 0));
        t.toggle_downtime(hm(11, 5));
        t.tick(hm(17, 40));
        let b = categorise_banner(&t).unwrap();
        assert_eq!(
            b.text,
            "3 lost blocks need a category. Anything left becomes System Down when you finish."
        );
        assert!(b.finish_button);
    }

    #[test]
    fn end_of_day_when_everything_is_categorised() {
        let mut t = tracker();
        t.tick(hm(17, 40));
        let b = categorise_banner(&t).unwrap();
        assert_eq!(b.text, "Day ended. Everything is categorised.");
        assert!(b.finish_button);
    }

    #[test]
    fn finishing_hides_the_banner() {
        let mut t = tracker();
        t.toggle_downtime(hm(10, 0));
        t.toggle_downtime(hm(10, 25));
        t.tick(hm(17, 40));
        t.finalise();
        assert_eq!(categorise_banner(&t), None);
    }
}

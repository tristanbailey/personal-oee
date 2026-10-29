//! The andon panel: stack light, status word and toggle (D-15 to D-18).

use eframe::egui::{self, Color32, Sense, Stroke, Vec2};
use oee_core::{Mode, PlannedKind, Tracker, BLOCK_MINUTES};

fn hhmm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// D-16: the big status word.
pub fn status_word(tracker: Option<&Tracker>) -> &'static str {
    match tracker.map(|t| t.mode()) {
        None => "NOT STARTED",
        Some(Mode::Work) => "WORK",
        Some(Mode::Downtime { .. }) => "DOWNTIME",
        Some(Mode::Planned { kind, .. }) => match kind {
            PlannedKind::Break => "BREAK",
            PlannedKind::Meeting => "MEETING",
        },
        Some(Mode::Ended) => "DAY ENDED",
    }
}

/// D-16: the line under the status word, which changes with the mode.
pub fn sub_line(tracker: Option<&Tracker>, now: u32) -> String {
    let Some(t) = tracker else {
        return "Set a start time to begin the day".into();
    };
    match t.mode() {
        Mode::Work => {
            let into = now.saturating_sub(t.record.start);
            let total = t.record.slot_count().max(1);
            let block = (into / BLOCK_MINUTES + 1).min(total);
            let left = BLOCK_MINUTES - into % BLOCK_MINUTES;
            format!("Block {block} of {total} · {left} min left")
        }
        Mode::Downtime { since } => {
            let mins = now.saturating_sub(since);
            let mut line = format!("Since {} · {mins} min", hhmm(since));
            // FR-014: a stop of 5 minutes or less is ignored.
            if mins <= 5 {
                line.push_str(" · under 5 min is ignored");
            }
            line
        }
        Mode::Planned { kind, until } => {
            let name = t
                .record
                .planned
                .iter()
                .find(|w| w.end == until && w.start <= now && !w.label.is_empty())
                .map(|w| w.label.as_str())
                .unwrap_or(match kind {
                    PlannedKind::Break => "Break",
                    PlannedKind::Meeting => "Meeting",
                });
            format!("{name} · until {}, then back to Work", hhmm(until))
        }
        Mode::Ended => format!("Finished at {}", hhmm(t.record.end)),
    }
}

/// Which lamps are lit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lamps {
    pub red: bool,
    pub amber: bool,
    pub green: bool,
}

impl Lamps {
    const OFF: Lamps = Lamps {
        red: false,
        amber: false,
        green: false,
    };
}

/// D-15: red = Downtime, amber = Planned, green = Work. All off when the day
/// hasn't started (`None`) or has ended.
pub fn lamps(mode: Option<Mode>) -> Lamps {
    match mode {
        Some(Mode::Work) => Lamps {
            green: true,
            ..Lamps::OFF
        },
        Some(Mode::Downtime { .. }) => Lamps {
            red: true,
            ..Lamps::OFF
        },
        Some(Mode::Planned { .. }) => Lamps {
            amber: true,
            ..Lamps::OFF
        },
        Some(Mode::Ended) | None => Lamps::OFF,
    }
}

/// Draw the three lamps stacked vertically: red, amber, green.
pub fn stack_light(ui: &mut egui::Ui, lit: Lamps) {
    let radius = 14.0;
    let gap = 8.0;
    let size = Vec2::new(radius * 2.0 + 16.0, (radius * 2.0 + gap) * 3.0 + 8.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 10.0, Color32::from_gray(30));
    let lamps = [
        (lit.red, Color32::from_rgb(0xef, 0x5b, 0x50)),
        (lit.amber, Color32::from_rgb(0xf0, 0xa6, 0x2a)),
        (lit.green, Color32::from_rgb(0x46, 0xb8, 0x74)),
    ];
    for (i, (on, colour)) in lamps.into_iter().enumerate() {
        let centre = egui::pos2(
            rect.center().x,
            rect.top() + 4.0 + radius + i as f32 * (radius * 2.0 + gap),
        );
        let fill = if on { colour } else { colour.gamma_multiply(0.18) };
        p.circle(centre, radius, fill, Stroke::new(1.0, Color32::from_gray(60)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::{hm, PlannedWindow, Settings};

    fn tracker() -> Tracker {
        Tracker::start_day(hm(9, 0), vec![], Settings::default())
    }

    #[test]
    fn work_line_counts_blocks_and_minutes_left() {
        let t = tracker();
        // Block 7 starts at 11:00. At 11:08 there are 12 minutes left.
        assert_eq!(
            sub_line(Some(&t), hm(11, 8)),
            "Block 7 of 26 · 12 min left"
        );
        assert_eq!(sub_line(Some(&t), hm(9, 0)), "Block 1 of 26 · 20 min left");
        assert_eq!(status_word(Some(&t)), "WORK");
    }

    #[test]
    fn downtime_line_shows_start_and_minutes() {
        let mut t = tracker();
        t.toggle_downtime(hm(10, 5));
        assert_eq!(status_word(Some(&t)), "DOWNTIME");
        assert_eq!(sub_line(Some(&t), hm(10, 12)), "Since 10:05 · 7 min");
        assert_eq!(
            sub_line(Some(&t), hm(10, 8)),
            "Since 10:05 · 3 min · under 5 min is ignored"
        );
    }

    #[test]
    fn planned_line_names_the_window() {
        let lunch = PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch");
        let mut t = Tracker::start_day(hm(9, 0), vec![lunch], Settings::default());
        t.tick(hm(12, 10));
        assert_eq!(status_word(Some(&t)), "BREAK");
        assert_eq!(
            sub_line(Some(&t), hm(12, 10)),
            "Lunch · until 13:00, then back to Work"
        );
        let standup = PlannedWindow::new(hm(14, 0), hm(14, 20), PlannedKind::Meeting, "");
        let mut t = Tracker::start_day(hm(9, 0), vec![standup], Settings::default());
        t.tick(hm(14, 5));
        assert_eq!(status_word(Some(&t)), "MEETING");
        assert!(sub_line(Some(&t), hm(14, 5)).starts_with("Meeting · until 14:20"));
    }

    #[test]
    fn not_started_and_ended_words() {
        assert_eq!(status_word(None), "NOT STARTED");
        let mut t = tracker();
        t.tick(hm(18, 0));
        assert_eq!(status_word(Some(&t)), "DAY ENDED");
        assert_eq!(sub_line(Some(&t), hm(18, 0)), "Finished at 17:40");
    }

    fn count(l: Lamps) -> usize {
        [l.red, l.amber, l.green].into_iter().filter(|x| *x).count()
    }

    #[test]
    fn each_running_mode_lights_exactly_one_lamp() {
        let work = lamps(Some(Mode::Work));
        let down = lamps(Some(Mode::Downtime { since: 600 }));
        let plan = lamps(Some(Mode::Planned {
            kind: PlannedKind::Break,
            until: 780,
        }));
        assert!(work.green && count(work) == 1);
        assert!(down.red && count(down) == 1);
        assert!(plan.amber && count(plan) == 1);
    }

    #[test]
    fn not_started_and_ended_light_nothing() {
        assert_eq!(count(lamps(None)), 0);
        assert_eq!(count(lamps(Some(Mode::Ended))), 0);
    }
}

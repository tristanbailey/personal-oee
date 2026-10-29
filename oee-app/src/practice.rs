//! Practice mode (D-45): a fake clock for trying things out without waiting.
//! Debug builds only; release builds never show the buttons, so the offset
//! stays 0 and the real clock is used.

/// Minutes in a day. The fake clock stops at 23:59 so times stay valid.
const DAY: u32 = 24 * 60;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Practice {
    /// Minutes added to the real time.
    pub offset: u32,
}

// Only the debug-build buttons use most of this.
#[cfg_attr(not(debug_assertions), allow(dead_code))]
impl Practice {
    pub const STEPS: [u32; 4] = [1, 5, 20, 60];

    pub fn add(&mut self, minutes: u32) {
        self.offset = self.offset.saturating_add(minutes);
    }

    pub fn reset(&mut self) {
        self.offset = 0;
    }

    pub fn is_active(&self) -> bool {
        self.offset > 0
    }

    /// The practice time for a real time of `real` minutes since midnight.
    pub fn apply(&self, real: u32) -> u32 {
        real.saturating_add(self.offset).min(DAY - 1)
    }
}

/// The practice buttons. Returns nothing: it changes `practice` directly.
#[cfg(debug_assertions)]
pub fn show(ui: &mut eframe::egui::Ui, practice: &mut Practice, now: u32) {
    use eframe::egui::{Color32, RichText};
    ui.separator();
    ui.label(RichText::new("Practice").color(Color32::from_rgb(0xf0, 0xa6, 0x2a)).small());
    for step in Practice::STEPS {
        if ui.small_button(format!("+{step}")).clicked() {
            practice.add(step);
        }
    }
    if practice.is_active() {
        if ui.small_button("Reset").clicked() {
            practice.reset();
        }
        ui.label(
            RichText::new(format!("+{} min → {:02}:{:02}", practice.offset, now / 60, now % 60))
                .small()
                .color(Color32::from_rgb(0xf0, 0xa6, 0x2a)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_clock_is_used_when_inactive() {
        let p = Practice::default();
        assert!(!p.is_active());
        assert_eq!(p.apply(9 * 60 + 5), 9 * 60 + 5);
    }

    #[test]
    fn buttons_add_minutes_and_reset_clears_them() {
        let mut p = Practice::default();
        for step in Practice::STEPS {
            p.add(step);
        }
        assert_eq!(p.offset, 1 + 5 + 20 + 60);
        assert!(p.is_active());
        assert_eq!(p.apply(9 * 60), 9 * 60 + 86);
        p.reset();
        assert_eq!(p, Practice::default());
    }

    #[test]
    fn the_fake_clock_stops_at_the_end_of_the_day() {
        let mut p = Practice::default();
        p.add(60);
        assert_eq!(p.apply(23 * 60 + 30), 24 * 60 - 1);
        p.add(u32::MAX);
        assert_eq!(p.apply(10), 24 * 60 - 1);
    }

    #[test]
    fn the_steps_match_the_backlog() {
        assert_eq!(Practice::STEPS, [1, 5, 20, 60]);
    }
}

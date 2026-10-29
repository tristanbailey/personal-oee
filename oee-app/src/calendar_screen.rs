//! The Calendar tab: the repeating weekly Planned Downtime (D-31).

use eframe::egui::{self, RichText};
use oee_core::timeline::snap;
use oee_core::{EntryId, PlannedKind, PlannedWindow, Weekday, WeeklyCalendar};

use crate::start::{parse_hhmm, window_line};

pub const NOTE: &str = "Repeats every week. Applies to days you start after a change.";
pub const EMPTY_DAY: &str = "Nothing planned";

pub const WEEK: [(Weekday, &str); 7] = [
    (Weekday::Mon, "Monday"),
    (Weekday::Tue, "Tuesday"),
    (Weekday::Wed, "Wednesday"),
    (Weekday::Thu, "Thursday"),
    (Weekday::Fri, "Friday"),
    (Weekday::Sat, "Saturday"),
    (Weekday::Sun, "Sunday"),
];

pub const NAME_LIMIT: usize = 40;

/// Which day(s) a new entry applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayChoice {
    Day(Weekday),
    /// Adds the same entry to Monday to Friday.
    MonToFri,
}

impl DayChoice {
    pub const ALL: [(DayChoice, &'static str); 8] = [
        (DayChoice::MonToFri, "Mon–Fri"),
        (DayChoice::Day(Weekday::Mon), "Monday"),
        (DayChoice::Day(Weekday::Tue), "Tuesday"),
        (DayChoice::Day(Weekday::Wed), "Wednesday"),
        (DayChoice::Day(Weekday::Thu), "Thursday"),
        (DayChoice::Day(Weekday::Fri), "Friday"),
        (DayChoice::Day(Weekday::Sat), "Saturday"),
        (DayChoice::Day(Weekday::Sun), "Sunday"),
    ];

    fn label(self) -> &'static str {
        Self::ALL.iter().find(|(c, _)| *c == self).map_or("", |(_, l)| l)
    }

    fn days(self) -> Vec<Weekday> {
        match self {
            DayChoice::Day(d) => vec![d],
            DayChoice::MonToFri => vec![
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
            ],
        }
    }
}

/// The add-entry form (D-33).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddForm {
    pub day: DayChoice,
    pub from: String,
    pub to: String,
    pub kind: PlannedKind,
    pub name: String,
    pub error: Option<String>,
}

impl Default for AddForm {
    fn default() -> Self {
        Self {
            day: DayChoice::MonToFri,
            from: "12:00".into(),
            to: "13:00".into(),
            kind: PlannedKind::Break,
            name: String::new(),
            error: None,
        }
    }
}

impl AddForm {
    /// Add the entry (one per day chosen). Returns how many were added. On any
    /// problem nothing is added.
    pub fn add_to(&self, calendar: &mut WeeklyCalendar) -> Result<usize, String> {
        let time = "Enter times as HH:MM, for example 12:00.";
        let start = parse_hhmm(&self.from).ok_or(time)?;
        let end = parse_hhmm(&self.to).ok_or(time)?;
        if end <= start {
            return Err("The end time must be after the start time.".into());
        }
        let name = self.name.trim();
        if name.chars().count() > NAME_LIMIT {
            return Err(format!("Names can be at most {NAME_LIMIT} characters."));
        }
        let days = self.day.days();
        for day in &days {
            calendar.add(*day, PlannedWindow::new(start, end, self.kind, name));
        }
        Ok(days.len())
    }
}

fn kind_label(kind: PlannedKind) -> &'static str {
    match kind {
        PlannedKind::Break => "Break",
        PlannedKind::Meeting => "Meeting",
    }
}

fn show_form(ui: &mut egui::Ui, calendar: &mut WeeklyCalendar, form: &mut AddForm) {
    ui.label(RichText::new("Add a break or meeting").strong());
    ui.horizontal_wrapped(|ui| {
        ui.label("Day");
        egui::ComboBox::from_id_salt("add_day")
            .selected_text(form.day.label())
            .show_ui(ui, |ui| {
                for (choice, label) in DayChoice::ALL {
                    ui.selectable_value(&mut form.day, choice, label);
                }
            });
        ui.label("From");
        ui.add(egui::TextEdit::singleline(&mut form.from).desired_width(50.0));
        ui.label("To");
        ui.add(egui::TextEdit::singleline(&mut form.to).desired_width(50.0));
        ui.label("Type");
        egui::ComboBox::from_id_salt("add_kind")
            .selected_text(kind_label(form.kind))
            .show_ui(ui, |ui| {
                for kind in [PlannedKind::Meeting, PlannedKind::Break] {
                    ui.selectable_value(&mut form.kind, kind, kind_label(kind));
                }
            });
        ui.label("Name");
        ui.add(
            egui::TextEdit::singleline(&mut form.name)
                .desired_width(140.0)
                .hint_text("optional")
                .char_limit(NAME_LIMIT),
        );
        if ui.button("Add").clicked() {
            match form.add_to(calendar) {
                Ok(_) => {
                    form.error = None;
                    form.name.clear();
                }
                Err(e) => form.error = Some(e),
            }
        }
    });
    if let Some(e) = &form.error {
        crate::theme::error(ui, e);
    }
}

/// One calendar entry as shown on its card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayLine {
    pub id: EntryId,
    pub text: String,
    /// FR-038: how an off-grid entry counts on a day starting at `day_start`.
    pub counts_as: Option<String>,
}

fn hhmm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// "Counts as 09:20–10:00 (40 min) on a 09:00 start", or `None` if the window
/// is already on the 20-minute grid.
pub fn counts_as(window: &PlannedWindow, day_start: u32) -> Option<String> {
    let snapped = snap(window.clone(), day_start);
    (snapped.start != window.start || snapped.end != window.end).then(|| {
        format!(
            "Counts as {}–{} ({} min) on a {} start",
            hhmm(snapped.start),
            hhmm(snapped.end),
            snapped.end - snapped.start,
            hhmm(day_start)
        )
    })
}

/// The entries on `day`, in time order.
pub fn day_lines(calendar: &WeeklyCalendar, day: Weekday, day_start: u32) -> Vec<DayLine> {
    calendar
        .entries_for(day)
        .into_iter()
        .map(|e| DayLine {
            id: e.id,
            text: window_line(&e.window),
            counts_as: counts_as(&e.window, day_start),
        })
        .collect()
}

/// Draw the screen. Add and Remove change `calendar` directly.
pub fn show(ui: &mut egui::Ui, calendar: &mut WeeklyCalendar, form: &mut AddForm, day_start: u32) {
    let mut remove = None;
    ui.heading("Calendar");
    ui.label(NOTE);
    ui.add_space(6.0);
    show_form(ui, calendar, form);
    ui.add_space(8.0);
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (day, name) in WEEK {
            egui::Frame::none()
                .fill(ui.visuals().faint_bg_color)
                .rounding(8.0)
                .inner_margin(10.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(name).strong());
                    let lines = day_lines(&*calendar, day, day_start);
                    if lines.is_empty() {
                        ui.label(RichText::new(EMPTY_DAY).weak());
                    }
                    for line in lines {
                        let id = line.id;
                        ui.horizontal(|ui| {
                            ui.monospace(line.text);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("Remove").clicked() {
                                        remove = Some(id);
                                    }
                                },
                            );
                        });
                        if let Some(note) = line.counts_as {
                            ui.label(RichText::new(note).small().weak());
                        }
                    }
                });
            ui.add_space(4.0);
        }
    });
    if let Some(id) = remove {
        calendar.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::{hm, PlannedKind, PlannedWindow};

    #[test]
    fn the_week_runs_monday_to_sunday() {
        let days: Vec<_> = WEEK.iter().map(|(d, _)| *d).collect();
        let mut sorted = days.clone();
        sorted.sort();
        assert_eq!(days, sorted);
        assert_eq!(WEEK[0].1, "Monday");
        assert_eq!(WEEK[6].1, "Sunday");
    }

    const NINE: u32 = 9 * 60;

    #[test]
    fn entries_read_like_the_prd_example_and_are_in_time_order() {
        let mut c = WeeklyCalendar::default();
        let lunch = c.add(
            Weekday::Mon,
            PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch"),
        );
        let standup = c.add(
            Weekday::Mon,
            PlannedWindow::new(hm(9, 40), hm(10, 0), PlannedKind::Meeting, "Standup"),
        );
        let lines = day_lines(&c, Weekday::Mon, NINE);
        let got: Vec<_> = lines.iter().map(|l| (l.id, l.text.as_str())).collect();
        assert_eq!(
            got,
            vec![
                (standup, "09:40–10:00 · Meeting · Standup"),
                (lunch, "12:00–13:00 · Break · Lunch"),
            ]
        );
    }

    #[test]
    fn off_grid_entries_say_how_they_count() {
        let meeting = PlannedWindow::new(hm(9, 30), hm(10, 0), PlannedKind::Meeting, "Sync");
        assert_eq!(
            counts_as(&meeting, NINE).as_deref(),
            Some("Counts as 09:20–10:00 (40 min) on a 09:00 start")
        );
        let lunch = PlannedWindow::new(hm(12, 10), hm(12, 50), PlannedKind::Break, "Lunch");
        assert_eq!(
            counts_as(&lunch, NINE).as_deref(),
            Some("Counts as 12:00–13:00 (60 min) on a 09:00 start")
        );
    }

    #[test]
    fn on_grid_entries_say_nothing() {
        let lunch = PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch");
        assert_eq!(counts_as(&lunch, NINE), None);
        // The same window is off-grid on an 08:50 start.
        assert!(counts_as(&lunch, hm(8, 50)).is_some());
    }

    #[test]
    fn empty_days_have_no_lines() {
        let c = WeeklyCalendar::default();
        for (day, _) in WEEK {
            assert!(day_lines(&c, day, NINE).is_empty());
        }
    }

    fn form(day: DayChoice, from: &str, to: &str) -> AddForm {
        AddForm {
            day,
            from: from.into(),
            to: to.into(),
            ..Default::default()
        }
    }

    #[test]
    fn adding_one_day() {
        let mut c = WeeklyCalendar::default();
        let mut f = form(DayChoice::Day(Weekday::Wed), "12:00", "13:00");
        f.name = "  Lunch ".into();
        assert_eq!(f.add_to(&mut c), Ok(1));
        assert_eq!(
            c.windows_for(Weekday::Wed),
            vec![PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch")]
        );
        assert!(c.windows_for(Weekday::Thu).is_empty());
    }

    #[test]
    fn mon_to_fri_adds_five_entries() {
        let mut c = WeeklyCalendar::default();
        let mut f = form(DayChoice::MonToFri, "09:40", "10:00");
        f.kind = PlannedKind::Meeting;
        assert_eq!(f.add_to(&mut c), Ok(5));
        for day in [Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri] {
            assert_eq!(c.windows_for(day).len(), 1, "{day:?}");
        }
        assert!(c.windows_for(Weekday::Sat).is_empty());
        assert!(c.windows_for(Weekday::Sun).is_empty());
    }

    #[test]
    fn an_end_not_after_the_start_adds_nothing() {
        let mut c = WeeklyCalendar::default();
        for (from, to) in [("13:00", "12:00"), ("12:00", "12:00")] {
            assert_eq!(
                form(DayChoice::MonToFri, from, to).add_to(&mut c),
                Err("The end time must be after the start time.".into())
            );
        }
        assert_eq!(c, WeeklyCalendar::default());
    }

    #[test]
    fn bad_times_and_long_names_add_nothing() {
        let mut c = WeeklyCalendar::default();
        assert!(form(DayChoice::MonToFri, "noon", "13:00").add_to(&mut c).is_err());
        assert!(form(DayChoice::MonToFri, "12:00", "25:00").add_to(&mut c).is_err());
        let mut f = form(DayChoice::MonToFri, "12:00", "13:00");
        f.name = "x".repeat(NAME_LIMIT + 1);
        assert!(f.add_to(&mut c).unwrap_err().contains("40"));
        f.name = "x".repeat(NAME_LIMIT);
        assert_eq!(f.add_to(&mut c), Ok(5));
    }

    #[test]
    fn the_header_note_matches_the_backlog() {
        assert_eq!(
            NOTE,
            "Repeats every week. Applies to days you start after a change."
        );
    }
}

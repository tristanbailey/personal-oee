//! The menu-bar item (D-41): a coloured dot for the mode, the minutes that
//! matter, and a small menu.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use eframe::egui;
use oee_core::{Mode, Tracker, BLOCK_MINUTES};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lamp {
    Red,
    Amber,
    Green,
    Off,
}

impl Lamp {
    fn rgb(self) -> [u8; 3] {
        match self {
            Lamp::Red => [0xef, 0x5b, 0x50],
            Lamp::Amber => [0xf0, 0xa6, 0x2a],
            Lamp::Green => [0x46, 0xb8, 0x74],
            Lamp::Off => [0x96, 0xa1, 0xa2],
        }
    }
}

/// What the menu-bar item shows. Compared each frame so the OS is only
/// touched when something changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayState {
    pub lamp: Lamp,
    /// Next to the dot, e.g. "12m". Empty when there's nothing to count.
    pub title: String,
    pub toggle_label: &'static str,
    pub toggle_enabled: bool,
}

/// Work: minutes left in the block. Downtime: minutes so far. Planned: minutes
/// until Work resumes.
pub fn tray_state(tracker: Option<&Tracker>, now: u32) -> TrayState {
    let idle = |lamp| TrayState {
        lamp,
        title: String::new(),
        toggle_label: "Start downtime",
        toggle_enabled: false,
    };
    let Some(t) = tracker else {
        return idle(Lamp::Off);
    };
    match t.mode() {
        Mode::Work => {
            let into = now.saturating_sub(t.record.start);
            TrayState {
                lamp: Lamp::Green,
                title: format!("{}m", BLOCK_MINUTES - into % BLOCK_MINUTES),
                toggle_label: "Start downtime",
                toggle_enabled: true,
            }
        }
        Mode::Downtime { since } => TrayState {
            lamp: Lamp::Red,
            title: format!("{}m", now.saturating_sub(since)),
            toggle_label: "Stop downtime",
            toggle_enabled: true,
        },
        Mode::Planned { until, .. } => TrayState {
            lamp: Lamp::Amber,
            title: format!("{}m", until.saturating_sub(now)),
            ..idle(Lamp::Amber)
        },
        Mode::Ended => idle(Lamp::Off),
    }
}

const SIZE: u32 = 22;

/// A filled circle on a transparent square, as RGBA.
pub fn dot_rgba(lamp: Lamp) -> Vec<u8> {
    let [r, g, b] = lamp.rgb();
    let centre = (SIZE as f32 - 1.0) / 2.0;
    let radius = 7.5;
    let mut out = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let d = ((x as f32 - centre).powi(2) + (y as f32 - centre).powi(2)).sqrt();
            // One pixel of soft edge.
            let coverage = (radius + 0.5 - d).clamp(0.0, 1.0);
            out.extend_from_slice(&[r, g, b, (coverage * 255.0) as u8]);
        }
    }
    out
}

fn icon(lamp: Lamp) -> Result<Icon, String> {
    Icon::from_rgba(dot_rgba(lamp), SIZE, SIZE).map_err(|e| e.to_string())
}

/// What the user picked from the menu since the last look.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Actions {
    pub toggle: bool,
    pub open: bool,
    pub quit: bool,
}

#[derive(Default)]
struct Flags {
    toggle: AtomicBool,
    open: AtomicBool,
    quit: AtomicBool,
}

pub struct Tray {
    icon: TrayIcon,
    toggle: MenuItem,
    flags: Arc<Flags>,
    shown: Option<TrayState>,
}

impl Tray {
    /// Must be called on the main thread, once the event loop is running.
    pub fn new(ctx: egui::Context) -> Result<Tray, String> {
        let toggle = MenuItem::new("Start downtime", false, None);
        let open = MenuItem::new("Open window", true, None);
        let quit = MenuItem::new("Quit Personal OEE", true, None);
        let menu = Menu::new();
        menu.append_items(&[&toggle, &open, &PredefinedMenuItem::separator(), &quit])
            .map_err(|e| e.to_string())?;

        let flags = Arc::new(Flags::default());
        let (f, toggle_id, open_id, quit_id) = (
            flags.clone(),
            toggle.id().clone(),
            open.id().clone(),
            quit.id().clone(),
        );
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let flag = if e.id == toggle_id {
                &f.toggle
            } else if e.id == open_id {
                &f.open
            } else if e.id == quit_id {
                &f.quit
            } else {
                return;
            };
            flag.store(true, Ordering::SeqCst);
            ctx.request_repaint();
        }));

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon(Lamp::Off)?)
            .with_tooltip("Personal OEE")
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Tray {
            icon: tray,
            toggle,
            flags,
            shown: None,
        })
    }

    /// Menu picks since the last call.
    pub fn take_actions(&self) -> Actions {
        Actions {
            toggle: self.flags.toggle.swap(false, Ordering::SeqCst),
            open: self.flags.open.swap(false, Ordering::SeqCst),
            quit: self.flags.quit.swap(false, Ordering::SeqCst),
        }
    }

    /// Bring the menu-bar item up to date. Does nothing if it's unchanged.
    pub fn update(&mut self, state: TrayState) {
        if self.shown.as_ref() == Some(&state) {
            return;
        }
        if self.shown.as_ref().map(|s| s.lamp) != Some(state.lamp) {
            if let Ok(i) = icon(state.lamp) {
                let _ = self.icon.set_icon(Some(i));
            }
        }
        self.icon.set_title(Some(if state.title.is_empty() {
            String::new()
        } else {
            format!(" {}", state.title)
        }));
        self.toggle.set_text(state.toggle_label);
        self.toggle.set_enabled(state.toggle_enabled);
        self.shown = Some(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oee_core::{hm, PlannedKind, PlannedWindow, Settings};

    fn tracker() -> Tracker {
        Tracker::start_day(hm(9, 0), vec![], Settings::default())
    }

    #[test]
    fn not_started_shows_a_grey_dot_and_a_disabled_toggle() {
        let s = tray_state(None, hm(9, 0));
        assert_eq!((s.lamp, s.title.as_str(), s.toggle_enabled), (Lamp::Off, "", false));
    }

    #[test]
    fn work_shows_green_and_the_minutes_left_in_the_block() {
        let s = tray_state(Some(&tracker()), hm(11, 8));
        assert_eq!(s.lamp, Lamp::Green);
        assert_eq!(s.title, "12m");
        assert_eq!((s.toggle_label, s.toggle_enabled), ("Start downtime", true));
        assert_eq!(tray_state(Some(&tracker()), hm(9, 0)).title, "20m");
    }

    #[test]
    fn downtime_shows_red_and_the_minutes_so_far() {
        let mut t = tracker();
        t.toggle_downtime(hm(10, 5));
        let s = tray_state(Some(&t), hm(10, 12));
        assert_eq!((s.lamp, s.title.as_str()), (Lamp::Red, "7m"));
        assert_eq!((s.toggle_label, s.toggle_enabled), ("Stop downtime", true));
    }

    #[test]
    fn planned_shows_amber_and_the_time_until_work_resumes() {
        let lunch = PlannedWindow::new(hm(12, 0), hm(13, 0), PlannedKind::Break, "Lunch");
        let mut t = Tracker::start_day(hm(9, 0), vec![lunch], Settings::default());
        t.tick(hm(12, 15));
        let s = tray_state(Some(&t), hm(12, 15));
        assert_eq!((s.lamp, s.title.as_str(), s.toggle_enabled), (Lamp::Amber, "45m", false));
    }

    #[test]
    fn a_finished_day_is_grey_with_no_count() {
        let mut t = tracker();
        t.tick(hm(18, 0));
        let s = tray_state(Some(&t), hm(18, 0));
        assert_eq!((s.lamp, s.title.as_str()), (Lamp::Off, ""));
    }

    #[test]
    fn the_dot_is_a_coloured_circle_on_a_transparent_square() {
        let px = dot_rgba(Lamp::Red);
        assert_eq!(px.len(), (SIZE * SIZE * 4) as usize);
        let at = |x: u32, y: u32| &px[((y * SIZE + x) * 4) as usize..][..4];
        assert_eq!(at(11, 11), &[0xef, 0x5b, 0x50, 255]);
        assert_eq!(at(0, 0)[3], 0);
        assert_ne!(dot_rgba(Lamp::Red), dot_rgba(Lamp::Green));
    }
}

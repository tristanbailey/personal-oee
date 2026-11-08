//! The menu-bar item (D-41): a coloured dot for the mode, the minutes that
//! matter, and a small menu.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

use eframe::egui;
use oee_core::{Mode, Tracker};
use oee_store::tabs::Tab;
use tray_icon::menu::accelerator::{Accelerator, Code, Modifiers};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
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
                title: format!("{}m", t.record.cycle_minutes - into % t.record.cycle_minutes),
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
    /// A screen picked from the View menu or Settings… (the window should
    /// come forward showing it).
    pub show_tab: Option<Tab>,
    /// Mini widget picked from the Window menu.
    pub mini: bool,
}

#[derive(Default)]
struct Flags {
    toggle: AtomicBool,
    open: AtomicBool,
    quit: AtomicBool,
    /// 0 = none, otherwise the position of the tab in `Tab::ALL` plus one.
    tab: AtomicU8,
    mini: AtomicBool,
}

fn encode_tab(tab: Tab) -> u8 {
    Tab::ALL.iter().position(|t| *t == tab).map_or(0, |i| i as u8 + 1)
}

fn decode_tab(code: u8) -> Option<Tab> {
    usize::from(code).checked_sub(1).and_then(|i| Tab::ALL.get(i).copied())
}

pub struct Tray {
    icon: TrayIcon,
    toggle: MenuItem,
    flags: Arc<Flags>,
    shown: Option<TrayState>,
    /// Kept alive so the app's own menu bar stays in place (D-47).
    _app_menu: Menu,
    /// The Window menu's Mini widget item, ticked while the widget is open.
    mini: CheckMenuItem,
    mini_shown: Option<bool>,
}

/// The ids of the app menu's items, so the event handler can tell them apart.
#[derive(Default)]
struct AppIds {
    open: Vec<MenuId>,
    quit: Option<MenuId>,
    tabs: Vec<(MenuId, Tab)>,
    mini: Option<MenuId>,
}

/// The app's own menu bar (D-47): the app menu with Settings…, a View menu
/// with the screens, and a Window menu with the mini widget, so the window's
/// actions are all reachable from the menu bar, including after the window is
/// closed. Quit goes through the same flag as the menu-bar item, so it really
/// quits instead of just hiding the window (D-43).
fn app_menu(ids: &mut AppIds) -> Result<(Menu, CheckMenuItem), String> {
    let err = |e: tray_icon::menu::Error| e.to_string();
    let cmd = |code| Some(Accelerator::new(Some(Modifiers::SUPER), code));
    let open_app = MenuItem::new("Open Personal OEE", true, cmd(Code::KeyO));
    let open_window = MenuItem::new("Personal OEE", true, cmd(Code::Digit0));
    let quit = MenuItem::new("Quit Personal OEE", true, cmd(Code::KeyQ));
    let settings = MenuItem::new("Settings…", true, cmd(Code::Comma));
    let mini = CheckMenuItem::new(
        "Mini widget",
        true,
        false,
        Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyM)),
    );
    ids.open.push(open_app.id().clone());
    ids.open.push(open_window.id().clone());
    ids.quit = Some(quit.id().clone());
    ids.tabs.push((settings.id().clone(), Tab::Settings));
    ids.mini = Some(mini.id().clone());

    let app = Submenu::new("Personal OEE", true);
    app.append_items(&[
        &PredefinedMenuItem::about(None, None),
        &PredefinedMenuItem::separator(),
        &settings,
        &PredefinedMenuItem::separator(),
        &open_app,
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::hide(None),
        &PredefinedMenuItem::hide_others(None),
        &PredefinedMenuItem::show_all(None),
        &PredefinedMenuItem::separator(),
        &quit,
    ])
    .map_err(err)?;

    let view = Submenu::new("View", true);
    for (tab, code) in [(Tab::Today, Code::Digit1), (Tab::Week, Code::Digit2), (Tab::Calendar, Code::Digit3)] {
        let item = MenuItem::new(tab.label(), true, cmd(code));
        ids.tabs.push((item.id().clone(), tab));
        view.append(&item).map_err(err)?;
    }

    let window = Submenu::new("Window", true);
    window
        .append_items(&[
            &open_window,
            &mini,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::minimize(None),
        ])
        .map_err(err)?;
    let menu = Menu::new();
    menu.append_items(&[&app, &view, &window]).map_err(err)?;
    #[cfg(target_os = "macos")]
    {
        menu.init_for_nsapp();
        window.set_as_windows_menu_for_nsapp();
    }
    Ok((menu, mini))
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

        let mut ids = AppIds::default();
        ids.open.push(open.id().clone());
        let (app_menu, mini) = app_menu(&mut ids)?;

        let flags = Arc::new(Flags::default());
        let (f, toggle_id, quit_id) = (flags.clone(), toggle.id().clone(), quit.id().clone());
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if let Some((_, tab)) = ids.tabs.iter().find(|(id, _)| *id == e.id) {
                f.tab.store(encode_tab(*tab), Ordering::SeqCst);
                ctx.request_repaint();
                return;
            }
            let flag = if e.id == toggle_id {
                &f.toggle
            } else if ids.open.contains(&e.id) {
                &f.open
            } else if e.id == quit_id || Some(&e.id) == ids.quit.as_ref() {
                &f.quit
            } else if Some(&e.id) == ids.mini.as_ref() {
                &f.mini
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
            _app_menu: app_menu,
            mini,
            mini_shown: None,
        })
    }

    /// Menu picks since the last call.
    pub fn take_actions(&self) -> Actions {
        Actions {
            toggle: self.flags.toggle.swap(false, Ordering::SeqCst),
            open: self.flags.open.swap(false, Ordering::SeqCst),
            quit: self.flags.quit.swap(false, Ordering::SeqCst),
            show_tab: decode_tab(self.flags.tab.swap(0, Ordering::SeqCst)),
            mini: self.flags.mini.swap(false, Ordering::SeqCst),
        }
    }

    /// Tick the Mini widget menu item while the widget is open.
    pub fn set_mini_open(&mut self, open: bool) {
        if self.mini_shown != Some(open) {
            self.mini.set_checked(open);
            self.mini_shown = Some(open);
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

    #[test]
    fn menu_tab_picks_survive_the_round_trip_through_a_flag() {
        for tab in Tab::ALL {
            assert_eq!(decode_tab(encode_tab(tab)), Some(tab));
        }
        assert_eq!(decode_tab(0), None);
        assert_eq!(decode_tab(99), None);
    }

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

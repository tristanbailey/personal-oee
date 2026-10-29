//! The prototype's colours as an egui theme, for light and dark mode (D-46).
//!
//! Text is always `ink` (or `muted`); the status colours go on fills, outlines
//! and dots. The soft pairs are too close in brightness to carry coloured text
//! (amber on pale amber is under 3:1 in light mode), and the tests below hold
//! the pairs that are used to their contrast.

use eframe::egui::{self, Color32, Stroke, Visuals};

pub const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub accent: Color32,
    pub work: Color32,
    pub work_soft: Color32,
    pub down: Color32,
    pub down_soft: Color32,
    pub plan: Color32,
    pub plan_soft: Color32,
    pub change: Color32,
    pub change_soft: Color32,
    pub rework: Color32,
    /// Text on a solid `work` or `down` fill (white on light, dark ink on dark).
    pub on_fill: Color32,
    /// Text on the `accent` fill.
    pub on_accent: Color32,
}

pub const LIGHT: Palette = Palette {
    bg: rgb(0xeef0ef),
    surface: rgb(0xffffff),
    ink: rgb(0x15191a),
    muted: rgb(0x5b6466),
    line: rgb(0xd3d8d6),
    accent: rgb(0xd9a400),
    work: rgb(0x1d8048),
    work_soft: rgb(0xdcefe3),
    down: rgb(0xc2352b),
    down_soft: rgb(0xf7dfdc),
    plan: rgb(0x44689c),
    plan_soft: rgb(0xdfe7f2),
    change: rgb(0xc47f00),
    change_soft: rgb(0xf6ead2),
    rework: rgb(0x7a4cae),
    on_fill: rgb(0xffffff),
    on_accent: rgb(0x15191a),
};

pub const DARK: Palette = Palette {
    bg: rgb(0x111415),
    surface: rgb(0x1a1e1f),
    ink: rgb(0xe6ebe9),
    muted: rgb(0x96a1a2),
    line: rgb(0x2e3536),
    accent: rgb(0xf0c22e),
    work: rgb(0x46b874),
    work_soft: rgb(0x183a26),
    down: rgb(0xef5b50),
    down_soft: rgb(0x4a1f1c),
    plan: rgb(0x84a6dc),
    plan_soft: rgb(0x1e2b3e),
    change: rgb(0xf0a62a),
    change_soft: rgb(0x3e2e10),
    rework: rgb(0xb08ae6),
    on_fill: rgb(0x15191a),
    on_accent: rgb(0x15191a),
};

pub fn palette(dark: bool) -> Palette {
    if dark {
        DARK
    } else {
        LIGHT
    }
}

/// The palette for whatever theme the UI is drawing in.
pub fn of(ui: &egui::Ui) -> Palette {
    palette(ui.visuals().dark_mode)
}

fn visuals(p: &Palette, base: Visuals) -> Visuals {
    let mut v = base;
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.surface;
    v.faint_bg_color = p.surface;
    v.override_text_color = Some(p.ink);
    v.hyperlink_color = p.plan;
    v.window_stroke = Stroke::new(1.0_f32, p.line);
    v.selection.bg_fill = p.accent;
    v.selection.stroke = Stroke::new(1.0_f32, p.on_accent);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.bg_stroke = Stroke::new(1.0_f32, p.line);
        w.fg_stroke.color = p.ink;
    }
    v.widgets.inactive.weak_bg_fill = p.surface;
    v.widgets.inactive.bg_fill = p.surface;
    v
}

/// Install the theme for both light and dark mode. egui follows the system.
pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(preference(std::env::var(THEME_ENV).ok().as_deref()));
    ctx.set_visuals_of(egui::Theme::Light, visuals(&LIGHT, Visuals::light()));
    ctx.set_visuals_of(egui::Theme::Dark, visuals(&DARK, Visuals::dark()));
}

pub const THEME_ENV: &str = "PERSONAL_OEE_THEME";

/// `light` or `dark` force a theme; anything else follows the system.
pub fn preference(value: Option<&str>) -> egui::ThemePreference {
    match value {
        Some("light") => egui::ThemePreference::Light,
        Some("dark") => egui::ThemePreference::Dark,
        _ => egui::ThemePreference::System,
    }
}

/// A line of error text in the theme's red.
pub fn error(ui: &mut egui::Ui, text: &str) {
    let p = of(ui);
    ui.label(egui::RichText::new(text).color(p.down).strong());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color32) -> f64 {
        let f = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    /// WCAG contrast ratio.
    fn contrast(a: Color32, b: Color32) -> f64 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn the_palettes_use_the_prototype_tokens() {
        assert_eq!(LIGHT.bg, Color32::from_rgb(0xee, 0xf0, 0xef));
        assert_eq!(LIGHT.accent, Color32::from_rgb(0xd9, 0xa4, 0x00));
        assert_eq!(LIGHT.work_soft, Color32::from_rgb(0xdc, 0xef, 0xe3));
        assert_eq!(LIGHT.rework, Color32::from_rgb(0x7a, 0x4c, 0xae));
        assert_eq!(DARK.bg, Color32::from_rgb(0x11, 0x14, 0x15));
        assert_eq!(DARK.down, Color32::from_rgb(0xef, 0x5b, 0x50));
        assert_eq!(DARK.change_soft, Color32::from_rgb(0x3e, 0x2e, 0x10));
        assert_eq!(palette(true), DARK);
        assert_eq!(palette(false), LIGHT);
    }

    #[test]
    fn text_has_readable_contrast_everywhere_it_is_drawn() {
        for (name, p) in [("light", LIGHT), ("dark", DARK)] {
            // Body text on the window and on tiles.
            for bg in [p.bg, p.surface] {
                assert!(contrast(p.ink, bg) >= 7.0, "{name} ink on page");
            }
            assert!(contrast(p.muted, p.bg) >= 4.5, "{name} muted on bg");
            // Ink on every soft fill (grid cells, pills, banners).
            for soft in [p.work_soft, p.down_soft, p.plan_soft, p.change_soft] {
                assert!(contrast(p.ink, soft) >= 7.0, "{name} ink on soft fill");
            }
            // Button text on solid fills, and the selected tab.
            for fill in [p.work, p.down] {
                assert!(contrast(p.on_fill, fill) >= 4.5, "{name} text on button");
            }
            assert!(contrast(p.on_accent, p.accent) >= 7.0, "{name} text on accent");
            // The OEE tile is the ink colour with the page colour on top.
            assert!(contrast(p.bg, p.ink) >= 7.0, "{name} OEE tile");
        }
    }

    #[test]
    fn status_colours_stand_out_from_their_soft_fills_as_outlines() {
        // Outlines and dots only need to be visible, not read.
        for p in [LIGHT, DARK] {
            for (strong, soft) in [
                (p.work, p.work_soft),
                (p.down, p.down_soft),
                (p.plan, p.plan_soft),
                (p.change, p.change_soft),
            ] {
                assert!(contrast(strong, soft) >= 2.5);
            }
            assert!(contrast(p.rework, p.surface) >= 4.5);
        }
    }

    #[test]
    fn the_theme_can_be_forced_for_testing() {
        assert_eq!(preference(Some("dark")), egui::ThemePreference::Dark);
        assert_eq!(preference(Some("light")), egui::ThemePreference::Light);
        assert_eq!(preference(Some("system")), egui::ThemePreference::System);
        assert_eq!(preference(None), egui::ThemePreference::System);
    }

    #[test]
    fn the_theme_visuals_use_the_palette() {
        let v = visuals(&LIGHT, Visuals::light());
        assert_eq!(v.panel_fill, LIGHT.bg);
        assert_eq!(v.window_fill, LIGHT.surface);
        assert_eq!(v.selection.bg_fill, LIGHT.accent);
        assert_eq!(v.override_text_color, Some(LIGHT.ink));
        let d = visuals(&DARK, Visuals::dark());
        assert_eq!(d.panel_fill, DARK.bg);
        assert!(d.dark_mode);
    }
}

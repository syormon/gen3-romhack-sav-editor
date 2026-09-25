//! The slate/amber palette from `src/style.css`, as egui visuals.

use eframe::egui::{self, Color32, CornerRadius, Stroke};

pub const BG: Color32 = Color32::from_rgb(0x0F, 0x17, 0x2A); // slate-900
pub const PANEL: Color32 = Color32::from_rgb(0x1E, 0x29, 0x3B); // slate-800
pub const BORDER: Color32 = Color32::from_rgb(0x33, 0x41, 0x55); // slate-700
pub const BORDER_LIGHT: Color32 = Color32::from_rgb(0x47, 0x55, 0x69); // slate-600
pub const MUTED: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8); // slate-400
pub const DIM: Color32 = Color32::from_rgb(0x64, 0x74, 0x8B); // slate-500
pub const TEXT: Color32 = Color32::from_rgb(0xF8, 0xFA, 0xFC); // slate-50
pub const AMBER: Color32 = Color32::from_rgb(0xFA, 0xCC, 0x15); // yellow-400
pub const BLUE: Color32 = Color32::from_rgb(0x60, 0xA5, 0xFA); // blue-400
pub const BLUE_DEEP: Color32 = Color32::from_rgb(0x25, 0x63, 0xEB); // blue-600
pub const SKY: Color32 = Color32::from_rgb(0x02, 0x84, 0xC7); // sky-600
pub const CYAN: Color32 = Color32::from_rgb(0x38, 0xBD, 0xF8); // sky-400
pub const GREEN: Color32 = Color32::from_rgb(0x10, 0xB9, 0x81); // emerald-500
pub const MINT: Color32 = Color32::from_rgb(0x34, 0xD3, 0x99); // emerald-400
pub const RED: Color32 = Color32::from_rgb(0xEF, 0x44, 0x44); // red-500
pub const RED_DEEP: Color32 = Color32::from_rgb(0xDC, 0x26, 0x26); // red-600
pub const SALMON: Color32 = Color32::from_rgb(0xF8, 0x71, 0x71); // red-400
pub const VIOLET: Color32 = Color32::from_rgb(0x7C, 0x3A, 0xED); // violet-600
pub const PINK: Color32 = Color32::from_rgb(0xEC, 0x48, 0x99); // pink-500

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    let v = &mut style.visuals;

    v.dark_mode = true;
    v.override_text_color = Some(TEXT);
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = BG;
    v.faint_bg_color = BG;
    v.window_stroke = Stroke::new(1.0, BORDER_LIGHT);
    v.window_corner_radius = CornerRadius::same(10);
    v.selection.bg_fill = BLUE_DEEP.gamma_multiply(0.7);
    v.selection.stroke = Stroke::new(1.0, TEXT);
    v.hyperlink_color = BLUE;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = PANEL;
    w.noninteractive.weak_bg_fill = PANEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    w.noninteractive.corner_radius = CornerRadius::same(6);

    w.inactive.bg_fill = BORDER;
    w.inactive.weak_bg_fill = BORDER;
    w.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.inactive.corner_radius = CornerRadius::same(6);

    w.hovered.bg_fill = BORDER_LIGHT;
    w.hovered.weak_bg_fill = BORDER_LIGHT;
    w.hovered.bg_stroke = Stroke::new(1.0, BLUE);
    w.hovered.fg_stroke = Stroke::new(1.5, TEXT);
    w.hovered.corner_radius = CornerRadius::same(6);

    w.active.bg_fill = BLUE_DEEP;
    w.active.weak_bg_fill = BLUE_DEEP;
    w.active.bg_stroke = Stroke::new(1.0, BLUE);
    w.active.fg_stroke = Stroke::new(2.0, TEXT);
    w.active.corner_radius = CornerRadius::same(6);

    w.open.bg_fill = BG;
    w.open.weak_bg_fill = BORDER;
    w.open.bg_stroke = Stroke::new(1.0, BORDER_LIGHT);

    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size.y = 24.0;

    ctx.set_style_of(egui::Theme::Dark, style);
}

/// A card-like frame: `#0f172a` fill with a slate border, as `.card-slot`.
pub fn card_frame(stroke_color: Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(BG)
        .stroke(Stroke::new(1.0, stroke_color))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(8, 8))
}

/// A section panel: `#1e293b` fill with a slate border.
pub fn panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(14))
}

/// An inset well: `#0f172a` on a panel.
pub fn well_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::same(10))
}

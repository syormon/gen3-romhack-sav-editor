//! Shared widgets: remote sprites with a fallback chain, coloured buttons,
//! labelled fields and the searchable pickers that stand in for the web build's
//! `<select>` and datalist elements.

use eframe::egui::{
    self, Color32, CornerRadius, Response, RichText, Sense, Stroke, TextureOptions, Ui, Vec2,
};
use egui::load::{SizeHint, TexturePoll};

use super::theme;
use crate::engine::lookup::{get_item_sprite_url, get_species_name, sprite_candidates};

/// Draws the first of `urls` that has loaded, a spinner while one is in flight,
/// and a lettered placeholder when every URL failed (or sprites are off).
fn remote_image(ui: &mut Ui, urls: &[String], size: f32, placeholder: &str, enabled: bool) {
    let target = Vec2::splat(size);

    if enabled {
        let mut any_pending = false;
        for uri in urls {
            match ui
                .ctx()
                .try_load_texture(uri, TextureOptions::LINEAR, SizeHint::default())
            {
                Ok(TexturePoll::Ready { texture }) => {
                    ui.add(
                        egui::Image::from_texture(texture)
                            .fit_to_exact_size(target)
                            .maintain_aspect_ratio(true),
                    );
                    return;
                }
                Ok(TexturePoll::Pending { .. }) => {
                    any_pending = true;
                    break;
                }
                Err(_) => continue,
            }
        }
        if any_pending {
            let (rect, _) = ui.allocate_exact_size(target, Sense::hover());
            ui.put(rect, egui::Spinner::new().size(size * 0.5));
            return;
        }
    }

    sprite_placeholder(ui, target, placeholder);
}

fn sprite_placeholder(ui: &mut Ui, size: Vec2, text: &str) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(6), theme::PANEL);
    painter.rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0, theme::BORDER),
        egui::StrokeKind::Inside,
    );
    let initials: String = text
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(3)
        .collect::<String>()
        .to_uppercase();
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initials,
        egui::FontId::proportional((size.x * 0.32).clamp(9.0, 22.0)),
        theme::DIM,
    );
}

pub fn species_sprite(ui: &mut Ui, species_id: u32, shiny: bool, size: f32, enabled: bool) {
    let urls = sprite_candidates(species_id, shiny);
    let name = get_species_name(species_id);
    remote_image(ui, &urls, size, &name, enabled);
}

pub fn item_sprite(ui: &mut Ui, item_id: u32, name: &str, size: f32, enabled: bool) {
    let url = get_item_sprite_url(item_id, Some(name));
    remote_image(ui, &[url], size, name, enabled);
}

/// A filled button in one of the palette colours.
pub fn color_button(ui: &mut Ui, label: &str, fill: Color32) -> Response {
    let text_color = if fill == theme::BORDER {
        theme::AMBER
    } else {
        theme::TEXT
    };
    ui.add(
        egui::Button::new(RichText::new(label).color(text_color).strong())
            .fill(fill)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(6)),
    )
}

pub fn small_color_button(ui: &mut Ui, label: &str, fill: Color32) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(theme::TEXT).size(11.0).strong())
            .fill(fill)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(4))
            .small(),
    )
}

pub fn field_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(11.0).color(theme::MUTED));
}

pub fn heading(ui: &mut Ui, text: &str, color: Color32) {
    ui.label(RichText::new(text).size(15.0).strong().color(color));
}

pub fn separator(ui: &mut Ui) {
    ui.add(egui::Separator::default().spacing(10.0));
}

/// A `ComboBox` over `(id, label)` pairs, mirroring a plain `<select>`.
pub fn id_combo(
    ui: &mut Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    width: f32,
    current: &mut u32,
    options: impl Iterator<Item = (u32, String)>,
) -> bool {
    let options: Vec<(u32, String)> = options.collect();
    let selected_label = options
        .iter()
        .find(|(id, _)| id == current)
        .map(|(_, name)| name.clone())
        .unwrap_or_else(|| "(None)".to_string());

    let mut changed = false;
    egui::ComboBox::from_id_salt(salt)
        .width(width)
        .selected_text(RichText::new(selected_label).size(12.0))
        .show_ui(ui, |ui| {
            ui.set_min_width(width);
            for (id, name) in &options {
                if ui
                    .selectable_label(*id == *current, RichText::new(name).size(12.0))
                    .clicked()
                {
                    *current = *id;
                    changed = true;
                }
            }
        });
    changed
}

/// Type-to-filter picker used for the long species/move/item lists, which are
/// far too large for a plain scrolling combo.
pub struct SearchPicker {
    pub salt: String,
    pub width: f32,
    pub row_height: f32,
    pub max_rows: usize,
}

impl SearchPicker {
    pub fn new(salt: impl Into<String>, width: f32) -> Self {
        Self {
            salt: salt.into(),
            width,
            row_height: 20.0,
            max_rows: 12,
        }
    }

    /// Shows a text field plus a filtered list; returns the id that was picked.
    ///
    /// `entries` is `(id, label, highlighted)`; highlighted rows are tinted, the
    /// way learnset moves are green in the web build.
    pub fn show(
        self,
        ui: &mut Ui,
        query: &mut String,
        entries: impl Iterator<Item = (u32, String, bool)>,
    ) -> Option<u32> {
        let mut picked = None;
        let q = query.trim().to_lowercase();

        ui.add(
            egui::TextEdit::singleline(query)
                .desired_width(self.width)
                .hint_text("Type to search…")
                .font(egui::TextStyle::Body),
        );

        let matches: Vec<(u32, String, bool)> = entries
            .filter(|(_, name, _)| q.is_empty() || name.to_lowercase().contains(&q))
            .take(400)
            .collect();

        egui::ScrollArea::vertical()
            .id_salt(&self.salt)
            .max_height(self.row_height * self.max_rows as f32)
            .show(ui, |ui| {
                ui.set_min_width(self.width);
                if matches.is_empty() {
                    ui.label(RichText::new("No matches").size(12.0).color(theme::MUTED));
                }
                for (id, name, highlight) in &matches {
                    let color = if *highlight { theme::MINT } else { theme::TEXT };
                    let text = if *highlight {
                        RichText::new(format!("★ {name}")).size(12.0).color(color)
                    } else {
                        RichText::new(name).size(12.0).color(color)
                    };
                    if ui.selectable_label(false, text).clicked() {
                        picked = Some(*id);
                    }
                }
            });

        picked
    }
}

/// A clamped integer field, standing in for `<input type="number" min max>`.
pub fn int_field(ui: &mut Ui, value: &mut u32, min: u32, max: u32) -> Response {
    let mut v = i64::from(*value);
    let response = ui.add(
        egui::DragValue::new(&mut v)
            .range(i64::from(min)..=i64::from(max))
            .speed(1.0)
            .update_while_editing(false),
    );
    *value = v.clamp(i64::from(min), i64::from(max)) as u32;
    response
}

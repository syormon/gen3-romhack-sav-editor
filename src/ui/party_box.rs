//! Party sidebar and PC box grid, including drag-and-drop between slots.

use eframe::egui::{self, RichText, Sense, Ui};

use crate::engine::layout::{box_capacity, party_size};
use crate::engine::lookup::get_species_name;
use crate::engine::save_parser::Pokemon;

use super::app::{Action, EditorApp, SlotRef};
use super::theme;
use super::widgets;

pub fn party_panel(app: &mut EditorApp, ui: &mut Ui) {
    // By slot, so each card is the record the game has in that position.
    let party: Vec<Option<Pokemon>> = match app.save.as_ref() {
        Some(save) => (0..party_size()).map(|i| save.party_slot(i)).collect(),
        None => return,
    };
    let party_len = party.iter().flatten().count();
    let selected = app.inspector.as_ref().map(|i| i.slot);
    let load_sprites = app.load_sprites;
    let mut actions: Vec<Action> = Vec::new();

    theme::panel_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            widgets::heading(ui, "Party Pokémon", theme::TEXT);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::Frame::new()
                    .fill(theme::BG)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER))
                    .corner_radius(egui::CornerRadius::same(10))
                    .inner_margin(egui::Margin::symmetric(8, 2))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{party_len} / {}", party_size()))
                                .size(12.0)
                                .color(theme::MUTED),
                        );
                    });
            });
        });
        ui.add_space(6.0);

        for (i, mon) in party.iter().enumerate() {
            let slot = SlotRef::party(i);
            slot_card(
                ui,
                mon.as_ref(),
                slot,
                selected == Some(slot),
                load_sprites,
                &mut actions,
                CardStyle::Wide,
            );
            ui.add_space(6.0);
        }
    });

    app.actions.append(&mut actions);
}

pub fn box_panel(app: &mut EditorApp, ui: &mut Ui) {
    // A game whose storage this editor cannot read declares no boxes; there is
    // then nothing to draw, and the paging arithmetic below has no valid range.
    let total_boxes = app.save.as_ref().map_or(0, |s| s.box_count());
    if total_boxes == 0 || box_capacity() == 0 {
        return;
    }
    let current_box = app.current_box;
    let selected = app.inspector.as_ref().map(|i| i.slot);
    let load_sprites = app.load_sprites;

    let mons: Vec<Option<Pokemon>> = match app.save.as_ref() {
        Some(save) => (0..box_capacity())
            .map(|i| save.box_pokemon(current_box, i))
            .collect(),
        None => return,
    };
    let box_name = app
        .save
        .as_ref()
        .and_then(|s| s.box_names().get(current_box).cloned())
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| format!("Box {}", current_box + 1));

    let mut actions: Vec<Action> = Vec::new();
    let mut delta: i32 = 0;

    theme::panel_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    current_box > 0,
                    egui::Button::new(RichText::new("◀").strong()),
                )
                .clicked()
            {
                delta = -1;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        current_box + 1 < total_boxes,
                        egui::Button::new(RichText::new("▶").strong()),
                    )
                    .clicked()
                {
                    delta = 1;
                }
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{box_name}  ({}/{})", current_box + 1, total_boxes))
                            .size(16.0)
                            .strong()
                            .color(theme::BLUE),
                    );
                });
            });
        });
        ui.add_space(8.0);

        let columns = 6;
        let spacing = ui.spacing().item_spacing.x;
        let cell_width =
            ((ui.available_width() - spacing * (columns as f32 - 1.0)) / columns as f32).max(72.0);

        egui::Grid::new("box-grid")
            .num_columns(columns)
            .spacing([spacing, spacing])
            .show(ui, |ui| {
                for (i, mon) in mons.iter().enumerate() {
                    let slot = SlotRef::boxed(current_box, i);
                    ui.scope(|ui| {
                        ui.set_width(cell_width);
                        slot_card(
                            ui,
                            mon.as_ref(),
                            slot,
                            selected == Some(slot),
                            load_sprites,
                            &mut actions,
                            CardStyle::Grid,
                        );
                    });
                    if (i + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
    });

    app.actions.append(&mut actions);
    if delta != 0 {
        let next = app.current_box as i32 + delta;
        app.current_box = next.clamp(0, total_boxes as i32 - 1) as usize;
        // The inspector points at a slot in the box we just left.
        if app.inspector.as_ref().is_some_and(|i| !i.slot.is_party()) {
            app.inspector = None;
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum CardStyle {
    /// Party sidebar: sprite beside name and level.
    Wide,
    /// Box grid: sprite above a compact caption.
    Grid,
}

/// One slot: click to inspect (or to create, when empty), drag to move or swap.
fn slot_card(
    ui: &mut Ui,
    mon: Option<&Pokemon>,
    slot: SlotRef,
    selected: bool,
    load_sprites: bool,
    actions: &mut Vec<Action>,
    style: CardStyle,
) {
    let id = egui::Id::new(("slot", slot.kind, slot.box_index, slot.index));
    let being_dragged = ui.ctx().is_being_dragged(id);
    let hovering_payload = egui::DragAndDrop::has_payload_of_type::<SlotRef>(ui.ctx());

    let border = if selected {
        theme::BLUE
    } else if mon.is_some() {
        theme::BORDER
    } else {
        theme::BORDER_LIGHT
    };

    let frame = theme::card_frame(border).fill(if mon.is_some() {
        theme::BG
    } else {
        theme::BG.gamma_multiply(0.5)
    });

    let response = frame
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            match mon {
                Some(mon) => {
                    // Drag source paints its own copy under the cursor.
                    ui.dnd_drag_source(id, slot, |ui| draw_mon(ui, mon, load_sprites, style));
                }
                None => draw_empty(ui, style),
            }
        })
        .response;

    let rect = response.rect;
    let click = ui.interact(rect, id.with("click"), Sense::click());

    if click.clicked() && !being_dragged {
        actions.push(match mon {
            Some(_) => Action::Inspect(slot),
            None => Action::Add(slot),
        });
    }

    // Drop target highlight and release handling.
    if hovering_payload && click.contains_pointer() {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(8),
            egui::Stroke::new(2.0, theme::CYAN),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(src) = click.dnd_release_payload::<SlotRef>()
        && *src != slot
    {
        actions.push(Action::Move {
            src: *src,
            dst: slot,
        });
    }
    if being_dragged {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(8),
            egui::Stroke::new(2.0, theme::AMBER),
            egui::StrokeKind::Inside,
        );
    }
}

fn draw_mon(ui: &mut Ui, mon: &Pokemon, load_sprites: bool, style: CardStyle) {
    let species_name = get_species_name(mon.species);
    let display_name = mon.display_name();
    let shiny = mon.is_shiny();

    match style {
        CardStyle::Wide => {
            ui.horizontal(|ui| {
                widgets::species_sprite(ui, mon.species, shiny, 44.0, load_sprites);
                ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&display_name).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("Lv.{}", mon.level))
                                    .size(11.0)
                                    .color(theme::MUTED),
                            );
                        });
                    });
                    ui.label(
                        RichText::new(if shiny {
                            format!("{species_name} ✨")
                        } else {
                            species_name.clone()
                        })
                        .size(11.0)
                        .color(theme::BLUE),
                    );
                });
            });
        }
        CardStyle::Grid => {
            ui.vertical_centered(|ui| {
                widgets::species_sprite(ui, mon.species, shiny, 44.0, load_sprites);
                ui.label(
                    RichText::new(truncate(&display_name, 11))
                        .size(11.0)
                        .strong(),
                );
                ui.label(
                    RichText::new(if shiny {
                        format!("Lv.{} ✨", mon.level)
                    } else {
                        format!("Lv.{}", mon.level)
                    })
                    .size(10.0)
                    .color(theme::MUTED),
                );
            });
        }
    }
}

fn draw_empty(ui: &mut Ui, style: CardStyle) {
    let height = match style {
        CardStyle::Wide => 44.0,
        CardStyle::Grid => 62.0,
    };
    ui.vertical_centered(|ui| {
        ui.add_space(height / 2.0 - 8.0);
        ui.label(RichText::new("+ Add").size(12.0).color(theme::DIM));
        ui.add_space(height / 2.0 - 8.0);
    });
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max_chars - 1).collect();
        out.push('…');
        out
    }
}

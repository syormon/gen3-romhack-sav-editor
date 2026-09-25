//! The Pokémon inspector: species, ability, nature, tera type, level, ball,
//! held item, the four move slots and IVs/EVs.

use eframe::egui::{self, RichText, Ui};

use crate::engine::experience::experience_for_level;
use crate::engine::lookup::{
    NATURES_LIST, all_moves_list, all_species_list, alphabetical_item_list, get_item_name,
    get_move_base_pp, get_move_name, get_species_abilities, get_species_name, learnset_move_ids,
    pokeballs, tera_types,
};
use crate::engine::personality::{
    make_personality_non_shiny, make_personality_shiny, set_personality_nature,
};

use super::app::EditorApp;
use super::theme;
use super::widgets::{self, SearchPicker, color_button, field_label, small_color_button};

const STAT_LABELS: [&str; 6] = ["HP", "Atk", "Def", "Spe", "SpA", "SpD"];

pub fn inspector_panel(app: &mut EditorApp, ui: &mut Ui) {
    if app.inspector.is_none() {
        return;
    }

    let mut close = false;
    let mut apply = false;
    let mut release = false;
    let load_sprites = app.load_sprites;

    theme::panel_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        let Some(insp) = app.inspector.as_mut() else {
            return;
        };

        // ---------------------------------------------------------- header
        ui.horizontal(|ui| {
            let shiny = insp.mon.is_shiny();
            egui::Frame::new()
                .fill(theme::BG)
                .stroke(egui::Stroke::new(1.0, theme::BORDER))
                .corner_radius(egui::CornerRadius::same(12))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    widgets::species_sprite(ui, insp.mon.species, shiny, 120.0, load_sprites);
                });

            ui.vertical(|ui| {
                let species_name = get_species_name(insp.mon.species);
                ui.label(
                    RichText::new(insp.mon.display_name())
                        .size(22.0)
                        .strong()
                        .color(theme::BLUE),
                );
                ui.label(
                    RichText::new(format!("{species_name} (Level {})", insp.mon.level))
                        .size(12.0)
                        .color(theme::MUTED),
                );
                ui.add_space(4.0);
                let badge = if shiny { "✨ Shiny" } else { "☆ Not shiny" };
                if ui
                    .button(RichText::new(badge).size(12.0))
                    .on_hover_text("Toggle shiny (rewrites the personality value)")
                    .clicked()
                {
                    if shiny {
                        insp.mon.personality =
                            make_personality_non_shiny(insp.mon.personality, insp.mon.ot_id);
                        insp.mon.shiny_modifier = 0;
                    } else {
                        insp.mon.personality =
                            make_personality_shiny(insp.mon.personality, insp.mon.ot_id);
                    }
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button(RichText::new("✕").size(16.0)).clicked() {
                    close = true;
                }
                if color_button(ui, "🗑 Release", theme::RED_DEEP).clicked() {
                    insp.confirm_release = true;
                }
            });
        });

        widgets::separator(ui);

        // -------------------------------------------------------- three columns
        ui.columns(3, |cols| {
            core_details(&mut cols[0], insp);
            moves_column(&mut cols[1], insp);
            stats_column(&mut cols[2], insp);
        });

        ui.add_space(14.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if color_button(ui, "Apply Changes", theme::GREEN).clicked() {
                apply = true;
            }
        });
    });

    // ------------------------------------------------------------- release
    if app.inspector.as_ref().is_some_and(|i| i.confirm_release) {
        let name = app
            .inspector
            .as_ref()
            .map(|i| i.mon.display_name())
            .unwrap_or_default();
        let mut cancel = false;
        egui::Modal::new(egui::Id::new("release-modal")).show(ui.ctx(), |ui| {
            ui.set_width(360.0);
            widgets::heading(ui, "Release Pokémon", theme::AMBER);
            ui.add_space(8.0);
            ui.label(format!("Are you sure you want to release {name}?"));
            ui.add_space(14.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if color_button(ui, "Release", theme::RED_DEEP).clicked() {
                    release = true;
                }
                if color_button(ui, "Cancel", theme::BORDER).clicked() {
                    cancel = true;
                }
            });
        });
        if cancel {
            if let Some(insp) = app.inspector.as_mut() {
                insp.confirm_release = false;
            }
        }
    }

    if release {
        if let Some(slot) = app.inspector.as_ref().map(|i| i.slot) {
            app.release_mon(slot);
            app.toast("Pokémon released");
        }
        return;
    }
    if close {
        app.inspector = None;
        return;
    }
    if apply {
        app.commit_inspector();
        // Re-read the slot so the panel shows exactly what landed in the save.
        if let Some(slot) = app.inspector.as_ref().map(|i| i.slot) {
            if let Some(updated) = app.mon_at(slot) {
                if let Some(insp) = app.inspector.as_mut() {
                    insp.mon = updated;
                }
            }
        }
        app.toast("Changes applied");
    }
}

fn core_details(ui: &mut Ui, insp: &mut super::app::Inspector) {
    ui.vertical(|ui| {
        field_label(ui, "Species");
        let current_species = get_species_name(insp.mon.species);
        if ui
            .button(RichText::new(&current_species).size(12.0))
            .clicked()
        {
            insp.species_picker_open = !insp.species_picker_open;
            insp.species_query.clear();
        }
        if insp.species_picker_open {
            let picked = SearchPicker::new("species-picker", ui.available_width() - 12.0).show(
                ui,
                &mut insp.species_query,
                all_species_list()
                    .iter()
                    .map(|s| (s.id, s.name.clone(), false)),
            );
            if let Some(id) = picked {
                if id != insp.mon.species {
                    insp.mon.species = id;
                    // The new species may sit on a different growth curve; keep
                    // the level the user sees and restate the experience.
                    insp.mon.experience = experience_for_level(id, insp.mon.level);
                }
                insp.species_picker_open = false;
            }
        }

        ui.add_space(6.0);
        field_label(ui, "Ability");
        let abilities = get_species_abilities(insp.mon.species);
        let selected = abilities
            .iter()
            .find(|a| a.slot == insp.mon.ability_num)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "None".to_string());
        egui::ComboBox::from_id_salt("ability")
            .width(ui.available_width() - 12.0)
            .selected_text(RichText::new(selected).size(12.0))
            .show_ui(ui, |ui| {
                for ability in &abilities {
                    let label = if ability.available {
                        ability.name.clone()
                    } else {
                        format!("{} (None)", ability.name)
                    };
                    ui.add_enabled_ui(ability.available, |ui| {
                        if ui
                            .selectable_label(
                                insp.mon.ability_num == ability.slot,
                                RichText::new(label).size(12.0),
                            )
                            .clicked()
                        {
                            insp.mon.ability_num = ability.slot;
                        }
                    });
                }
            });

        ui.add_space(6.0);
        field_label(ui, "Nature");
        let nature = insp.mon.nature();
        let nature_label = NATURES_LIST
            .iter()
            .find(|n| n.id == nature)
            .map(|n| format!("{}{}", n.name, n.modifier))
            .unwrap_or_else(|| "Hardy(Neutral)".to_string());
        egui::ComboBox::from_id_salt("nature")
            .width(ui.available_width() - 12.0)
            .selected_text(RichText::new(nature_label).size(12.0))
            .show_ui(ui, |ui| {
                for n in NATURES_LIST.iter() {
                    if ui
                        .selectable_label(
                            n.id == nature,
                            RichText::new(format!("{}{}", n.name, n.modifier)).size(12.0),
                        )
                        .clicked()
                    {
                        insp.mon.personality = set_personality_nature(insp.mon.personality, n.id);
                    }
                }
            });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                field_label(ui, "Tera Type");
                widgets::id_combo(
                    ui,
                    "tera",
                    110.0,
                    &mut insp.mon.tera_type,
                    tera_types().into_iter().map(|t| (t.id, t.name)),
                );
            });
            ui.vertical(|ui| {
                field_label(ui, "Level");
                let mut level = u32::from(insp.mon.level);
                widgets::int_field(ui, &mut level, 1, 100, 60.0);
                let level = level as u8;
                if level != insp.mon.level {
                    // Level is derived from experience (boxed Pokémon store no
                    // level at all, and the game recalculates a party member's
                    // from experience), so move the experience total with it.
                    insp.mon.level = level;
                    insp.mon.experience = experience_for_level(insp.mon.species, level);
                }
            });
        });

        ui.add_space(6.0);
        ui.vertical(|ui| {
            field_label(ui, "Poké Ball");
            widgets::id_combo(
                ui,
                "ball",
                ui.available_width() - 12.0,
                &mut insp.mon.pokeball,
                pokeballs().into_iter().map(|b| (b.id, b.name)),
            );
        });

        ui.add_space(6.0);
        field_label(ui, "Held Item");
        let held_name = get_item_name(insp.mon.held_item);
        if ui.button(RichText::new(&held_name).size(12.0)).clicked() {
            insp.item_picker_open = !insp.item_picker_open;
            insp.item_query.clear();
        }
        if insp.item_picker_open {
            let picked = SearchPicker::new("held-item-picker", ui.available_width() - 12.0).show(
                ui,
                &mut insp.item_query,
                std::iter::once((0, "(None)".to_string(), false)).chain(
                    alphabetical_item_list()
                        .iter()
                        .map(|it| (it.id, it.name.clone(), false)),
                ),
            );
            if let Some(id) = picked {
                insp.mon.held_item = id;
                insp.item_picker_open = false;
            }
        }
    });
}

fn moves_column(ui: &mut Ui, insp: &mut super::app::Inspector) {
    ui.vertical(|ui| {
        widgets::heading(ui, "Moves & PP Up", theme::AMBER);
        ui.add_space(4.0);

        let learnset = learnset_move_ids(insp.mon.species);

        for i in 0..4 {
            theme::well_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());

                let move_name = get_move_name(insp.mon.moves[i]);
                let is_learnset = learnset.contains(&insp.mon.moves[i]);
                let label = if is_learnset {
                    RichText::new(format!("★ {move_name}"))
                        .size(12.0)
                        .color(theme::MINT)
                } else {
                    RichText::new(move_name).size(12.0)
                };
                if ui.button(label).clicked() {
                    insp.move_picker = if insp.move_picker == Some(i) {
                        None
                    } else {
                        Some(i)
                    };
                    insp.move_query.clear();
                }

                if insp.move_picker == Some(i) {
                    // Learnset moves first and starred, as in the web build's
                    // "Compatible / Learnset Moves" option group.
                    let mut entries: Vec<(u32, String, bool)> =
                        vec![(0, "(None)".to_string(), false)];
                    let mut others: Vec<(u32, String, bool)> = Vec::new();
                    for m in all_moves_list() {
                        if learnset.contains(&m.id) {
                            entries.push((m.id, m.name.clone(), true));
                        } else {
                            others.push((m.id, m.name.clone(), false));
                        }
                    }
                    entries.extend(others);

                    let picked = SearchPicker::new(
                        format!("move-picker-{i}"),
                        ui.available_width(),
                    )
                    .show(ui, &mut insp.move_query, entries.into_iter());
                    if let Some(id) = picked {
                        insp.mon.moves[i] = id;
                        insp.mon.pps[i] = max_pp(id, insp.mon.pp_bonuses[i]);
                        insp.move_picker = None;
                    }
                }

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        field_label(ui, "PP");
                        widgets::int_field(ui, &mut insp.mon.pps[i], 0, 99, 50.0);
                    });
                    ui.vertical(|ui| {
                        field_label(ui, "PP Up (+0-3)");
                        let mut bonus = insp.mon.pp_bonuses[i];
                        if widgets::id_combo(
                            ui,
                            ("ppbonus", i),
                            70.0,
                            &mut bonus,
                            (0..4u32).map(|b| (b, format!("+{b}"))),
                        ) {
                            insp.mon.pp_bonuses[i] = bonus;
                            insp.mon.pps[i] = max_pp(insp.mon.moves[i], bonus);
                        }
                    });
                });
            });
            ui.add_space(6.0);
        }
    });
}

/// `Math.floor(basePp * (1 + 0.2 * bonus))`
fn max_pp(move_id: u32, bonus: u32) -> u32 {
    let base = get_move_base_pp(move_id) as f64;
    (base * (1.0 + 0.2 * bonus as f64)).floor() as u32
}

fn stats_column(ui: &mut Ui, insp: &mut super::app::Inspector) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            widgets::heading(ui, "IVs & EVs", theme::AMBER);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if small_color_button(ui, "↩ Undo", theme::BORDER_LIGHT).clicked() {
                    insp.mon.ivs = insp.backup_ivs;
                    insp.mon.evs = insp.backup_evs;
                }
                if small_color_button(ui, "⭐ Max IVs", theme::BLUE).clicked() {
                    insp.mon.ivs = [31; 6];
                }
            });
        });
        ui.add_space(6.0);

        egui::Grid::new("iv-ev-grid")
            .num_columns(3)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                field_label(ui, "Stat");
                field_label(ui, "IV (0-31)");
                field_label(ui, "EV (0-252)");
                ui.end_row();

                for idx in 0..6 {
                    ui.label(RichText::new(STAT_LABELS[idx]).size(12.0));
                    widgets::int_field(ui, &mut insp.mon.ivs[idx], 0, 31, 56.0);
                    widgets::int_field(ui, &mut insp.mon.evs[idx], 0, 252, 56.0);
                    ui.end_row();
                }
            });

        ui.add_space(10.0);
        let ev_total: u32 = insp.mon.evs.iter().sum();
        let color = if ev_total > 510 {
            theme::RED
        } else {
            theme::MUTED
        };
        ui.label(
            RichText::new(format!("EV total: {ev_total} / 510"))
                .size(11.0)
                .color(color),
        );
        ui.label(
            RichText::new(format!(
                "OT: {}  •  Exp: {}  •  Friendship: {}",
                insp.mon.ot_name, insp.mon.experience, insp.mon.friendship
            ))
            .size(11.0)
            .color(theme::DIM),
        );
    });
}

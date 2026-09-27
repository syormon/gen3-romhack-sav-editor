//! The "Edit Trainer & Wallet" modal.

use eframe::egui::{self, RichText};

use crate::engine::charmap::is_encodable;
use crate::engine::layout::{max_coins, max_money, player_name_length};

use super::app::EditorApp;
use super::theme;
use super::widgets::{self, color_button, field_label};

pub fn trainer_modal(app: &mut EditorApp, ctx: &egui::Context) {
    if app.trainer_modal.is_none() {
        return;
    }
    let mut save = false;
    let mut cancel = false;

    let response = egui::Modal::new(egui::Id::new("trainer-modal")).show(ctx, |ui| {
        ui.set_width(420.0);
        let Some(form) = app.trainer_modal.as_mut() else {
            return;
        };

        widgets::heading(ui, "Edit Trainer & Wallet", theme::AMBER);
        ui.add_space(10.0);

        field_label(ui, "Trainer Name");
        ui.add(
            egui::TextEdit::singleline(&mut form.name)
                .desired_width(f32::INFINITY)
                .char_limit(player_name_length()),
        );
        // Only characters the in-game charmap can represent survive a save.
        let unsupported: String = form.name.chars().filter(|c| !is_encodable(*c)).collect();
        if !unsupported.is_empty() {
            ui.label(
                RichText::new(format!(
                    "These characters cannot be stored and will become spaces: {unsupported}"
                ))
                .size(11.0)
                .color(theme::SALMON),
            );
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                field_label(ui, "Gender");
                let mut gender = u32::from(form.gender_code);
                widgets::id_combo(
                    ui,
                    "trainer-gender",
                    100.0,
                    &mut gender,
                    [(0u32, "Boy".to_string()), (1u32, "Girl".to_string())].into_iter(),
                );
                form.gender_code = gender as u8;
            });
            ui.vertical(|ui| {
                field_label(ui, "Money ($)");
                widgets::int_field(ui, &mut form.money, 0, max_money());
            });
            ui.vertical(|ui| {
                field_label(ui, "Coins");
                widgets::int_field(ui, &mut form.coins, 0, max_coins());
            });
        });

        ui.add_space(8.0);
        ui.label(
            RichText::new(format!("Trainer ID {} / Secret ID {}", form.tid, form.sid))
                .size(11.0)
                .color(theme::DIM),
        );

        ui.add_space(14.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if color_button(ui, "Save Changes", theme::GREEN).clicked() {
                save = true;
            }
            if color_button(ui, "Cancel", theme::BORDER).clicked() {
                cancel = true;
            }
        });
    });

    if save {
        if let (Some(form), Some(state)) = (app.trainer_modal.as_ref(), app.save.as_mut()) {
            state.set_trainer_info(form);
        }
        app.trainer_modal = None;
        app.toast("Trainer updated");
    } else if cancel
        || response.backdrop_response.clicked()
        || ctx.input(|i| i.key_pressed(egui::Key::Escape))
    {
        app.trainer_modal = None;
    }
}

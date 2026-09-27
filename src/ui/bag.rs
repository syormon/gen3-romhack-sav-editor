//! The bag modal: one tab per pocket, the items currently carried, and a
//! search-as-you-type list of everything that pocket accepts.

use eframe::egui::{self, RichText};

use crate::engine::layout::{pocket_capacity, pocket_count, pocket_name};
use crate::engine::lookup::{get_item_name, get_items_for_pocket};
use crate::engine::save_parser::BagItem;

use super::app::EditorApp;
use super::theme;
use super::widgets::{self, color_button, small_color_button};

const MAX_QTY: u32 = 999;

pub fn bag_modal(app: &mut EditorApp, ctx: &egui::Context) {
    if app.bag.is_none() {
        return;
    }

    let load_sprites = app.load_sprites;
    let mut save = false;
    let mut cancel = false;
    // Deferred so the closures below do not need `&mut app`.
    let mut pending_add: Option<(u32, u32)> = None;
    let mut pending_remove: Option<usize> = None;
    let mut alert: Option<(String, String)> = None;

    let response = egui::Modal::new(egui::Id::new("bag-modal")).show(ctx, |ui| {
        ui.set_width(840.0);
        ui.set_height(600.0);
        let Some(bag) = app.bag.as_mut() else { return };
        let active = bag.active;
        let capacity = pocket_capacity(active);
        let pocket_label = pocket_name(active);
        let shortcuts: Vec<_> = crate::game::current()
            .behavior()
            .quick_add
            .iter()
            .filter(|q| q.pocket == pocket_label)
            .cloned()
            .collect();

        ui.horizontal(|ui| {
            widgets::heading(ui, "🎒 Player's Bag", theme::AMBER);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "{} / {} slots",
                        bag.items(active).len(),
                        capacity
                    ))
                    .size(12.0)
                    .strong()
                    .color(theme::MUTED),
                );
            });
        });
        widgets::separator(ui);

        // ------------------------------------------------------- pocket tabs
        ui.horizontal_wrapped(|ui| {
            for pocket in 0..pocket_count() {
                let selected = pocket == active;
                let fill = if selected { theme::SKY } else { theme::BORDER };
                if widgets::small_color_button(ui, &pocket_name(pocket), fill).clicked() {
                    bag.active = pocket;
                    bag.search.clear();
                }
            }
        });
        widgets::separator(ui);

        egui::ScrollArea::vertical()
            .id_salt("bag-scroll")
            .show(ui, |ui| {
                let items: Vec<BagItem> = bag.items(active).to_vec();

                // ------------------------------------------- items in the bag
                theme::well_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        widgets::heading(
                            ui,
                            &format!("📦 Items Currently In Bag ({})", items.len()),
                            theme::BLUE,
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("Max capacity: {capacity}"))
                                    .size(11.0)
                                    .color(theme::MUTED),
                            );
                            // Shortcuts this game pack declares for the pocket.
                            for (i, quick) in shortcuts.iter().enumerate() {
                                let fill = if i % 2 == 0 { theme::VIOLET } else { theme::PINK };
                                if small_color_button(ui, &quick.label, fill).clicked() {
                                    pending_add = Some((quick.item, quick.quantity));
                                }
                            }
                        });
                    });
                    ui.add_space(6.0);

                    if items.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(format!(
                                    "Your {} pocket is currently empty. Use the section below to search and add items.",
                                    pocket_label
                                ))
                                .size(12.0)
                                .color(theme::DIM),
                            );
                            ui.add_space(12.0);
                        });
                    }

                    // Key items and mega stones are one-of, so their quantity is fixed.
                    // One-of pockets hold a single copy per slot.
                    let unique = pocket_label == "KeyItems" || pocket_label == "MegaStones";

                    for (idx, item) in items.iter().enumerate() {
                        let name = get_item_name(item.id);
                        egui::Frame::new()
                            .fill(theme::PANEL)
                            .stroke(egui::Stroke::new(1.0, theme::BORDER))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(10, 6))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("#{}", idx + 1))
                                            .size(11.0)
                                            .color(theme::DIM),
                                    );
                                    widgets::item_sprite(ui, item.id, &name, 22.0, load_sprites);
                                    ui.label(RichText::new(&name).size(13.0).strong());

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if small_color_button(ui, "🗑 Remove", theme::RED)
                                                .clicked()
                                            {
                                                pending_remove = Some(idx);
                                            }
                                            if unique {
                                                ui.label(
                                                    RichText::new("1")
                                                        .size(12.0)
                                                        .color(theme::AMBER),
                                                );
                                            } else {
                                                let mut qty =
                                                    bag.items(active)[idx].quantity.max(1);
                                                widgets::int_field(ui, &mut qty, 1, MAX_QTY);
                                                bag.items_mut(active)[idx].quantity = qty;
                                            }
                                            ui.label(
                                                RichText::new("Qty (max 999)")
                                                    .size(11.0)
                                                    .color(theme::MUTED),
                                            );
                                        },
                                    );
                                });
                            });
                        ui.add_space(4.0);
                    }
                });

                ui.add_space(8.0);

                // ------------------------------------------ add available items
                if items.len() < capacity {
                    theme::well_frame().show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        widgets::heading(
                            ui,
                            &format!("🔍 Add Available Item to {}", pocket_label),
                            theme::AMBER,
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut bag.search)
                                    .desired_width(ui.available_width() - 180.0)
                                    .hint_text("Type to search item… (e.g. Master Ball, Potion)"),
                            );
                            ui.label(RichText::new("Qty").size(11.0).color(theme::MUTED));
                            widgets::int_field(ui, &mut bag.add_qty, 1, MAX_QTY);
                        });
                        ui.add_space(6.0);

                        let query = bag.search.trim().to_lowercase();
                        let in_bag: Vec<u32> = items.iter().map(|i| i.id).collect();
                        let available = get_items_for_pocket(active);
                        let matches: Vec<_> = available
                            .iter()
                            .filter(|it| !in_bag.contains(&it.id))
                            .filter(|it| {
                                query.is_empty() || it.name.to_lowercase().contains(&query)
                            })
                            .take(if query.is_empty() { 15 } else { 200 })
                            .collect();

                        if matches.is_empty() {
                            ui.label(
                                RichText::new("No matching available items found")
                                    .size(12.0)
                                    .color(theme::MUTED),
                            );
                        }

                        egui::ScrollArea::vertical()
                            .id_salt("bag-search-results")
                            .max_height(200.0)
                            .show(ui, |ui| {
                                for it in matches {
                                    egui::Frame::new()
                                        .fill(theme::PANEL)
                                        .stroke(egui::Stroke::new(1.0, theme::BORDER))
                                        .corner_radius(egui::CornerRadius::same(6))
                                        .inner_margin(egui::Margin::symmetric(10, 5))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                widgets::item_sprite(
                                                    ui,
                                                    it.id,
                                                    &it.name,
                                                    20.0,
                                                    load_sprites,
                                                );
                                                ui.label(RichText::new(&it.name).size(12.0));
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        if small_color_button(
                                                            ui, "+ Add", theme::SKY,
                                                        )
                                                        .clicked()
                                                        {
                                                            pending_add =
                                                                Some((it.id, bag.add_qty));
                                                        }
                                                    },
                                                );
                                            });
                                        });
                                    ui.add_space(3.0);
                                }
                            });
                    });
                }
            });

        widgets::separator(ui);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if color_button(ui, "Save Bag", theme::GREEN).clicked() {
                save = true;
            }
            if color_button(ui, "Cancel", theme::BORDER).clicked() {
                cancel = true;
            }
        });
    });

    // --------------------------------------------------------- apply changes
    if let Some(idx) = pending_remove
        && let Some(bag) = app.bag.as_mut()
    {
        let active = bag.active;
        let list = bag.items_mut(active);
        if idx < list.len() {
            list.remove(idx);
        }
    }

    if let Some((item_id, qty)) = pending_add
        && let Some(bag) = app.bag.as_mut()
    {
        let active = bag.active;
        let capacity = pocket_capacity(active);

        if bag.exists_anywhere(item_id, active, None) {
            // Already carried: stack it if it is in this pocket, otherwise refuse.
            let existing = bag.items_mut(active).iter_mut().find(|it| it.id == item_id);
            match existing {
                Some(item) => item.quantity = (item.quantity + qty).min(MAX_QTY),
                None => {
                    alert = Some((
                        "Item already in bag".to_string(),
                        format!(
                            "\"{}\" is already present in another pocket of your bag!",
                            get_item_name(item_id)
                        ),
                    ));
                }
            }
        } else if bag.items(active).len() >= capacity {
            alert = Some((
                "Pocket full".to_string(),
                format!("Pocket reached max capacity ({capacity} slots)."),
            ));
        } else {
            bag.items_mut(active).push(BagItem {
                id: item_id,
                quantity: qty.clamp(1, MAX_QTY),
            });
        }
    }

    if let Some((title, body)) = alert {
        app.alert(title, body);
    }

    if save {
        app.commit_bag();
        app.bag = None;
        app.toast("Bag saved");
    } else if cancel
        || response.backdrop_response.clicked()
        || ctx.input(|i| i.key_pressed(egui::Key::Escape))
    {
        app.bag = None;
    }
}

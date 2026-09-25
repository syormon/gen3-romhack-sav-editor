//! Application state and the top-level layout: header, drop zone, trainer bar,
//! party sidebar, box grid, inspector and modals.

use std::sync::mpsc;

use web_time::Instant;

use eframe::egui::{self, RichText};

use crate::engine::layout::*;
use crate::engine::mon_writer::{Rng, clear_pokemon_slot, pack_and_write_pokemon};
use crate::engine::save_parser::{BagItem, GameSave, Pokemon, TrainerInfo};
use crate::engine::save_writer::export_updated_save;

use super::theme;
use super::widgets::{self, color_button};
use super::{bag, inspector, party_box, trainer};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum SlotKind {
    Party,
    Box,
}

/// Identifies one storage slot: a party position, or a slot in a given box.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct SlotRef {
    pub kind: SlotKind,
    pub index: usize,
    pub box_index: usize,
}

impl SlotRef {
    pub fn party(index: usize) -> Self {
        Self {
            kind: SlotKind::Party,
            index,
            box_index: 0,
        }
    }

    pub fn boxed(box_index: usize, index: usize) -> Self {
        Self {
            kind: SlotKind::Box,
            index,
            box_index,
        }
    }

    pub fn is_party(self) -> bool {
        self.kind == SlotKind::Party
    }

    /// Byte offset within the buffer this slot lives in (sb1 or storage).
    pub fn offset(self) -> usize {
        if self.is_party() {
            party_offset() + self.index * mon_size()
        } else {
            storage_boxes_offset() + (self.box_index * box_capacity() + self.index) * box_mon_size()
        }
    }
}

pub struct Toast {
    pub text: String,
    pub born: Instant,
}

/// A blocking message with an OK button, replacing the web build's `alert()`.
pub struct Message {
    pub title: String,
    pub body: String,
}

/// A save file as the editor holds it: a name to show, and the bytes as read.
///
/// The bytes rather than a path, because in a browser a picked or dropped file
/// has no path to go back to. It helps natively too: re-reading a save under a
/// different game no longer depends on the file still being where it was.
#[derive(Clone)]
pub struct SaveFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// A save file waiting on the question of which game wrote it.
///
/// The bytes are kept so the answer can be changed without re-reading the file:
/// every pick re-parses them under that game's layout, and what appears behind
/// the dialog is that parse.
pub struct PendingSave {
    pub name: String,
    pub bytes: Vec<u8>,
    /// Index into `EditorApp::packs`, once something is chosen.
    pub choice: Option<usize>,
    /// Why the chosen game could not read the file.
    pub error: Option<String>,
    /// The save and game that were open before, so Cancel puts them back
    /// instead of throwing away the session.
    pub restore: Option<(SaveFile, std::path::PathBuf)>,
}

/// Working state for the "Add New Pokémon" dialog.
pub struct AddModal {
    pub slot: SlotRef,
    pub query: String,
}

/// Working copy of the Pokémon currently open in the inspector. Edits only
/// reach the save buffer when "Apply Changes" is pressed, as in the web build.
pub struct Inspector {
    pub slot: SlotRef,
    pub mon: Pokemon,
    pub backup_ivs: [u32; 6],
    pub backup_evs: [u32; 6],
    pub species_query: String,
    pub species_picker_open: bool,
    pub move_picker: Option<usize>,
    pub move_query: String,
    pub item_picker_open: bool,
    pub item_query: String,
    pub confirm_release: bool,
}

impl Inspector {
    pub fn new(slot: SlotRef, mon: Pokemon) -> Self {
        Self {
            backup_ivs: mon.ivs,
            backup_evs: mon.evs,
            species_query: crate::engine::lookup::get_species_name(mon.species),
            species_picker_open: false,
            move_picker: None,
            move_query: String::new(),
            item_picker_open: false,
            item_query: String::new(),
            confirm_release: false,
            slot,
            mon,
        }
    }
}

/// Working copy of every bag pocket, committed on "Save Bag".
pub struct BagModal {
    /// One entry per pocket, in the pack's order.
    pub pockets: Vec<Vec<BagItem>>,
    pub active: usize,
    pub search: String,
    pub add_qty: u32,
}

impl BagModal {
    pub fn items(&self, pocket: usize) -> &[BagItem] {
        self.pockets.get(pocket).map_or(&[], Vec::as_slice)
    }

    pub fn items_mut(&mut self, pocket: usize) -> &mut Vec<BagItem> {
        if pocket >= self.pockets.len() {
            self.pockets.resize_with(pocket + 1, Vec::new);
        }
        &mut self.pockets[pocket]
    }

    /// `itemExistsAnywhereInBag`: an item may only live in one pocket.
    pub fn exists_anywhere(
        &self,
        item_id: u32,
        skip_pocket: usize,
        skip_index: Option<usize>,
    ) -> bool {
        self.pockets.iter().enumerate().any(|(p, list)| {
            list.iter().enumerate().any(|(i, it)| {
                let skipped = p == skip_pocket && Some(i) == skip_index;
                !skipped && it.id == item_id
            })
        })
    }
}

/// One user action queued from inside the UI closures and applied afterwards.
pub enum Action {
    Inspect(SlotRef),
    Add(SlotRef),
    Move { src: SlotRef, dst: SlotRef },
}

pub struct EditorApp {
    /// Games found on disk, for the header's picker.
    pub packs: Vec<crate::game::PackInfo>,
    /// Problems reported by the active pack, shown once after loading.
    pub pack_warnings: Vec<String>,
    /// A save file read but not yet parsed, because the game it belongs to is
    /// still being chosen.
    pub pending_save: Option<PendingSave>,
    pub save: Option<GameSave>,
    /// The file `save` was parsed from, kept so it can be re-read under
    /// another game.
    pub loaded: Option<SaveFile>,
    /// Files that finished reading asynchronously — everything in a browser,
    /// where picking and dropping both hand the bytes over later.
    pub incoming: mpsc::Receiver<Result<SaveFile, String>>,
    /// Handed to those reads so they can deliver. Natively every read is
    /// synchronous, so nothing sends on it there.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub incoming_tx: mpsc::Sender<Result<SaveFile, String>>,
    pub current_box: usize,
    pub inspector: Option<Inspector>,
    pub add_modal: Option<AddModal>,
    pub trainer_modal: Option<TrainerInfo>,
    pub bag: Option<BagModal>,
    pub message: Option<Message>,
    pub toasts: Vec<Toast>,
    pub rng: Rng,
    pub load_sprites: bool,
    pub actions: Vec<Action>,
}

impl EditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx);
        let (incoming_tx, incoming) = mpsc::channel();

        Self {
            packs: crate::game::discover(),
            pack_warnings: if crate::game::is_loaded() {
                crate::game::current().warnings()
            } else {
                Vec::new()
            },
            pending_save: None,
            save: None,
            loaded: None,
            incoming,
            incoming_tx,
            current_box: 0,
            inspector: None,
            add_modal: None,
            trainer_modal: None,
            bag: None,
            message: None,
            toasts: Vec::new(),
            rng: Rng::from_clock(),
            load_sprites: true,
            actions: Vec::new(),
        }
    }

    /// Opens a save named on the command line.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_from_disk(&mut self, path: &std::path::Path) {
        match read_disk_file(path) {
            Ok(file) => self.load_file(file),
            Err(err) => self.alert("Could not open file", err),
        }
    }

    /// Loads a game pack and makes it the one the editor reads saves with.
    ///
    /// Everything on screen was parsed with the old game's layout, so it is all
    /// discarded. The file itself is re-read under the new layout, which is the
    /// one-click fix for having picked the wrong game.
    pub fn switch_game(&mut self, ctx: &egui::Context, dir: &std::path::Path) {
        let pack = match crate::game::activate(dir) {
            Ok(pack) => pack,
            Err(err) => {
                self.alert("Could not load that game", err);
                return;
            }
        };

        let reopen = self.loaded.take();
        self.save = None;
        self.inspector = None;
        self.add_modal = None;
        self.bag = None;
        self.trainer_modal = None;
        self.current_box = 0;
        self.pack_warnings = pack.warnings();

        ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title()));
        ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(std::sync::Arc::new(
            crate::load_icon(),
        ))));

        match reopen {
            Some(file) => self.load_file(file),
            None => self.toast(format!("Loaded {}", pack.manifest.name)),
        }
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.toasts.push(Toast {
            text: text.into(),
            born: Instant::now(),
        });
    }

    pub fn alert(&mut self, title: impl Into<String>, body: impl Into<String>) {
        self.message = Some(Message {
            title: title.into(),
            body: body.into(),
        });
    }

    // ---------------------------------------------------------------- loading

    /// Reads a save from disk. Which game it belongs to is asked afterwards, if
    /// it is not already settled — the file has to be in hand before there is
    /// anything to ask about.
    fn load_file(&mut self, file: SaveFile) {
        if crate::game::is_loaded() {
            self.open_save(file);
        } else {
            self.pending_save = Some(PendingSave {
                name: file.name,
                bytes: file.bytes,
                choice: None,
                error: None,
                restore: None,
            });
        }
    }

    /// Opens a save the user picked while the editor was already running, and
    /// asks again which game wrote it.
    ///
    /// A new file is exactly the moment the answer can change — there is
    /// nothing tying the next save to the game the last one came from. The
    /// current game is pre-selected, so staying put is one click, and the
    /// preview behind the dialog shows straight away when it is wrong.
    fn load_file_and_ask(&mut self, file: SaveFile) {
        // With a single game installed there is nothing to ask.
        if self.packs.len() < 2 && crate::game::is_loaded() {
            return self.open_save(file);
        }

        let current = crate::game::current_opt()
            .and_then(|pack| self.packs.iter().position(|p| p.dir == pack.dir));
        let restore = self
            .loaded
            .clone()
            .zip(current.map(|i| self.packs[i].dir.clone()));

        self.pending_save = Some(PendingSave {
            name: file.name,
            bytes: file.bytes,
            choice: None,
            error: None,
            restore,
        });
        if let Some(index) = current {
            self.preview_with_game(index);
        }
    }

    /// Parses a save with the active game's layout and puts it on screen.
    fn open_save(&mut self, file: SaveFile) {
        match GameSave::from_bytes(file.bytes.clone()) {
            Ok(save) => {
                self.save = Some(save);
                self.loaded = Some(file);
                self.current_box = 0;
                self.inspector = None;
                self.add_modal = None;
                self.bag = None;
                self.trainer_modal = None;
                self.toast("Save file loaded");
            }
            Err(err) => self.alert("Could not read that save", err),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn pick_and_load(&mut self, _ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Pokémon save file", &["sav"])
            .pick_file()
        {
            match read_disk_file(&path) {
                Ok(file) => self.load_file_and_ask(file),
                Err(err) => self.alert("Could not open file", err),
            }
        }
    }

    /// The browser can only hand a picked file over asynchronously, so the
    /// bytes arrive later through `incoming`.
    #[cfg(target_arch = "wasm32")]
    fn pick_and_load(&mut self, ctx: &egui::Context) {
        let tx = self.incoming_tx.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let Some(handle) = rfd::AsyncFileDialog::new()
                .add_filter("Pokémon save file", &["sav"])
                .pick_file()
                .await
            else {
                return;
            };
            let file = SaveFile {
                name: handle.file_name(),
                bytes: handle.read().await,
            };
            let _ = tx.send(Ok(file));
            ctx.request_repaint();
        });
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let Some(dropped) = ctx.input(|i| i.raw.dropped_files.first().cloned()) else {
            return;
        };
        let name = dropped
            .path()
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        #[cfg(not(target_arch = "wasm32"))]
        match dropped.bytes() {
            Ok(bytes) => self.load_file_and_ask(SaveFile { name, bytes }),
            Err(err) => self.alert("Could not open file", err),
        }

        #[cfg(target_arch = "wasm32")]
        {
            let tx = self.incoming_tx.clone();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = dropped
                    .bytes_async()
                    .await
                    .map(|bytes| SaveFile { name, bytes });
                let _ = tx.send(result);
                ctx.request_repaint();
            });
        }
    }

    /// Takes in files whose bytes arrived since the last frame.
    fn receive_files(&mut self) {
        while let Ok(result) = self.incoming.try_recv() {
            match result {
                Ok(file) => self.load_file_and_ask(file),
                Err(err) => self.alert("Could not open file", err),
            }
        }
    }

    // ---------------------------------------------------------------- writing

    /// Commit whatever modal is open, mirroring `commitActiveModals()`.
    pub fn commit_active_modals(&mut self) {
        self.commit_inspector();
        if let Some(form) = self.trainer_modal.clone() {
            if let Some(save) = self.save.as_mut() {
                save.set_trainer_info(&form);
            }
        }
        self.commit_bag();
    }

    pub fn commit_inspector(&mut self) {
        let (Some(insp), Some(save)) = (self.inspector.as_ref(), self.save.as_mut()) else {
            return;
        };
        let slot = insp.slot;
        let buffer = if slot.is_party() {
            &mut save.sb1
        } else {
            &mut save.storage
        };
        pack_and_write_pokemon(buffer, slot.offset(), &insp.mon, slot.is_party());
        if slot.is_party() {
            save.refresh_party_count();
        }
    }

    pub fn commit_bag(&mut self) {
        let (Some(bag), Some(save)) = (self.bag.as_ref(), self.save.as_mut()) else {
            return;
        };
        for (pocket, items) in bag.pockets.iter().enumerate() {
            save.set_pocket_items(pocket, items);
        }
    }

    pub fn mon_at(&self, slot: SlotRef) -> Option<Pokemon> {
        let save = self.save.as_ref()?;
        if slot.is_party() {
            save.party().get(slot.index).cloned()
        } else {
            save.box_pokemon(slot.box_index, slot.index)
        }
    }

    /// Port of `swapOrMovePokemon`.
    fn move_mon(&mut self, src: SlotRef, dst: SlotRef) {
        if src == dst {
            return;
        }
        let Some(save) = self.save.as_mut() else {
            return;
        };

        let party_len: usize = save.party().len();
        let src_mon = if src.is_party() {
            save.party().get(src.index).cloned()
        } else {
            save.box_pokemon(src.box_index, src.index)
        };
        let dst_mon = if dst.is_party() {
            save.party().get(dst.index).cloned()
        } else {
            save.box_pokemon(dst.box_index, dst.index)
        };

        let Some(src_mon) = src_mon else { return };

        if src.is_party() && !dst.is_party() && party_len <= 1 && dst_mon.is_none() {
            self.alert(
                "Party cannot be empty",
                "Cannot move your last Pokémon to the storage box!",
            );
            return;
        }
        let save = self.save.as_mut().expect("checked above");

        match dst_mon {
            // Occupied destination: swap the two slots.
            Some(dst_mon) => {
                write_mon(save, dst, &src_mon);
                write_mon(save, src, &dst_mon);
            }
            // Empty destination: move, then close the gap in the party.
            None => {
                write_mon(save, dst, &src_mon);
                let src_buffer = if src.is_party() {
                    &mut save.sb1
                } else {
                    &mut save.storage
                };
                clear_pokemon_slot(src_buffer, src.offset(), src.is_party());

                if src.is_party() {
                    let remaining = save.party();
                    for i in 0..party_size() {
                        let off = party_offset() + i * mon_size();
                        match remaining.get(i) {
                            Some(mon) => pack_and_write_pokemon(&mut save.sb1, off, mon, true),
                            None => clear_pokemon_slot(&mut save.sb1, off, true),
                        }
                    }
                }
            }
        }

        save.refresh_party_count();
        self.inspector = None;
    }

    /// Port of the inspector's Release button.
    pub fn release_mon(&mut self, slot: SlotRef) {
        let Some(party_len) = self.save.as_ref().map(|s| s.party().len()) else {
            return;
        };

        if slot.is_party() && party_len <= 1 {
            self.alert("Party cannot be empty", "Cannot release your last Pokémon!");
            return;
        }
        let save = self.save.as_mut().expect("checked above");

        let buffer = if slot.is_party() {
            &mut save.sb1
        } else {
            &mut save.storage
        };
        clear_pokemon_slot(buffer, slot.offset(), slot.is_party());

        if slot.is_party() {
            // Shift the rest of the party up one slot.
            for i in slot.index..party_size() - 1 {
                let curr_off = party_offset() + i * mon_size();
                let next_off = party_offset() + (i + 1) * mon_size();
                let next: Vec<u8> = save.sb1[next_off..next_off + mon_size()].to_vec();
                save.sb1[curr_off..curr_off + mon_size()].copy_from_slice(&next);
            }
            clear_pokemon_slot(&mut save.sb1, party_offset() + 5 * mon_size(), true);
            save.refresh_party_count();
        }

        self.inspector = None;
    }

    /// Port of `submitAddPokemon`.
    pub fn create_mon(&mut self, slot: SlotRef, species_id: u32) {
        let Some(save) = self.save.as_mut() else {
            return;
        };
        let trainer = save.trainer_info();
        let mon =
            crate::engine::mon_writer::create_default_pokemon(species_id, &trainer, &mut self.rng);
        write_mon(save, slot, &mon);
        if slot.is_party() {
            save.refresh_party_count();
        }
    }

    fn export(&mut self) {
        self.commit_active_modals();
        let Some(save) = self.save.as_ref() else {
            return;
        };

        let updated = export_updated_save(
            &save.buffer,
            save.active_slot,
            &save.sb1,
            &save.sb2,
            &save.storage,
        );

        let file_name = crate::game::current().manifest.export_file_name();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Pokémon save file", &["sav"])
                .set_file_name(file_name)
                .save_file()
            else {
                return;
            };
            match std::fs::write(&path, &updated) {
                Ok(()) => self.toast(format!(
                    "Saved {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )),
                Err(err) => self.alert("Could not write file", err.to_string()),
            }
        }

        // A web page cannot write to disk; it offers the file as a download.
        #[cfg(target_arch = "wasm32")]
        match super::web::download(&file_name, &updated) {
            Ok(()) => self.toast(format!("Downloaded {file_name}")),
            Err(err) => self.alert("Could not download the save", err),
        }
    }

    // ------------------------------------------------------------------- view

    fn header(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(16, 12)),
            )
            .show(ui, |ui| {
                let Some(pack) = crate::game::current_opt() else {
                    ui.label(
                        RichText::new("Pokémon Save Editor")
                            .size(22.0)
                            .strong()
                            .color(theme::AMBER),
                    );
                    return;
                };
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(format!("{} Save Editor", pack.manifest.name))
                                .size(22.0)
                                .strong()
                                .color(theme::AMBER),
                        );
                        ui.horizontal(|ui| {
                            if !pack.manifest.version.is_empty() {
                                ui.label(
                                    RichText::new(format!("v{}", pack.manifest.version))
                                        .size(12.0)
                                        .color(theme::MUTED),
                                );
                            }
                            if let Some(link) = &pack.manifest.link {
                                ui.hyperlink_to(
                                    RichText::new(&link.label).size(12.0).color(theme::SALMON),
                                    &link.url,
                                );
                            }
                        });
                        if !pack.manifest.notice.is_empty() {
                            ui.label(
                                RichText::new(format!("⚠ {}", pack.manifest.notice))
                                    .size(12.0)
                                    .strong()
                                    .color(theme::SALMON),
                            );
                        }
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.packs.len() > 1 {
                            let current_name = crate::game::current().manifest.name.clone();
                            let mut switch_to: Option<std::path::PathBuf> = None;
                            egui::ComboBox::from_id_salt("game-picker")
                                .selected_text(RichText::new(&current_name).size(12.0))
                                .show_ui(ui, |ui| {
                                    for pack in &self.packs {
                                        if ui
                                            .selectable_label(
                                                pack.name == current_name,
                                                pack.label(),
                                            )
                                            .clicked()
                                            && pack.name != current_name
                                        {
                                            switch_to = Some(pack.dir.clone());
                                        }
                                    }
                                });
                            if let Some(dir) = switch_to {
                                let ctx = ui.ctx().clone();
                                self.switch_game(&ctx, &dir);
                            }
                        }
                        if self.save.is_some() {
                            if color_button(ui, "💾 Save & Export .sav", theme::BLUE_DEEP).clicked()
                            {
                                self.export();
                            }
                            if color_button(ui, "📂 Load Different .sav", theme::BORDER).clicked()
                            {
                                self.pick_and_load(&ui.ctx().clone());
                            }
                            if color_button(ui, "Apply Changes", theme::GREEN).clicked() {
                                self.commit_active_modals();
                                self.toast("Changes applied");
                            }
                            ui.checkbox(
                                &mut self.load_sprites,
                                RichText::new("Sprites").size(12.0),
                            )
                            .on_hover_text(
                                "Download Pokémon and item sprites from PokéAPI over the network",
                            );
                        }
                    });
                });
            });
    }

    fn drop_zone(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        ui.allocate_ui(available, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(available.y * 0.25);
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .stroke(egui::Stroke::new(2.0, theme::BORDER_LIGHT))
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(egui::Margin::symmetric(48, 44))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("Drop your .sav file here, or click to browse")
                                    .size(17.0),
                            );
                            ui.add_space(6.0);
                            // Before a game is chosen there is no layout, so
                            // nothing specific can be promised about the file.
                            let hint = match crate::game::current_opt() {
                                Some(pack) => format!(
                                    "Expects a {} KB save for {}",
                                    crate::engine::layout::min_save_size() / 1024,
                                    pack.manifest.title()
                                ),
                                None => "You'll say which game it came from next".to_string(),
                            };
                            ui.label(RichText::new(hint).size(12.0).color(theme::MUTED));
                            ui.add_space(14.0);
                            if color_button(ui, "Browse…", theme::BLUE_DEEP).clicked() {
                                self.pick_and_load(&ui.ctx().clone());
                            }
                        });
                    });
            });
        });
    }

    fn trainer_bar(&mut self, ui: &mut egui::Ui) {
        let Some(trainer) = self.save.as_ref().map(|s| s.trainer_info()) else {
            return;
        };
        let mut open_trainer = false;
        let mut open_bag = false;

        theme::panel_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Trainer:").color(theme::MUTED));
                ui.label(RichText::new(&trainer.name).strong().color(theme::BLUE));
                ui.label(
                    RichText::new(format!("({})", trainer.gender_label()))
                        .size(12.0)
                        .color(theme::MUTED),
                );
                ui.label(RichText::new("•").color(theme::BORDER_LIGHT));
                ui.label(RichText::new("Money:").color(theme::MUTED));
                ui.label(
                    RichText::new(format!("${}", thousands(trainer.money)))
                        .strong()
                        .color(theme::GREEN),
                );
                ui.label(RichText::new("•").color(theme::BORDER_LIGHT));
                ui.label(RichText::new("Coins:").color(theme::MUTED));
                ui.label(
                    RichText::new(thousands(trainer.coins))
                        .strong()
                        .color(theme::AMBER),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    open_trainer = color_button(ui, "✏ Edit Trainer", theme::BORDER).clicked();
                    open_bag = color_button(ui, "🎒 Bag & Items", theme::SKY).clicked();
                });
            });
        });

        if open_trainer {
            self.trainer_modal = Some(trainer);
        }
        if open_bag && pocket_count() == 0 {
            self.alert(
                "No bag data",
                format!(
                    "{} has no bag pockets in its game.json, so there is nothing to edit here.",
                    crate::game::current().manifest.name
                ),
            );
        } else if open_bag {
            if let Some(save) = self.save.as_ref() {
                self.bag = Some(BagModal {
                    pockets: (0..pocket_count()).map(|p| save.pocket_items(p)).collect(),
                    active: 0,
                    search: String::new(),
                    add_qty: 1,
                });
            }
        }
    }

    fn apply_actions(&mut self) {
        for action in std::mem::take(&mut self.actions) {
            match action {
                Action::Inspect(slot) => {
                    if let Some(mon) = self.mon_at(slot) {
                        self.inspector = Some(Inspector::new(slot, mon));
                    }
                }
                Action::Add(slot) => {
                    self.add_modal = Some(AddModal {
                        slot,
                        query: String::new(),
                    });
                }
                Action::Move { src, dst } => self.move_mon(src, dst),
            }
        }
    }

    fn toasts(&mut self, ctx: &egui::Context) {
        const LIFETIME: f32 = 2.5;
        self.toasts
            .retain(|t| t.born.elapsed().as_secs_f32() < LIFETIME + 0.3);
        if self.toasts.is_empty() {
            return;
        }
        ctx.request_repaint();

        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 16.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                for toast in &self.toasts {
                    let age = toast.born.elapsed().as_secs_f32();
                    let alpha = if age > LIFETIME {
                        1.0 - (age - LIFETIME) / 0.3
                    } else {
                        1.0
                    };
                    egui::Frame::new()
                        .fill(theme::GREEN.gamma_multiply(alpha))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(20, 10))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(&toast.text)
                                    .strong()
                                    .color(theme::TEXT.gamma_multiply(alpha)),
                            );
                        });
                }
            });
    }

    fn message_modal(&mut self, ctx: &egui::Context) {
        let Some(message) = self.message.as_ref() else {
            return;
        };
        let title = message.title.clone();
        let body = message.body.clone();
        let mut close = false;

        let response = egui::Modal::new(egui::Id::new("message-modal")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            widgets::heading(ui, &title, theme::AMBER);
            ui.add_space(8.0);
            ui.label(body);
            ui.add_space(14.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if color_button(ui, "OK", theme::BLUE_DEEP).clicked() {
                    close = true;
                }
            });
        });

        if close
            || response.backdrop_response.clicked()
            || ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.message = None;
        }
    }

    /// Asks which game a freshly loaded save belongs to.
    ///
    /// The editor cannot answer this itself. A save file carries no marker of
    /// the game that wrote it, every game here uses the same 128 KB container,
    /// and the wrong layout does not fail — it finds Pokemon at the wrong
    /// offsets and shows convincing rubbish. So the answer is applied live:
    /// pick a game and the save behind this dialog is re-parsed with it, which
    /// is usually enough to see at a glance whether the answer is right.
    fn game_modal(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_save.as_ref() else {
            return;
        };

        let packs = self.packs.clone();
        let file_name = pending.name.clone();
        let choice = pending.choice;
        let error = pending.error.clone();

        let mut pick: Option<usize> = None;
        let mut confirm = false;
        let mut cancel = false;

        // A heavier backdrop than the other dialogs use: what is behind this one
        // is a guess that has not been confirmed yet, and it should read as
        // inert rather than as the editor proper.
        egui::Modal::new(egui::Id::new("game-modal"))
            .backdrop_color(egui::Color32::from_black_alpha(190))
            .show(ctx, |ui| {
                ui.set_width(430.0);
                widgets::heading(ui, "Which game is this save from?", theme::AMBER);
                ui.add_space(8.0);
                ui.label(
                    RichText::new(&file_name)
                        .size(13.0)
                        .strong()
                        .color(theme::SKY),
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Save files carry no marker of the game that wrote them.")
                        .size(12.0)
                        .color(theme::MUTED),
                );
                ui.label(
                    RichText::new("The wrong choice shows convincing nonsense, not an error.")
                        .size(12.0)
                        .color(theme::MUTED),
                );
                ui.add_space(16.0);

                let selected = match choice {
                    Some(i) => packs[i].label(),
                    None => "Select a game…".to_string(),
                };
                egui::ComboBox::from_id_salt("game-modal-combo")
                    .selected_text(RichText::new(selected).size(14.0))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for (i, pack) in packs.iter().enumerate() {
                            if ui
                                .selectable_label(choice == Some(i), pack.label())
                                .clicked()
                            {
                                pick = Some(i);
                            }
                        }
                    });

                ui.add_space(8.0);
                match (&error, choice) {
                    (Some(err), _) => {
                        ui.label(
                            RichText::new(format!("⚠ {err}"))
                                .size(12.0)
                                .color(theme::RED),
                        );
                    }
                    (None, Some(_)) => {
                        ui.label(
                            RichText::new("Opened behind this dialog — does it look right?")
                                .size(12.0)
                                .color(theme::GREEN),
                        );
                    }
                    (None, None) => {
                        ui.label(RichText::new(" ").size(12.0));
                    }
                }

                ui.add_space(16.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Greyed out until a game has actually been chosen, so the
                    // dialog cannot be dismissed without answering it.
                    let ready = choice.is_some() && error.is_none();
                    let fill = if ready { theme::GREEN } else { theme::BORDER };
                    ui.add_enabled_ui(ready, |ui| {
                        if color_button(ui, "Open", fill).clicked() {
                            confirm = true;
                        }
                    });
                    if color_button(ui, "Cancel", theme::BORDER).clicked() {
                        cancel = true;
                    }
                });
            });

        if let Some(index) = pick {
            self.preview_with_game(index);
        } else if confirm {
            let pack = crate::game::current();
            self.pack_warnings = pack.warnings();
            self.pending_save = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title()));
            ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(std::sync::Arc::new(
                crate::load_icon(),
            ))));
            self.toast(format!("Opened as {}", pack.manifest.name));
        } else if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            let restore = self.pending_save.take().and_then(|p| p.restore);
            self.save = None;
            self.loaded = None;
            match restore {
                // Put back the save that was open before this one was picked.
                Some((file, dir)) => {
                    if crate::game::activate(&dir).is_ok() {
                        self.load_file(file);
                    }
                }
                None => crate::game::clear(),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(window_title()));
        }
    }

    /// Re-reads the pending save under `packs[index]`, so the dialog's current
    /// answer is on screen behind it.
    fn preview_with_game(&mut self, index: usize) {
        let Some(pending) = self.pending_save.as_ref() else {
            return;
        };
        let dir = self.packs[index].dir.clone();
        let file = SaveFile {
            name: pending.name.clone(),
            bytes: pending.bytes.clone(),
        };
        let bytes = file.bytes.clone();

        let outcome = crate::game::activate(&dir).and_then(|_| GameSave::from_bytes(bytes));

        self.save = None;
        self.inspector = None;
        self.bag = None;
        self.trainer_modal = None;
        self.add_modal = None;
        self.current_box = 0;
        self.pack_warnings.clear();

        let error = match outcome {
            Ok(save) => {
                self.save = Some(save);
                self.loaded = Some(file);
                None
            }
            Err(err) => Some(err),
        };

        if let Some(pending) = self.pending_save.as_mut() {
            pending.choice = Some(index);
            pending.error = error;
        }
    }

    fn add_modal(&mut self, ctx: &egui::Context) {
        let Some(add) = self.add_modal.as_mut() else {
            return;
        };
        let slot = add.slot;
        let mut query = std::mem::take(&mut add.query);
        let mut create: Option<u32> = None;
        let mut close = false;

        let species = crate::engine::lookup::all_species_list();
        let q = query.trim().to_lowercase();
        let exact = species.iter().find(|s| s.name.to_lowercase() == q);
        let partial = species.iter().find(|s| s.name.to_lowercase().contains(&q));
        let matched = exact.or(partial).filter(|_| !q.is_empty());

        let response = egui::Modal::new(egui::Id::new("add-modal")).show(ctx, |ui| {
            ui.set_width(440.0);
            widgets::heading(ui, "Add New Pokémon", theme::AMBER);
            ui.add_space(10.0);

            let edit = ui.add(
                egui::TextEdit::singleline(&mut query)
                    .desired_width(f32::INFINITY)
                    .hint_text("e.g. Torchic, Bulbasaur…"),
            );
            edit.request_focus();

            ui.add_space(6.0);
            match matched {
                Some(m) => {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Will generate:")
                                .size(12.0)
                                .color(theme::MUTED),
                        );
                        ui.label(
                            RichText::new(&m.name)
                                .size(12.0)
                                .strong()
                                .color(theme::BLUE),
                        );
                    });
                }
                None if q.is_empty() => {
                    ui.label(RichText::new(" ").size(12.0));
                }
                None => {
                    ui.label(
                        RichText::new("No species found")
                            .size(12.0)
                            .color(theme::RED),
                    );
                }
            }

            let submit = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            ui.add_space(14.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if color_button(ui, "Create Pokémon", theme::GREEN).clicked() || submit {
                    if let Some(m) = matched {
                        create = Some(m.id);
                    }
                }
                if color_button(ui, "Cancel", theme::BORDER).clicked() {
                    close = true;
                }
            });
        });

        if let Some(add) = self.add_modal.as_mut() {
            add.query = query;
        }

        if let Some(species_id) = create {
            self.create_mon(slot, species_id);
            self.add_modal = None;
            self.toast("Pokémon created");
        } else if close
            || response.backdrop_response.clicked()
            || ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.add_modal = None;
        }
    }
}

/// The window caption: the active game's name, or the bare product name while
/// the picker is up.
pub fn window_title() -> String {
    match crate::game::current_opt() {
        Some(pack) => format!("{} Save Editor", pack.manifest.name),
        None => "Pokémon Save Editor".to_string(),
    }
}

/// Reads a save from disk into the form the editor holds it in.
#[cfg(not(target_arch = "wasm32"))]
fn read_disk_file(path: &std::path::Path) -> Result<SaveFile, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(SaveFile { name, bytes })
}

/// Writes a Pokémon into the buffer that backs `slot`.
pub fn write_mon(save: &mut GameSave, slot: SlotRef, mon: &Pokemon) {
    let is_party = slot.is_party();
    let buffer = if is_party {
        &mut save.sb1
    } else {
        &mut save.storage
    };
    pack_and_write_pokemon(buffer, slot.offset(), mon, is_party);
}

/// `Number.prototype.toLocaleString()` for the money and coin readouts.
pub fn thousands(value: u32) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

impl eframe::App for EditorApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::BG.to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;

        self.handle_dropped_files(ctx);
        self.receive_files();
        self.header(ui);

        if self.save.is_some() {
            egui::Panel::left("party-panel")
                .exact_size(320.0)
                .resizable(false)
                .frame(
                    egui::Frame::new()
                        .fill(theme::BG)
                        .inner_margin(egui::Margin::symmetric(12, 8)),
                )
                .show(ui, |ui| {
                    party_box::party_panel(self, ui);
                });
        }

        egui::CentralPanel::default_margins()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(ui, |ui| {
                if self.save.is_none() {
                    self.drop_zone(ui);
                    return;
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.trainer_bar(ui);
                    ui.add_space(10.0);
                    party_box::box_panel(self, ui);
                    ui.add_space(14.0);
                    inspector::inspector_panel(self, ui);
                });
            });

        // Held back until the game question is settled, so two dialogs never
        // stack up on top of each other.
        if !self.pack_warnings.is_empty() && self.message.is_none() && self.pending_save.is_none() {
            let warnings = std::mem::take(&mut self.pack_warnings);
            self.alert(
                format!("{} data notes", crate::game::current().manifest.name),
                warnings.join("\n"),
            );
        }

        self.apply_actions();

        trainer::trainer_modal(self, ctx);
        bag::bag_modal(self, ctx);
        self.add_modal(ctx);
        self.game_modal(ctx);
        self.message_modal(ctx);
        self.toasts(ctx);
    }
}

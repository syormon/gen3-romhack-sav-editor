//! A save editor for Pokémon ROM hacks.
//!
//! The editor itself knows nothing about any particular game: each one is a
//! folder under `assets/` holding a `game.json` manifest and its data
//! tables. See `src/game/mod.rs` for the format, and the README for how to add
//! one.
//!
//!     pokemon-save-editor [--game <folder|name>] [save.sav]
//!
//! A save is loaded first, then the editor asks which game wrote it. It has to
//! ask: two games' saves are both 128 KB of otherwise indistinguishable bytes,
//! and reading one with the other's layout produces convincing nonsense rather
//! than an error. `--game` answers ahead of time and skips the question.
//!
//! Always keep a backup before overwriting a save.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[allow(dead_code)]
mod engine;
mod game;
mod ui;

use eframe::egui;

/// Fallback window icon, used before a game is chosen and for packs that ship
/// no `icon.png`.
const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

/// The browser entry point: attach to the page's canvas and run.
///
/// There is no command line and no filesystem here. Game packs come from the
/// copy build.rs compiles in, and saves arrive through the file picker or by
/// dropping them on the page.
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("a browser document");
        let canvas = document
            .get_element_by_id("editor_canvas")
            .expect("index.html should have a canvas with id editor_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("editor_canvas should be a <canvas>");

        let started = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(ui::app::EditorApp::new(cc)))),
            )
            .await;

        // Swap the page's "Loading…" text for the app, or for the reason it
        // could not start.
        if let Some(status) = document.get_element_by_id("loading_text") {
            match started {
                Ok(()) => status.remove(),
                Err(err) => status.set_inner_html(&format!(
                    "<p>The editor could not start.</p><p><code>{err:?}</code></p>"
                )),
            }
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let mut initial_file: Option<std::path::PathBuf> = None;
    let mut wanted_game: Option<String> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--game" | "-g" => wanted_game = args.next(),
            _ => initial_file = Some(std::path::PathBuf::from(arg)),
        }
    }

    let packs = game::discover();
    if packs.is_empty() {
        let searched = game::search_roots()
            .iter()
            .map(|p| format!("  {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n");
        let message = format!(
            "No games found.\n\nEach game lives in its own folder containing {} \
             plus its data tables. Searched:\n\n{searched}",
            game::MANIFEST_NAME
        );
        eprintln!("{message}");
        rfd::MessageDialog::new()
            .set_title("No games installed")
            .set_description(&message)
            .set_level(rfd::MessageLevel::Error)
            .show();
        std::process::exit(1);
    }

    // A game named on the command line, or the only one installed, needs no
    // asking. Otherwise the editor opens on the picker.
    let preselected = match &wanted_game {
        Some(wanted) => match game::find(&packs, wanted) {
            Some(info) => Some(info.dir.clone()),
            None => {
                let known = packs
                    .iter()
                    .map(|p| format!("  {}", p.folder_name()))
                    .collect::<Vec<_>>()
                    .join("\n");
                eprintln!("No game matching \"{wanted}\". Installed:\n{known}");
                std::process::exit(2);
            }
        },
        None if packs.len() == 1 => Some(packs[0].dir.clone()),
        None => None,
    };

    if let Some(dir) = &preselected {
        if let Err(err) = game::activate(dir) {
            eprintln!("{err}");
            rfd::MessageDialog::new()
                .set_title("Could not load game data")
                .set_description(&err)
                .set_level(rfd::MessageLevel::Error)
                .show();
            std::process::exit(1);
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([980.0, 640.0])
            .with_title(ui::app::window_title())
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "pokemon-save-editor",
        options,
        Box::new(move |cc| {
            let mut app = ui::app::EditorApp::new(cc);
            if let Some(path) = initial_file {
                app.load_from_disk(&path);
            }
            Ok(Box::new(app))
        }),
    )
}

/// The active pack's own `icon.png`, or the bundled fallback.
///
/// The window icon is set from this at startup and again whenever the game
/// changes, via `ViewportCommand::Icon`.
pub fn load_icon() -> egui::IconData {
    let pack_icon =
        game::current_opt().and_then(|pack| game::read_pack_file(&pack.dir, "icon.png"));

    if let Some(icon) = pack_icon.as_deref().and_then(decode_icon) {
        return icon;
    }
    decode_icon(ICON_PNG).unwrap_or_else(|| {
        // Only reachable if the bundled PNG stops decoding, which a test
        // guards against. Say so rather than handing over a blank icon: a
        // 1x1 transparent one looks exactly like having no icon at all.
        eprintln!("warning: the bundled icon.png did not decode; the window will have no icon");
        egui::IconData {
            rgba: vec![0; 4],
            width: 1,
            height: 1,
        }
    })
}

/// Decodes PNG bytes into the RGBA form egui wants.
///
/// `None` rather than a placeholder, so a caller cannot mistake a failure for
/// a usable icon. winit also drops an `IconData` whose buffer length does not
/// match its dimensions, so that is checked here instead of being discovered
/// as a missing icon at runtime.
fn decode_icon(bytes: &[u8]) -> Option<egui::IconData> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    let rgba = img.into_raw();
    (width > 0 && height > 0 && rgba.len() == (width as usize) * (height as usize) * 4).then_some(
        egui::IconData {
            rgba,
            width,
            height,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window icon is only as reliable as this file decoding, and a
    /// failure there is invisible at runtime — the window just comes up bare.
    #[test]
    fn the_bundled_icon_decodes_to_something_visible() {
        let icon = decode_icon(ICON_PNG).expect("assets/icon.png must decode");

        assert_eq!(icon.width, icon.height, "icons should be square");
        assert!(icon.width >= 32, "too small to scale down cleanly");
        assert_eq!(
            icon.rgba.len(),
            (icon.width as usize) * (icon.height as usize) * 4,
            "winit silently ignores an icon whose buffer does not match its size"
        );
        assert!(
            icon.rgba.chunks_exact(4).any(|px| px[3] > 0),
            "every pixel is transparent, which is the same as having no icon"
        );
    }

    #[test]
    fn a_file_that_is_not_an_image_is_refused_rather_than_faked() {
        assert!(decode_icon(b"this is not a png").is_none());
        assert!(decode_icon(&[]).is_none());
    }
}

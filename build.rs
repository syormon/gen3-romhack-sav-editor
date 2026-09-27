//! Build-time steps:
//!
//! 1. Compile every game pack under `assets/` into the binary, so the browser
//!    build — which has no filesystem — has games to offer, and so a native
//!    release still works without its `assets` folder.
//! 2. Give the Windows executable an icon.
//!
//! On the icon: `ViewportBuilder::with_icon` covers the *window* — the title
//! bar, Alt-Tab and the taskbar button of a running instance. It cannot cover
//! the .exe itself: Explorer, the Start menu and a pinned shortcut read an icon
//! resource compiled into the binary, and without one they show the generic
//! Windows executable icon. The .ico is generated here from `assets/icon.png`
//! rather than committed, so the PNG stays the single source of truth.

use std::path::{Path, PathBuf};

fn main() {
    // A directory here makes Cargo rescan everything inside it, so adding or
    // editing a pack re-embeds it on the next build.
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));

    embed_packs(
        &manifest_dir.join("assets"),
        &out_dir.join("embedded_packs.rs"),
    );

    // Two different "windows" here. `#[cfg(windows)]` is the machine running
    // this script: the icon tooling is a build-dependency for Windows hosts
    // only, so on Linux and macOS it does not exist to call. The environment
    // variable is the machine the binary is *for*, so a Windows host building
    // for the web does not try to put a Windows resource into a .wasm.
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windows_icon(&manifest_dir.join("assets").join("icon.png"), &out_dir);
    }
}

/// Writes `PACKS: &[(folder, &[(file, bytes)])]`, one entry per folder under
/// `assets/` that holds a `game.json`.
///
/// Only `.json` files are taken, so stray files an editor or the OS leaves
/// behind do not end up inside the binary.
fn embed_packs(assets: &Path, out: &Path) {
    let mut packs: Vec<(String, Vec<(String, PathBuf)>)> = Vec::new();

    let mut folders: Vec<PathBuf> = std::fs::read_dir(assets)
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    folders.sort();

    for dir in folders {
        if !dir.join("game.json").is_file() {
            continue;
        }
        let Some(folder) = dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let mut files: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
            .map(|entries| entries.flatten().map(|e| e.path()).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.is_file())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
            .filter_map(|p| {
                let name = p.file_name()?.to_str()?.to_string();
                Some((name, p))
            })
            .collect();
        files.sort();
        packs.push((folder.to_string(), files));
    }

    let mut code = String::from(
        "/// One pack: its folder under `assets/`, and its files by name.\n\
         pub type Pack = (&'static str, &'static [(&'static str, &'static [u8])]);\n\
         \n\
         /// Every game pack under `assets/`, compiled in by build.rs.\n\
         pub static PACKS: &[Pack] = &[\n",
    );
    for (folder, files) in &packs {
        code.push_str(&format!("    ({folder:?}, &[\n"));
        for (name, path) in files {
            // `{:?}` renders a path as a correctly escaped string literal,
            // backslashes and all.
            code.push_str(&format!(
                "        ({name:?}, include_bytes!({:?})),\n",
                path.display().to_string()
            ));
        }
        code.push_str("    ]),\n");
    }
    code.push_str("];\n");

    std::fs::write(out, code).expect("write embedded_packs.rs");
}

#[cfg(windows)]
fn windows_icon(png: &Path, out_dir: &Path) {
    let ico = out_dir.join("icon.ico");

    if let Err(err) = write_ico(png, &ico) {
        // A missing icon is not worth failing a build over, but it should not
        // pass in silence either.
        println!("cargo:warning=no executable icon: {err}");
        return;
    }

    let mut res = winresource::WindowsResource::new();
    res.set_icon(&ico.to_string_lossy());
    if let Err(err) = res.compile() {
        println!("cargo:warning=could not embed the executable icon: {err}");
    }
}

/// Writes a multi-resolution .ico built from one PNG.
///
/// Windows picks a size per context — 16px in the title bar, 32 in Alt-Tab,
/// 256 for large thumbnails — and scales whatever is nearest if the size it
/// wants is missing, which is what makes a single-size icon look muddy.
#[cfg(windows)]
fn write_ico(png: &Path, ico: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use image::{ExtendedColorType, codecs::ico};

    let source = image::open(png)?.to_rgba8();
    let mut frames = Vec::new();

    for size in [16u32, 24, 32, 48, 64, 128, 256] {
        let scaled = image::imageops::resize(&source, size, size, image::imageops::Lanczos3);
        frames.push(ico::IcoFrame::as_png(
            scaled.as_raw(),
            size,
            size,
            ExtendedColorType::Rgba8,
        )?);
    }

    let file = std::fs::File::create(ico)?;
    ico::IcoEncoder::new(std::io::BufWriter::new(file)).encode_images(&frames)?;
    Ok(())
}

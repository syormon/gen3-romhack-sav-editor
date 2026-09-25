//! Gives the Windows executable an icon.
//!
//! `ViewportBuilder::with_icon` covers the *window* — the title bar, Alt-Tab
//! and the taskbar button of a running instance. It cannot cover the .exe
//! itself: Explorer, the Start menu and a pinned shortcut read an icon
//! resource compiled into the binary, and without one they show the generic
//! Windows executable icon.
//!
//! The .ico is generated here from `assets/icon.png` rather than committed, so
//! the PNG stays the single source of truth for both paths.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.png");
    println!("cargo:rerun-if-changed=build.rs");

    // `cfg!(windows)` here would describe the machine doing the building, not
    // the machine the binary is for.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let ico = out_dir.join("icon.ico");

    if let Err(err) = write_ico("assets/icon.png", &ico) {
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
fn write_ico(png: &str, ico: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
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

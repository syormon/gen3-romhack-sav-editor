//! Browser-only helpers.

use eframe::wasm_bindgen::{JsCast as _, JsValue};

/// Offers `bytes` to the user as a file download named `file_name`.
///
/// A page cannot write to disk, so exporting a save means handing the browser
/// a blob and clicking a temporary link to it — the same thing a "Download"
/// button on any site does.
pub fn download(file_name: &str, bytes: &[u8]) -> Result<(), String> {
    fn js(err: JsValue) -> String {
        err.as_string().unwrap_or_else(|| format!("{err:?}"))
    }

    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/octet-stream");
    let blob =
        web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(js)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js)?;

    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("no document to download from")?;
    let link = document
        .create_element("a")
        .map_err(js)?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "could not create a download link".to_string())?;
    link.set_href(&url);
    link.set_download(file_name);
    link.click();

    // Release the blob, but not straight away: Firefox has cancelled downloads
    // whose URL was revoked in the same tick as the click. Never releasing it
    // would pin 128 KB per export for the life of the page.
    let window = web_sys::window().ok_or("no window")?;
    let release = eframe::wasm_bindgen::closure::Closure::once_into_js(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    window
        .set_timeout_with_callback_and_timeout_and_arguments_0(release.unchecked_ref(), 10_000)
        .map_err(js)?;
    Ok(())
}

//! User-supplied, bundled pixel-art branding; no filesystem or network lookups.
pub fn logo(dark: bool) -> &'static [u8] {
    if dark {
        include_bytes!("../../logo/whiteDraugr128.png")
    } else {
        include_bytes!("../../logo/blackDraugr128.png")
    }
}
pub fn window_icon() -> Result<iced::window::Icon, String> {
    let image = image::load_from_memory(logo(false))
        .map_err(|e| e.to_string())?
        .into_rgba8();
    iced::window::icon::from_rgba(image.into_raw(), 128, 128).map_err(|e| e.to_string())
}
pub fn tray_rgba(color: [u8; 3]) -> Vec<u8> {
    let image = image::load_from_memory(include_bytes!("../../logo/whiteCrown128.png"))
        .expect("bundled crown PNG")
        .into_rgba8();
    let mut rgba =
        image::imageops::resize(&image, 32, 32, image::imageops::FilterType::Nearest).into_raw();
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel[..3].copy_from_slice(&color);
    }
    rgba
}
#[cfg(test)]
#[path = "branding_tests.rs"]
mod tests;

use super::*;
#[test]
fn theme_logos_use_original_assets_with_identical_visible_shapes() {
    let light = image::load_from_memory(logo(false)).unwrap().into_rgba8();
    let dark = image::load_from_memory(logo(true)).unwrap().into_rgba8();
    assert_eq!(light.dimensions(), (128, 128));
    assert_eq!(dark.dimensions(), (128, 128));
    assert!(light.pixels().zip(dark.pixels()).all(|(a, b)| a[3] == b[3]));
    assert!(light.pixels().any(|p| p.0 == [0, 0, 0, 255]));
    assert!(dark.pixels().any(|p| p.0 == [255, 255, 255, 255]));
    assert!(window_icon().is_ok());
}
#[test]
fn tray_preserves_crown_alpha_and_state_colors() {
    let crown = image::load_from_memory(include_bytes!("../../logo/whiteCrown128.png"))
        .unwrap()
        .into_rgba8();
    let mask = image::imageops::resize(&crown, 32, 32, image::imageops::FilterType::Nearest);
    for color in [
        [56, 162, 128],
        [217, 165, 56],
        [208, 83, 83],
        [130, 145, 160],
    ] {
        let rgba = tray_rgba(color);
        assert_eq!(rgba.len(), 32 * 32 * 4);
        assert!(
            rgba.as_chunks::<4>()
                .0
                .iter()
                .zip(mask.pixels())
                .all(|(pixel, source)| pixel[3] == source[3]
                    && (pixel[3] == 0 || pixel[..3] == color))
        );
        assert!(rgba.as_chunks::<4>().0.iter().any(|p| p[3] == 255));
        assert!(rgba.as_chunks::<4>().0.iter().any(|p| p[3] == 0));
    }
}

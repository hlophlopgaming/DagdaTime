pub struct View {
    pub label: &'static str,
    pub color: [u8; 3],
}
pub fn view(status: Option<&serde_json::Value>) -> View {
    match status.and_then(|value| value["state"].as_str()) {
        Some("tracking") => View {
            label: "Tracking",
            color: [25, 165, 135],
        },
        Some("paused") => View {
            label: "Paused",
            color: [222, 162, 38],
        },
        Some("away") => View {
            label: "Away",
            color: [100, 150, 218],
        },
        Some("starting") => View {
            label: "Starting",
            color: [130, 145, 160],
        },
        Some(_) => View {
            label: "Needs attention",
            color: [216, 103, 85],
        },
        None => View {
            label: "Service offline",
            color: [216, 103, 85],
        },
    }
}
pub fn icon(color: [u8; 3]) -> tauri::image::Image<'static> {
    let mut pixels = vec![0u8; 32 * 32 * 4];
    for y in 0..32i32 {
        for x in 0..32i32 {
            if (x - 16).pow(2) + (y - 16).pow(2) <= 11 * 11 {
                let index = (y as usize * 32 + x as usize) * 4;
                pixels[index..index + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
    }
    tauri::image::Image::new_owned(pixels, 32, 32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn tray_reflects_tracking_paused_and_error_status() {
        assert_eq!(view(Some(&json!({"state":"tracking"}))).label, "Tracking");
        assert_eq!(view(Some(&json!({"state":"paused"}))).label, "Paused");
        assert_eq!(
            view(Some(&json!({"state":"error"}))).label,
            "Needs attention"
        );
        assert_eq!(view(None).label, "Service offline");
    }
}

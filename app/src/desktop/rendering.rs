//! Which renderer Iced uses, chosen by the user and applied before Iced starts.
//!
//! Builds with the `gpu` feature contain both wgpu and tiny-skia. Software stays
//! the default: wgpu keeps the graphics driver loaded after the window closes, so
//! the tray-only process holds noticeably more memory (11 → 29 MiB PSS in one
//! Linux measurement). Hardware acceleration is an explicit, restart-applied
//! choice. A user-provided `ICED_BACKEND` always wins over the stored choice.

use std::path::Path;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Renderer {
    #[default]
    Software,
    Hardware,
}

impl Renderer {
    fn code(self) -> &'static str {
        match self {
            Self::Software => "software",
            Self::Hardware => "hardware",
        }
    }
    fn from_code(code: &str) -> Option<Self> {
        [Self::Software, Self::Hardware]
            .into_iter()
            .find(|renderer| renderer.code() == code)
    }
}

pub fn path() -> std::io::Result<std::path::PathBuf> {
    // A per-machine display choice: keep it beside the language, not the history.
    Ok(draugr_time_daemon::paths::default_dir()?.join("renderer.json"))
}

pub fn load(path: &Path) -> std::io::Result<Renderer> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let code: String = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
            Ok(Renderer::from_code(&code).unwrap_or_default())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Renderer::default()),
        Err(error) => Err(error),
    }
}

#[cfg_attr(not(feature = "gpu"), allow(dead_code))]
pub fn save(path: &Path, renderer: Renderer) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec(renderer.code()).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(temp, path)
}

/// The `ICED_BACKEND` value to set, if any. Hardware leaves Iced's own order
/// (wgpu, then tiny-skia when no adapter works); software pins tiny-skia so
/// wgpu never creates an instance or loads a driver.
fn backend_override(renderer: Renderer, user_backend_set: bool) -> Option<&'static str> {
    match (renderer, user_backend_set) {
        (_, true) | (Renderer::Hardware, false) => None,
        (Renderer::Software, false) => Some("tiny-skia"),
    }
}

/// Applies the stored choice. Call once, before Iced starts and before other
/// threads that read the environment are spawned. An unreadable choice falls
/// back to software rendering.
pub fn apply() {
    if !cfg!(feature = "gpu") {
        return; // tiny-skia is the only renderer compiled in.
    }
    let renderer = path().and_then(|path| load(&path)).unwrap_or_default();
    let user_backend_set = std::env::var_os("ICED_BACKEND").is_some();
    if let Some(backend) = backend_override(renderer, user_backend_set) {
        std::env::set_var("ICED_BACKEND", backend);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "draugr-renderer-{name}-{}.json",
            std::process::id()
        ))
    }

    #[test]
    fn missing_or_unknown_choice_means_software() {
        let path = temp_path("missing");
        let _ = std::fs::remove_file(&path);
        assert_eq!(load(&path).unwrap(), Renderer::Software);
        std::fs::write(&path, br#""vulkan""#).unwrap();
        assert_eq!(load(&path).unwrap(), Renderer::Software);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn choice_persists_both_ways() {
        let path = temp_path("persist");
        save(&path, Renderer::Hardware).unwrap();
        assert_eq!(load(&path).unwrap(), Renderer::Hardware);
        save(&path, Renderer::Software).unwrap();
        assert_eq!(load(&path).unwrap(), Renderer::Software);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn corrupt_choice_is_an_error_not_a_silent_switch() {
        let path = temp_path("corrupt");
        std::fs::write(&path, b"{not json").unwrap();
        assert!(load(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn software_pins_tiny_skia_and_hardware_keeps_iced_order() {
        assert_eq!(
            backend_override(Renderer::Software, false),
            Some("tiny-skia")
        );
        assert_eq!(backend_override(Renderer::Hardware, false), None);
    }

    #[test]
    fn explicit_iced_backend_overrides_the_stored_choice() {
        assert_eq!(backend_override(Renderer::Software, true), None);
        assert_eq!(backend_override(Renderer::Hardware, true), None);
    }
}

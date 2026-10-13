#[cfg(any(target_os = "linux", test))]
fn desktop_icon(text: &str, filename: &str, app: &str) -> Option<String> {
    let mut entry = false;
    let mut values = std::collections::HashMap::new();
    for line in text.lines() {
        if line.starts_with('[') {
            entry = line == "[Desktop Entry]";
            continue;
        }
        if entry {
            if let Some((key, value)) = line.split_once('=') {
                values.insert(key.trim(), value.trim());
            }
        }
    }
    let executable = values
        .get("Exec")
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| std::path::Path::new(value).file_name())
        .and_then(|value| value.to_str());
    let identities = [
        filename.strip_suffix(".desktop"),
        values.get("StartupWMClass").copied(),
        values.get("Name").copied(),
        executable,
    ];
    identities
        .into_iter()
        .flatten()
        .any(|value| value.eq_ignore_ascii_case(app))
        .then(|| values.get("Icon").map(|value| (*value).to_owned()))
        .flatten()
}

trait IconProvider {
    fn icon(&self, app: &str) -> Option<String>;
}
struct SystemIcons;
#[cfg(target_os = "linux")]
impl IconProvider for SystemIcons {
    fn icon(&self, app: &str) -> Option<String> {
        use base64::Engine;
        let mut directories = Vec::new();
        if let Some(home) = std::env::var_os("HOME") {
            let home = std::path::PathBuf::from(home);
            directories.push(home.join(".local/share/applications"));
            directories.push(home.join(".local/share/flatpak/exports/share/applications"));
        }
        if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
            directories.insert(0, std::path::PathBuf::from(data).join("applications"));
        }
        for directory in std::env::var("XDG_DATA_DIRS")
            .unwrap_or_else(|_| "/usr/local/share:/usr/share:/var/lib/flatpak/exports/share".into())
            .split(':')
        {
            directories.push(std::path::PathBuf::from(directory).join("applications"));
        }
        let icon = directories
            .into_iter()
            .filter_map(|directory| std::fs::read_dir(directory).ok())
            .flatten()
            .filter_map(Result::ok)
            .find_map(|entry| {
                let filename = entry.file_name();
                let filename = filename.to_str()?;
                if !filename.ends_with(".desktop") {
                    return None;
                }
                desktop_icon(&std::fs::read_to_string(entry.path()).ok()?, filename, app)
            })
            .unwrap_or_else(|| app.to_owned());
        let path = if std::path::Path::new(&icon).is_absolute() {
            std::path::PathBuf::from(icon)
        } else {
            freedesktop_icons::lookup(&icon)
                .with_size(32)
                .with_theme("breeze")
                .with_cache()
                .find()
                .or_else(|| {
                    freedesktop_icons::lookup(&icon)
                        .with_size(32)
                        .with_cache()
                        .find()
                })?
        };
        let mime = match path.extension()?.to_str()? {
            "svg" => "image/svg+xml",
            "png" => "image/png",
            "xpm" => return None,
            _ => return None,
        };
        if std::fs::metadata(&path).ok()?.len() > 2_000_000 {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        Some(format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    }
}
#[cfg(not(target_os = "linux"))]
impl IconProvider for SystemIcons {
    fn icon(&self, _app: &str) -> Option<String> {
        None
    }
}

pub fn application_icons(apps: Vec<String>) -> std::collections::HashMap<String, Option<String>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Option<String>>>,
    > = std::sync::OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    let mut result = std::collections::HashMap::new();
    for app in apps.into_iter().take(200) {
        if app.is_empty() || app.len() > 256 || app.contains(['/', '\\']) {
            continue;
        }
        if cache.len() > 1000 {
            cache.clear();
        }
        let icon = cache
            .entry(app.clone())
            .or_insert_with(|| SystemIcons.icon(&app));
        result.insert(app, icon.clone());
    }
    result
}

#[cfg(all(test, target_os = "linux"))]
mod desktop_tests {
    #[test]
    #[ignore = "requires installed KDE applications and icon themes"]
    fn kde_application_icons_resolve_to_local_images() {
        let icons = super::application_icons(vec!["org.kde.dolphin".into(), "konsole".into()]);
        assert!(icons.values().all(|icon| icon
            .as_ref()
            .is_some_and(|icon| icon.starts_with("data:image/"))));
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn desktop_entry_matches_window_identity_and_ignores_actions() {
        let text = "[Desktop Entry]\nName=Files\nIcon=org.kde.dolphin\nStartupWMClass=Dolphin\nExec=dolphin %u\n[Desktop Action New]\nIcon=wrong";
        assert_eq!(
            super::desktop_icon(text, "org.kde.dolphin.desktop", "DOLPHIN"),
            Some("org.kde.dolphin".into())
        );
        assert_eq!(
            super::desktop_icon(text, "org.kde.dolphin.desktop", "org.kde.dolphin"),
            Some("org.kde.dolphin".into())
        );
        assert_eq!(
            super::desktop_icon(text, "org.kde.dolphin.desktop", "unknown"),
            None
        );
    }
}

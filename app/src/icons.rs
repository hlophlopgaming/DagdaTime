#[cfg(target_os = "linux")]
fn resolve_executable(command: &str) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(command);
    if path.is_absolute() {
        return path.canonicalize().ok();
    }
    if path.components().count() != 1 {
        return None;
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .find_map(|folder| folder.join(command).canonicalize().ok())
}
#[cfg(target_os = "linux")]
fn desktop_values(text: &str) -> std::collections::HashMap<&str, &str> {
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
    values
}
#[cfg(target_os = "linux")]
fn desktop_executable(text: &str) -> Option<std::path::PathBuf> {
    let values = desktop_values(text);
    let tokens = shlex::split(values.get("Exec")?)?;
    let mut arguments = tokens.iter();
    let mut executable = arguments.next()?;
    if std::path::Path::new(executable)
        .file_name()
        .is_some_and(|name| name == "env")
    {
        executable =
            arguments.find(|argument| !argument.contains('=') && !argument.starts_with('-'))?;
    }
    resolve_executable(executable)
}
#[cfg(target_os = "linux")]
fn desktop_icon(text: &str, process: &std::path::Path) -> Option<String> {
    let process = process.canonicalize().ok()?;
    (desktop_executable(text)? == process)
        .then(|| {
            desktop_values(text)
                .get("Icon")
                .map(|value| (*value).to_owned())
        })
        .flatten()
}
#[cfg(target_os = "linux")]
fn application_directories() -> Vec<std::path::PathBuf> {
    let mut directories = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home = std::path::PathBuf::from(home);
        directories.push(home.join(".local/share/applications"));
        directories.push(home.join(".local/share/flatpak/exports/share/applications"));
    }
    if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
        directories.insert(0, std::path::PathBuf::from(data).join("applications"));
    }
    directories.extend(
        std::env::var("XDG_DATA_DIRS")
            .unwrap_or_else(|_| "/usr/local/share:/usr/share:/var/lib/flatpak/exports/share".into())
            .split(':')
            .map(|directory| std::path::PathBuf::from(directory).join("applications")),
    );
    directories
}
#[cfg(target_os = "linux")]
#[allow(dead_code)]
pub fn preview_application_processes() -> Vec<String> {
    let mut candidates = Vec::new();
    for directory in application_directories() {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if entry
                .path()
                .extension()
                .is_none_or(|extension| extension != "desktop")
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            let values = desktop_values(&text);
            if values.get("NoDisplay") == Some(&"true")
                || values.get("Hidden") == Some(&"true")
                || values
                    .get("Type")
                    .is_some_and(|kind| *kind != "Application")
            {
                continue;
            }
            let Some(process) = desktop_executable(&text) else {
                continue;
            };
            let process = process.to_string_lossy().into_owned();
            if candidates.contains(&process) || SystemIcons.icon(&process).is_none() {
                continue;
            }
            candidates.push(process);
            if candidates.len() == 5 {
                return candidates;
            }
        }
    }
    candidates
}
#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn preview_application_processes() -> Vec<String> {
    Vec::new()
}

trait IconProvider {
    fn icon(&self, app: &str) -> Option<String>;
}
struct SystemIcons;
#[cfg(target_os = "linux")]
impl IconProvider for SystemIcons {
    fn icon(&self, app: &str) -> Option<String> {
        use base64::Engine;
        let process = std::path::Path::new(app);
        if !process.is_absolute() {
            return None;
        }
        let icon = application_directories()
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
                desktop_icon(&std::fs::read_to_string(entry.path()).ok()?, process)
            })?;
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
        if app.is_empty() || app.len() > 4096 || app.contains('\0') {
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
mod tests {
    #[test]
    #[ignore = "requires installed Linux applications and icon themes"]
    fn preview_processes_resolve_to_application_icons() {
        let apps = super::preview_application_processes();
        assert!(
            !apps.is_empty(),
            "no installed desktop apps were discovered"
        );
        assert!(apps
            .iter()
            .all(|app| std::path::Path::new(app).is_absolute()));
        let icons = super::application_icons(apps);
        assert!(icons.values().all(Option::is_some));
    }

    #[test]
    fn desktop_icon_matches_exact_process_executable_not_labels() {
        let exe = std::env::current_exe().unwrap();
        let text=format!("[Desktop Entry]\nName=Wrong name\nStartupWMClass=WrongClass\nIcon=correct\nExec=\"{}\" %u\n[Desktop Action New]\nIcon=wrong",exe.display());
        assert_eq!(super::desktop_icon(&text, &exe), Some("correct".into()));
        assert_eq!(
            super::desktop_icon(&text, std::path::Path::new("/missing/other")),
            None
        );
        assert_eq!(
            super::desktop_icon(
                "[Desktop Entry]\nName=editor\nStartupWMClass=editor\nIcon=wrong",
                &exe
            ),
            None
        );
    }
}
#[cfg(all(test, target_os = "linux"))]
mod desktop_tests {
    #[test]
    #[ignore = "requires installed KDE applications and icon themes"]
    fn kde_application_icons_resolve_to_local_images() {
        let apps = ["dolphin", "konsole"].map(|name| {
            super::resolve_executable(name)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        });
        let icons = super::application_icons(apps.into());
        assert!(icons.values().all(|icon| icon
            .as_ref()
            .is_some_and(|icon| icon.starts_with("data:image/"))));
    }
}

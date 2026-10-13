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
fn desktop_icon(text: &str, process: &std::path::Path, app_name: &str) -> Option<(u8, String)> {
    let process = process.canonicalize().ok()?;
    if desktop_executable(text)? != process {
        return None;
    }
    let values = desktop_values(text);
    let icon = values.get("Icon")?.to_string();
    let score = [values.get("Name"), values.get("StartupWMClass")]
        .into_iter()
        .flatten()
        .map(|candidate| name_match_score(app_name, candidate))
        .max()
        .unwrap_or_default();
    Some((score, icon))
}
#[cfg(target_os = "linux")]
fn name_match_score(app_name: &str, candidate: &str) -> u8 {
    let normalize = |value: &str| {
        value
            .chars()
            .filter(|character| character.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let name = normalize(app_name);
    let candidate = normalize(candidate);
    if name.is_empty() || candidate.len() < 3 {
        0
    } else if name == candidate {
        100
    } else if name.starts_with(&candidate) {
        90
    } else if name.contains(&candidate) {
        80
    } else {
        0
    }
}
#[cfg(target_os = "linux")]
fn choose_desktop_icon(
    entries: &[String],
    process: &std::path::Path,
    app_name: &str,
) -> Option<String> {
    let matches: Vec<_> = entries
        .iter()
        .filter_map(|text| desktop_icon(text, process, app_name))
        .collect();
    if matches.len() == 1 {
        return matches.into_iter().next().map(|(_, icon)| icon);
    }
    let best_score = matches.iter().map(|(score, _)| *score).max()?;
    if best_score == 0
        || matches
            .iter()
            .filter(|(score, _)| *score == best_score)
            .count()
            != 1
    {
        return None;
    }
    matches
        .into_iter()
        .find(|(score, _)| *score == best_score)
        .map(|(_, icon)| icon)
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
pub fn preview_application_processes() -> Vec<(String, String)> {
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
            let Some(app_name) = values.get("Name").map(|name| (*name).to_string()) else {
                continue;
            };
            let process = process.to_string_lossy().into_owned();
            let candidate = (process, app_name);
            if candidates.contains(&candidate)
                || SystemIcons.icon(&candidate.0, &candidate.1).is_none()
            {
                continue;
            }
            candidates.push(candidate);
            if candidates.len() == 5 {
                return candidates;
            }
        }
    }
    candidates
}
#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn preview_application_processes() -> Vec<(String, String)> {
    Vec::new()
}

trait IconProvider {
    fn icon(&self, process: &str, app_name: &str) -> Option<String>;
}
struct SystemIcons;
#[cfg(target_os = "linux")]
impl IconProvider for SystemIcons {
    fn icon(&self, app: &str, app_name: &str) -> Option<String> {
        use base64::Engine;
        let process = std::path::Path::new(app);
        if !process.is_absolute() {
            return None;
        }
        let entries: Vec<_> = application_directories()
            .into_iter()
            .filter_map(|directory| std::fs::read_dir(directory).ok())
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let filename = entry.file_name();
                filename
                    .to_str()
                    .filter(|filename| filename.ends_with(".desktop"))?;
                std::fs::read_to_string(entry.path()).ok()
            })
            .collect();
        let icon = choose_desktop_icon(&entries, process, app_name)?;
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
    fn icon(&self, process: &str, app_name: &str) -> Option<String> {
        #[cfg(target_os = "windows")]
        return windows_icon(process, app_name);
        #[cfg(target_os = "macos")]
        return macos_icon(process, app_name);
        #[allow(unreachable_code)]
        let _ = (process, app_name);
        None
    }
}

pub fn application_icons(
    apps: Vec<(String, String)>,
) -> std::collections::HashMap<String, Option<String>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Option<String>>>,
    > = std::sync::OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    let mut result = std::collections::HashMap::new();
    for (process, app_name) in apps.into_iter().take(200) {
        if process.is_empty()
            || process.len() > 4096
            || process.contains('\0')
            || app_name.len() > 1024
        {
            continue;
        }
        if cache.len() > 1000 {
            cache.clear();
        }
        let key = icon_key(&process, &app_name);
        let icon = cache
            .entry(key.clone())
            .or_insert_with(|| SystemIcons.icon(&process, &app_name));
        result.insert(key, icon.clone());
    }
    result
}

#[cfg(target_os = "windows")]
fn windows_icon(process: &str, _app_name: &str) -> Option<String> {
    use std::{os::windows::ffi::OsStrExt, ptr::null_mut};
    use windows_sys::Win32::{
        Graphics::Gdi::{
            CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
            SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC,
            HGDIOBJ,
        },
        UI::{
            Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON},
            WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL, HICON},
        },
    };

    struct Resources {
        icon: HICON,
        screen: HDC,
        dc: HDC,
        bitmap: HBITMAP,
        previous: HGDIOBJ,
    }
    impl Drop for Resources {
        fn drop(&mut self) {
            unsafe {
                if !self.previous.is_null() && !self.dc.is_null() {
                    SelectObject(self.dc, self.previous);
                }
                if !self.bitmap.is_null() {
                    DeleteObject(self.bitmap);
                }
                if !self.dc.is_null() {
                    DeleteDC(self.dc);
                }
                if !self.screen.is_null() {
                    ReleaseDC(null_mut(), self.screen);
                }
                if !self.icon.is_null() {
                    DestroyIcon(self.icon);
                }
            }
        }
    }

    let wide: Vec<u16> = std::ffi::OsStr::new(process)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut info = SHFILEINFOW::default();
    let found = unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if found == 0 || info.hIcon.is_null() {
        return None;
    }
    let mut resources = Resources {
        icon: info.hIcon,
        screen: null_mut(),
        dc: null_mut(),
        bitmap: null_mut(),
        previous: null_mut(),
    };
    let bytes = (|| {
        let screen = unsafe { GetDC(null_mut()) };
        if screen.is_null() {
            return None;
        }
        resources.screen = screen;
        let dc = unsafe { CreateCompatibleDC(screen) };
        if dc.is_null() {
            return None;
        }
        resources.dc = dc;
        let mut bitmap_info = BITMAPINFO::default();
        bitmap_info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: 32,
            biHeight: -32,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 32 * 32 * 4,
            ..Default::default()
        };
        let mut pixels = null_mut();
        resources.bitmap = unsafe {
            CreateDIBSection(dc, &bitmap_info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0)
        };
        if resources.bitmap.is_null() || pixels.is_null() {
            return None;
        }
        resources.previous = unsafe { SelectObject(dc, resources.bitmap) };
        if resources.previous.is_null()
            || unsafe { DrawIconEx(dc, 0, 0, resources.icon, 32, 32, 0, null_mut(), DI_NORMAL) }
                == 0
        {
            return None;
        }
        let bgra = unsafe { std::slice::from_raw_parts(pixels.cast::<u8>(), 32 * 32 * 4) };
        let alpha_present = bgra.chunks_exact(4).any(|pixel| pixel[3] != 0);
        let mut rgba = Vec::with_capacity(bgra.len());
        for pixel in bgra.chunks_exact(4) {
            rgba.extend_from_slice(&[
                pixel[2],
                pixel[1],
                pixel[0],
                if alpha_present { pixel[3] } else { 255 },
            ]);
        }
        Some(rgba)
    })()?;
    encode_png(&bytes, 32, 32)
}

#[cfg(target_os = "macos")]
fn macos_icon(process: &str, _app_name: &str) -> Option<String> {
    use base64::Engine;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSBitmapImageRepPropertyKey, NSWorkspace,
    };
    use objc2_foundation::{NSDictionary, NSString};
    use std::ptr::NonNull;

    let process_path = std::path::Path::new(process);
    let icon_path = process_path
        .ancestors()
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .unwrap_or(process_path);
    let image = NSWorkspace::sharedWorkspace()
        .iconForFile(&NSString::from_str(&icon_path.to_string_lossy()));
    let tiff = image.TIFFRepresentation()?;
    let bitmap = NSBitmapImageRep::imageRepWithData(&tiff)?;
    let properties = NSDictionary::<NSBitmapImageRepPropertyKey, AnyObject>::new();
    let png = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)
    }?;
    let length = png.length();
    if length == 0 || length > 2_000_000 {
        return None;
    }
    let mut bytes = vec![0; length];
    let buffer = NonNull::new(bytes.as_mut_ptr().cast())?;
    unsafe { png.getBytes_length(buffer, length as _) };
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(target_os = "windows")]
fn encode_png(rgba: &[u8], width: u32, height: u32) -> Option<String> {
    use base64::Engine;
    let mut png_bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().ok()?.write_image_data(rgba).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png_bytes)
    ))
}

fn icon_key(process: &str, app_name: &str) -> String {
    format!("{process}\0{app_name}")
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
            .all(|(process, _)| std::path::Path::new(process).is_absolute()));
        let icons = super::application_icons(apps);
        assert!(icons.values().all(Option::is_some));
    }

    #[test]
    fn desktop_icon_matches_exact_process_executable_not_labels() {
        let exe = std::env::current_exe().unwrap();
        let text=format!("[Desktop Entry]\nName=Wrong name\nStartupWMClass=WrongClass\nIcon=correct\nExec=\"{}\" %u\n[Desktop Action New]\nIcon=wrong",exe.display());
        assert_eq!(
            super::desktop_icon(&text, &exe, "WrongClass"),
            Some((100, "correct".into()))
        );
        assert_eq!(
            super::desktop_icon(&text, std::path::Path::new("/missing/other"), "WrongClass"),
            None
        );
        assert_eq!(
            super::desktop_icon(
                "[Desktop Entry]\nName=editor\nStartupWMClass=editor\nIcon=wrong",
                &exe,
                "WrongClass"
            ),
            None
        );
    }

    #[test]
    fn launcher_icons_are_disambiguated_by_application_name() {
        let exe = std::env::current_exe().unwrap();
        let entry = |name: &str, icon: &str| {
            format!(
                "[Desktop Entry]\nName={name}\nStartupWMClass={name}\nIcon={icon}\nExec=\"{}\"\n",
                exe.display()
            )
        };
        assert_eq!(
            super::choose_desktop_icon(
                &[entry("Wrong Game", "wrong"), entry("Right Game", "right")],
                &exe,
                "Right Game",
            ),
            Some("right".into())
        );
        assert_eq!(
            super::choose_desktop_icon(
                &[entry("Wrong Game", "wrong"), entry("Right Game", "right")],
                &exe,
                "Unknown Window",
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
        let apps = ["Dolphin", "Konsole"].map(|name| {
            (
                super::resolve_executable(name.to_lowercase().as_str())
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                name.into(),
            )
        });
        let icons = super::application_icons(apps.into());
        assert!(icons.values().all(|icon| icon
            .as_ref()
            .is_some_and(|icon| icon.starts_with("data:image/"))));
    }
}

use std::{fs, io, path::PathBuf};

pub const PORT: u16 = 56123;

pub fn default_dir() -> io::Result<PathBuf> {
    dirs::data_local_dir()
        .map(|path| application_dir(&path))
        .ok_or_else(|| io::Error::other("cannot find local application data directory"))
}

fn application_dir(root: &std::path::Path) -> PathBuf {
    let current = root.join("DraugrTime");
    let legacy = root.join("DagdaTime");
    if !current.exists() && legacy.is_dir() {
        legacy
    } else {
        current
    }
}

pub fn read_choice(config: &std::path::Path) -> io::Result<Option<PathBuf>> {
    match fs::read(config) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn save_choice(config: &std::path::Path, selected: &std::path::Path) -> io::Result<()> {
    if !selected.is_absolute() {
        return Err(io::Error::other("Choose an absolute folder path"));
    }
    fs::create_dir_all(selected)?;
    fs::create_dir_all(
        config
            .parent()
            .ok_or_else(|| io::Error::other("invalid settings path"))?,
    )?;
    fs::write(
        config,
        serde_json::to_vec(selected).map_err(io::Error::other)?,
    )
}

pub fn data_dir() -> io::Result<PathBuf> {
    static ACTIVE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    if let Some(path) = ACTIVE.get() {
        return Ok(path.clone());
    }
    let path = if let Some(path) =
        std::env::var_os("DRAUGR_TIME_DATA_DIR").or_else(|| std::env::var_os("DAGDATIME_DATA_DIR"))
    {
        PathBuf::from(path)
    } else {
        let default = default_dir()?;
        read_choice(&default.join("preferences.json"))?.unwrap_or(default)
    };
    Ok(ACTIVE.get_or_init(|| path).clone())
}

pub fn token(path: &std::path::Path) -> io::Result<String> {
    if let Ok(token) = fs::read_to_string(path) {
        if token.trim().len() >= 32 {
            return Ok(token.trim().to_owned());
        }
        return Err(io::Error::other("invalid local API token file"));
    }
    let token = uuid::Uuid::new_v4().to_string();
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    options.open(path)?.write_all(token.as_bytes())?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renamed_app_keeps_legacy_history_and_uses_new_directory_for_new_installs() {
        let root = std::env::temp_dir().join(format!(
            "draugr-time-rename-{}-{}",
            std::process::id(),
            crate::now_ms()
        ));
        fs::create_dir_all(&root).unwrap();
        assert_eq!(application_dir(&root), root.join("DraugrTime"));
        let legacy = root.join("DagdaTime");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("activity.sqlite"), b"existing history").unwrap();
        assert_eq!(application_dir(&root), legacy);
        assert_eq!(
            fs::read(legacy.join("activity.sqlite")).unwrap(),
            b"existing history"
        );
        fs::create_dir_all(root.join("DraugrTime")).unwrap();
        assert_eq!(application_dir(&root), root.join("DraugrTime"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn directory_choice_persists_without_moving_history() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        let config = root.join("preferences.json");
        let selected = root.join("chosen");
        assert_eq!(read_choice(&config).unwrap(), None);
        save_choice(&config, &selected).unwrap();
        assert_eq!(read_choice(&config).unwrap(), Some(selected));
        assert!(!root.join("activity.sqlite").exists());
        assert!(save_choice(&config, std::path::Path::new("relative")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}

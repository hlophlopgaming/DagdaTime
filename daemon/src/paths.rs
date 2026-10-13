use std::{fs, io, path::PathBuf};

pub const PORT: u16 = 56123;

pub fn data_dir() -> io::Result<PathBuf> {
    if let Some(path) = std::env::var_os("DAGDATIME_DATA_DIR") {
        return Ok(path.into());
    }
    dirs::data_local_dir()
        .map(|path| path.join("DagdaTime"))
        .ok_or_else(|| io::Error::other("cannot find local application data directory"))
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

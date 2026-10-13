use serde_json::Value;
use std::{
    sync::{Mutex, OnceLock},
    time::Duration,
};

pub fn allowed(path: &str, method: &str) -> bool {
    match method {
        "GET" => {
            ["/status", "/tags", "/tag_ranges"].contains(&path) || path.starts_with("/events?")
        }
        "POST" => ["/pause", "/resume"].contains(&path),
        _ => false,
    }
}
/// One client for the whole process: building a blocking reqwest client starts a
/// thread with its own async runtime, which used to happen on every request.
fn client() -> Result<&'static reqwest::blocking::Client, String> {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_millis(500))
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|error| error.to_string())?;
    Ok(CLIENT.get_or_init(|| client))
}

/// The token file is read once and re-read only after the service rejects it.
fn token(refresh: bool) -> Result<String, String> {
    static TOKEN: Mutex<Option<String>> = Mutex::new(None);
    let mut cached = TOKEN.lock().unwrap();
    if refresh || cached.is_none() {
        let directory = dagdatime_daemon::paths::data_dir().map_err(|error| error.to_string())?;
        let token = std::fs::read_to_string(directory.join("api-token")).map_err(|_| {
            "The background service is not running. Start tracking to continue.".to_string()
        })?;
        *cached = Some(token.trim().to_owned());
    }
    Ok(cached.clone().unwrap_or_default())
}

pub fn request(path: &str, method: &str, body: &Value) -> Result<Value, String> {
    let url = format!("http://127.0.0.1:{}{path}", dagdatime_daemon::paths::PORT);
    let method =
        reqwest::Method::from_bytes(method.as_bytes()).map_err(|error| error.to_string())?;
    let mut refresh = false;
    loop {
        let response = client()?
            .request(method.clone(), &url)
            .bearer_auth(token(refresh)?)
            .json(body)
            .send()
            .map_err(|_| {
                "The background service is unavailable. Start tracking or check permissions."
                    .to_string()
            })?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED && !refresh {
            refresh = true;
            continue;
        }
        let value: Value = response.json().map_err(|error| error.to_string())?;
        if !status.is_success() {
            return Err(value["error"]
                .as_str()
                .unwrap_or("Background service request failed")
                .into());
        }
        return Ok(value);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frontend_cannot_access_arbitrary_routes_or_shutdown() {
        assert!(allowed("/status", "GET"));
        assert!(allowed("/events?from=0&to=1", "GET"));
        assert!(allowed("/pause", "POST"));
        for path in ["https://example.com", "/shutdown", "/../../secret"] {
            assert!(!allowed(path, "POST"));
        }
        assert!(!allowed("/tags", "DELETE"));
    }
}

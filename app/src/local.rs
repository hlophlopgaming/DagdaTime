use serde_json::Value;
use std::time::Duration;

pub fn allowed(path: &str, method: &str) -> bool {
    match method {
        "GET" => {
            ["/status", "/tags", "/tag_ranges"].contains(&path) || path.starts_with("/events?")
        }
        "POST" => ["/pause", "/resume"].contains(&path),
        _ => false,
    }
}
pub fn request(path: &str, method: &str, body: &Value) -> Result<Value, String> {
    let directory = dagdatime_daemon::paths::data_dir().map_err(|error| error.to_string())?;
    let token = std::fs::read_to_string(directory.join("api-token")).map_err(|_| {
        "The background service is not running. Start tracking to continue.".to_string()
    })?;
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_millis(500))
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|error| error.to_string())?;
    let url = format!("http://127.0.0.1:{}{path}", dagdatime_daemon::paths::PORT);
    let method =
        reqwest::Method::from_bytes(method.as_bytes()).map_err(|error| error.to_string())?;
    let response = client
        .request(method, url)
        .bearer_auth(token.trim())
        .json(body)
        .send()
        .map_err(|_| {
            "The background service is unavailable. Start tracking or check permissions."
                .to_string()
        })?;
    let status = response.status();
    let value: Value = response.json().map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(value["error"]
            .as_str()
            .unwrap_or("Background service request failed")
            .into());
    }
    Ok(value)
}
pub fn start() -> Result<(), String> {
    if request("/status", "GET", &Value::Null).is_ok() {
        return Ok(());
    }
    let mut executable = std::env::current_exe().map_err(|error| error.to_string())?;
    executable.set_file_name(if cfg!(windows) {
        "tracker.exe"
    } else {
        "tracker"
    });
    std::process::Command::new(&executable)
        .arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| {
            "The background service executable is missing or cannot start. Reinstall DagdaTime."
                .to_string()
        })?;
    for _ in 0..20 {
        if request("/status", "GET", &Value::Null).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("The background service could not start. Check file permissions and whether another instance is running.".into())
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

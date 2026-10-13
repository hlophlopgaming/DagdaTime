use std::process::Command;

#[test]
fn help_does_not_require_a_desktop() {
    let output = Command::new(env!("CARGO_BIN_EXE_tracker"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("tracker debug"));
}

#[test]
fn unknown_command_exits_with_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_tracker"))
        .arg("unknown")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("Usage: tracker debug"));
}

#[cfg(target_os = "linux")]
#[test]
fn wayland_debug_exits_with_explicit_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_tracker"))
        .arg("debug")
        .env("XDG_SESSION_TYPE", "wayland")
        .env("WAYLAND_DISPLAY", "wayland-0")
        .env("XDG_CURRENT_DESKTOP", "UnknownCompositor")
        .env_remove("SWAYSOCK")
        .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("Wayland"));
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a dedicated X11 display (CI uses Xvfb)"]
fn debug_cli_emits_one_sample_per_second() {
    use std::{
        io::{BufRead, BufReader},
        process::{Child, Stdio},
        time::Instant,
    };
    struct StopOnDrop(Child);
    impl Drop for StopOnDrop {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = StopOnDrop(
        Command::new(env!("CARGO_BIN_EXE_tracker"))
            .arg("debug")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut output = BufReader::new(child.0.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let first = Instant::now();
    let sample: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert!(sample["idle_seconds"].is_u64());
    assert_eq!(sample["errors"], serde_json::json!([]));
    line.clear();
    output.read_line(&mut line).unwrap();
    let elapsed = first.elapsed().as_secs_f64();
    assert!(
        (0.8..3.0).contains(&elapsed),
        "polling interval was {elapsed} seconds"
    );
    let sample: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert!(sample["idle_seconds"].is_u64());
}

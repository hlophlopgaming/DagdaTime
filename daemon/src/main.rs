use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

use dagdatime_watchers::{IdleWatcher, NativeWatcher, WindowWatcher};

fn main() {
    if let Err(error) = run() {
        eprintln!("tracker: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments == ["--help"] || arguments == ["-h"] {
        println!("Usage: tracker debug\nPrint current window and idle seconds every second; Ctrl+C stops.");
        return Ok(());
    }
    if arguments != ["debug"] {
        return Err("Usage: tracker debug".into());
    }
    let watcher = NativeWatcher::new()?;
    let mut output = io::stdout().lock();
    loop {
        let start = Instant::now();
        if let Err(error) = debug_tick(&watcher, &watcher, &mut output) {
            if error.kind() == io::ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(error.into());
        }
        std::thread::sleep(Duration::from_secs(1).saturating_sub(start.elapsed()));
    }
}

fn debug_tick(
    window: &impl WindowWatcher,
    idle: &impl IdleWatcher,
    output: &mut impl Write,
) -> io::Result<()> {
    let mut errors = Vec::new();
    let window = window.active_window().unwrap_or_else(|error| {
        errors.push(format!("window: {error}"));
        None
    });
    let idle_seconds = match idle.idle_seconds() {
        Ok(seconds) => Some(seconds),
        Err(error) => {
            errors.push(format!("idle: {error}"));
            None
        }
    };
    serde_json::to_writer(
        &mut *output,
        &serde_json::json!({
            "window": window, "idle_seconds": idle_seconds, "errors": errors,
        }),
    )
    .map_err(io::Error::from)?;
    writeln!(output)?;
    output.flush()
}

#[cfg(test)]
#[path = "debug_tests.rs"]
mod tests;

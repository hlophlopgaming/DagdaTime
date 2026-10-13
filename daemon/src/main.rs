use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

use draugr_time_watchers::{IdleWatcher, NativeWatcher, WindowWatcher};

fn main() {
    if let Err(error) = run() {
        eprintln!("tracker: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments == ["--help"] || arguments == ["-h"] {
        println!("Usage: tracker debug | serve [--idle-threshold SECONDS]\nDebug prints window and idle each second. Serve records activity and exposes the authenticated localhost API.");
        return Ok(());
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "serve")
    {
        let threshold = match arguments.as_slice() {
            [_] => 300,
            [_, flag, value] if flag == "--idle-threshold" => value.parse::<u64>()?,
            _ => return Err("Usage: tracker serve [--idle-threshold SECONDS]".into()),
        };
        if threshold == 0 || threshold > 86_400 {
            return Err("idle threshold must be 1..86400 seconds".into());
        }
        return draugr_time_daemon::service::serve(threshold);
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

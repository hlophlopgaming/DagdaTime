#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Native Draugr Time desktop application.
//!
//! On Linux and macOS the same executable also runs the console commands
//! (`draugr-time report ...`). Windows keeps the separate `draugr-time-cli.exe`
//! because the release GUI binary has no console attached.
mod branding;
#[cfg(not(windows))]
mod cli;
mod desktop;
mod icons;
fn main() -> iced::Result {
    #[cfg(not(windows))]
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if cli::is_invocation(&args) {
            std::process::exit(cli::main(args));
        }
    }
    desktop::run()
}

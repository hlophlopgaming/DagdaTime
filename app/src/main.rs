#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Native Draugr Time desktop application.
mod desktop;
mod icons;
fn main() -> iced::Result {
    desktop::run()
}

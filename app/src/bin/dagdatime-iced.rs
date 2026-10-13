#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Experimental Iced frontend; main keeps the Tauri implementation until accepted.
#[path = "../icons.rs"]
mod icons;
#[path = "../iced_prototype/mod.rs"]
mod prototype;
fn main() -> iced::Result {
    prototype::run()
}

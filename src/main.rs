#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#![allow(clippy::missing_errors_doc)]

mod app;
mod config;
mod native;
mod palette;
mod system;
mod ui;
mod widgets;

#[cfg(target_os = "windows")]
fn main() -> iced::Result {
    app::run()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("This project currently targets Windows only.");
}

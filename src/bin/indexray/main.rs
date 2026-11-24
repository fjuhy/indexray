#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("IndexRay native desktop app requires Windows.");
    std::process::exit(1);
}

#[cfg(windows)]
mod logging;
#[cfg(windows)]
mod config;
#[cfg(windows)]
mod engine;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod settings_dlg;
#[cfg(windows)]
#[path = "../profile/mod.rs"]
mod profile;

#[cfg(windows)]
fn main() {
    if let Err(err) = profile::index_dir() {
        use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
        use windows::core::{PCWSTR, w};
        let message: Vec<_> = format!("{err:#}").encode_utf16().chain([0]).collect();
        unsafe {
            MessageBoxW(None, PCWSTR(message.as_ptr()), w!("IndexRay"), MB_OK | MB_ICONERROR);
        }
        return;
    }
    logging::init();
    dowse::use_directory_scans();
    if let Err(err) = ui::run() {
        logging::log_line("fatal", &format!("{err:#}"));
    }
}

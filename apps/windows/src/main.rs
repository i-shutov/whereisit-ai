#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(windows)]
mod native;

#[cfg(windows)]
fn main() {
    native::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Atlas Windows requires Windows 10 or newer. Use atlas-cli on this platform.");
}

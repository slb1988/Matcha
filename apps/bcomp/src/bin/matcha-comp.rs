//! `matcha-comp` – GUI launcher that returns immediately (reference: `BComp.exe`).
//!
//! Windows GUI subsystem: VCS tools and shortcuts start it without flashing a console;
//! callers still see the exit code (e.g. 103 when `matcha` is missing).
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    bcomp::launch(bcomp::LaunchMode::ReturnImmediately, args).into()
}

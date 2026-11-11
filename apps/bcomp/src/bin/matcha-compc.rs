//! `matcha-compc` – console launcher that waits for the comparison and returns its exit
//! code (reference: `BComp.com`). Requires an attached console.
//!
//! Console subsystem. Distributions ship it as `matcha.com` next to `matcha.exe`
//! (`cargo xtask dist`), so `matcha …` typed in cmd/PowerShell resolves to it via `PATHEXT`
//! while double-clicking `matcha.exe` opens the GUI without a console.

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    bcomp::launch(bcomp::LaunchMode::WaitForExitCode, args).into()
}

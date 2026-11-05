//! `matcha-compc` – console launcher that waits for the comparison and returns its exit
//! code (reference: `BComp.com`). Requires an attached console.

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    bcomp::launch(bcomp::LaunchMode::WaitForExitCode, args).into()
}

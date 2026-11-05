//! `matcha-comp` – GUI launcher that returns immediately (reference: `BComp.exe`).

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    bcomp::launch(bcomp::LaunchMode::ReturnImmediately, args).into()
}

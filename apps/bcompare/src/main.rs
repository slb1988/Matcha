//! `matcha` – main GUI executable (reference: the single-instance main program).
//!
//! Minimal argument handling for text comparison and the
//! scroll benchmark. The real command line goes through `bc_script::cli::CliArgs::parse`
//! (switch set + positional inference) and single-instance hand-off
//! (`bc_script::cli::Switch::Solo` opts out) once those are implemented.
//!
//! ```text
//! matcha [<left> <right>] [--bench[=<frames>]]
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut options = bc_gui::LaunchOptions::default();
    let mut positional: Vec<PathBuf> = Vec::new();

    for arg in std::env::args().skip(1) {
        if arg == "--bench-dir" {
            options.bench_dir = true;
        } else if arg == "--bench-table" {
            options.bench_table = true;
            options.bench = Some(bc_gui::bench::BenchOptions::default());
        } else if let Some(rest) = arg.strip_prefix("--bench") {
            let mut bench = bc_gui::bench::BenchOptions::default();
            if let Some(n) = rest.strip_prefix('=') {
                match n.parse::<usize>() {
                    Ok(frames) => bench.frames = frames,
                    Err(_) => {
                        eprintln!("--bench expects a frame count, got {n:?}");
                        return ExitCode::from(bc_script::ExitCode::UnknownError.code() as u8);
                    }
                }
            }
            options.bench = Some(bench);
        } else if let Some(path) = arg.strip_prefix("--screenshot=") {
            options.screenshot = Some(PathBuf::from(path));
        } else if arg == "--help" || arg == "/?" || arg == "-h" {
            println!("usage: matcha [<left> <right>] [--bench[=<frames>]] [--bench-table] [--screenshot=<file.bmp>]");
            println!("       matcha --bench-dir <left-folder> <right-folder>");
            println!("(full switch set: see bc-script::cli; not wired yet)");
            return ExitCode::SUCCESS;
        } else {
            positional.push(PathBuf::from(arg));
        }
    }

    if positional.len() >= 2 {
        options.left = Some(positional.remove(0));
        options.right = Some(positional.remove(0));
    }

    if options.bench_dir && !matches!((&options.left,&options.right),(Some(l),Some(r)) if l.is_dir()&&r.is_dir()) {
        eprintln!("--bench-dir requires two readable folders");
        return ExitCode::FAILURE;
    }
    match bc_gui::run(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("gui error: {e}");
            ExitCode::from(bc_script::ExitCode::UnknownError.code() as u8)
        }
    }
}

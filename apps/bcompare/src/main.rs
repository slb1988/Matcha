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
    bc_gui::perf::t0();
    bc_gui::perf::mark("main:enter");
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
        } else if let Some(scene) = arg.strip_prefix("--snapshot=") {
            if !bc_gui::snapshot::is_scene(Some(scene)) {
                eprintln!("unknown snapshot scene: {scene}");
                return ExitCode::FAILURE;
            }
            options.snapshot = Some(scene.to_string());
        } else if let Some(path) = arg.strip_prefix("--snapshot-out=") {
            if path.is_empty() {
                eprintln!("--snapshot-out requires a PNG path");
                return ExitCode::FAILURE;
            }
            options.snapshot_out = Some(PathBuf::from(path));
        } else if let Some(size) = arg.strip_prefix("--window-size=") {
            match parse_window_size(size) {
                Ok(size) => options.window_size = Some(size),
                Err(message) => {
                    eprintln!("{message}");
                    return ExitCode::FAILURE;
                }
            }
        } else if let Some(path) = arg.strip_prefix("--screenshot=") {
            options.screenshot = Some(PathBuf::from(path));
        } else if arg == "--help" || arg == "/?" || arg == "-h" {
            println!("usage: matcha [<left> <right>] [--bench[=<frames>]] [--bench-table] [--screenshot=<file.bmp>]");
            println!("       matcha --bench-dir <left-folder> <right-folder>");
            println!(
                "       matcha --snapshot=<scene> [--snapshot-out=<file.png>] [--window-size=WxH]"
            );
            println!("scenes: {}", bc_gui::snapshot::SCENES.join(", "));
            println!("(full switch set: see bc-script::cli; not wired yet)");
            return ExitCode::SUCCESS;
        } else {
            positional.push(PathBuf::from(arg));
        }
    }

    if options.snapshot_out.is_some() && options.snapshot.is_none() {
        eprintln!("--snapshot-out requires --snapshot=<scene>");
        return ExitCode::FAILURE;
    }
    if positional.len() >= 2 {
        options.left = Some(positional.remove(0));
        options.right = Some(positional.remove(0));
    }

    if options.bench_dir
        && !matches!((&options.left,&options.right),(Some(l),Some(r)) if l.is_dir()&&r.is_dir())
    {
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

/// 客户区物理像素，拒绝非有限、零、溢出及不完整的尺寸。
fn parse_window_size(value: &str) -> Result<[f32; 2], String> {
    let invalid = || format!("--window-size expects WxH (64..=8192), got {value:?}");
    let (w, h) = value.split_once('x').ok_or_else(invalid)?;
    let w = w.parse::<u32>().map_err(|_| invalid())?;
    let h = h.parse::<u32>().map_err(|_| invalid())?;
    if !(64..=8192).contains(&w) || !(64..=8192).contains(&h) {
        return Err(invalid());
    }
    Ok([w as f32, h as f32])
}
#[cfg(test)]
mod tests {
    #[test]
    fn window_size_validates_both_client_dimensions() {
        assert_eq!(
            super::parse_window_size("1384x961").unwrap(),
            [1384.0, 961.0]
        );
        for bad in ["0x961", "-1x20", "1xNaN", "100", "100x100x100", "99999x100"] {
            assert!(super::parse_window_size(bad).is_err());
        }
    }
}

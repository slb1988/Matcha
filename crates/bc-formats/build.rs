//! Build script: converts `data/bcfileformats.xml` (the `BCFILEFORMATS` RCDATA resource
//! extracted from the reference binary) into JSON that the library embeds and
//! deserialises lazily. The model and the XML mapping are shared with the library via
//! `#[path]` includes so there is exactly one definition of each.

#[path = "src/model.rs"]
#[allow(dead_code)]
mod model;

#[path = "src/bcff.rs"]
#[allow(dead_code, unused_variables)]
mod bcff;

use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let src = Path::new(&manifest_dir).join("data").join("bcfileformats.xml");
    println!("cargo:rerun-if-changed={}", src.display());
    println!("cargo:rerun-if-changed=src/model.rs");
    println!("cargo:rerun-if-changed=src/bcff.rs");

    let xml = std::fs::read_to_string(&src)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", src.display()));
    let formats = bcff::parse_bcfileformats(&xml)
        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", src.display()));
    let json = serde_json::to_string(&formats).expect("serialise built-in formats");

    let dest = Path::new(&out_dir).join("builtin_formats.json");
    std::fs::write(&dest, json).unwrap_or_else(|e| panic!("cannot write {}: {e}", dest.display()));
}

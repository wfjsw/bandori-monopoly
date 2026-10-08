//! Generates the 上学时间 school table from `data/schools.json`, so the event
//! reads the data file instead of a hand-copied list. `src/school_time.rs`
//! pulls the output in with `include!`.

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let path = manifest.join("../../data/schools.json");
    println!("cargo:rerun-if-changed={}", path.display());

    let text = std::fs::read_to_string(&path).expect("read data/schools.json");
    let json: serde_json::Value =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).expect("parse data/schools.json");
    let fallback = json["fallback"].as_str().expect("schools.json: `fallback`");
    let rows = json["schools"].as_array().expect("schools.json: `schools`");

    let mut out = String::new();
    writeln!(out, "/// Generated from `data/schools.json` (`fallback`).").unwrap();
    writeln!(out, "const FALLBACK: &str = {fallback:?};").unwrap();
    writeln!(out, "/// Generated from `data/schools.json` (`schools`): character -> school tile.").unwrap();
    writeln!(out, "const SCHOOLS: &[(&str, &str)] = &[").unwrap();
    for r in rows {
        let character = r["character"].as_str().expect("schools.json: `character`");
        let school = r["school"].as_str().expect("schools.json: `school`");
        writeln!(out, "    ({character:?}, {school:?}),").unwrap();
    }
    writeln!(out, "];").unwrap();

    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("schools.rs");
    std::fs::write(dest, out).expect("write schools.rs");
}

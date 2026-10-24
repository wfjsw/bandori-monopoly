//! Print the generated Kani guard IR table (debugging aid; the real build
//! generates it from `rules/kani/build.rs`).
//!
//! ```sh
//! cargo run -p rules-cond --features lower --example gen_cond_ir [-- OUT.rs]
//! ```

fn main() {
    // Match `rules/kani/build.rs`: the real ruleset vocabulary, so sugar
    // guards lower. `generate` reports any remaining skips (reason + count).
    let _ = rules_cond::install_ext(bandori_std::cel::ext());
    // `crates/cond` → `rules/`.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules");
    let (src, n) = rules_cond::gen_ir::generate(&root);
    match std::env::args().nth(1) {
        Some(p) => {
            std::fs::write(&p, src).unwrap();
            eprintln!("wrote {p} ({n} guards)");
        }
        None => print!("{src}"),
    }
}
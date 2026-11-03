#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn world_size() {
    eprintln!(
        "world={} namemap={}",
        std::mem::size_of::<rules_kani::world::World>(),
        std::mem::size_of::<rules_kani::world::NameMap>()
    );
}

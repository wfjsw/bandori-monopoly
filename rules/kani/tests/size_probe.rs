#[test]
fn world_size() {
    eprintln!(
        "world={} namemap={}",
        std::mem::size_of::<rules_kani::world::World>(),
        std::mem::size_of::<rules_kani::world::NameMap>()
    );
}

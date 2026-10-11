//! Consistency of `generated/card_ids.rs` with `invoke::RULESET`.
//!
//! The generated table is the compile-time id → index map; `by_idx` walks
//! `RULESET` with integer arithmetic. They must agree on every id.

#[test]
fn card_count_matches_ruleset() {
    let n = rules_kani::invoke::ruleset_len();
    assert_eq!(
        n,
        rules_kani::generated::CARD_COUNT,
        "gen_card_ids.py is stale — re-run tools/kani/gen_card_ids.py"
    );
}

#[test]
fn every_idx_resolves_to_its_id() {
    let n = rules_kani::generated::CARD_COUNT;
    for i in 0..n {
        let def = rules_kani::invoke::by_idx(i);
        assert_eq!(
            def.id,
            rules_kani::generated::IDS[i],
            "index {i} id mismatch (RULESET vs generated)"
        );
    }
}

#[test]
fn card_id_opt_roundtrips() {
    for (i, id) in rules_kani::generated::IDS.iter().enumerate() {
        assert_eq!(rules_kani::generated::card_id_opt(id), Some(i));
    }
    assert_eq!(rules_kani::generated::card_id_opt("not-a-real-id"), None);
}

#[test]
fn named_consts_match_ids() {
    // Spot-check a few well-known ids through the macro's runtime twin.
    assert_eq!(
        rules_kani::generated::card_id_const("MyGO:轮符雨"),
        rules_kani::invoke::find_idx("MyGO:轮符雨").unwrap()
    );
    assert_eq!(
        rules_kani::generated::card_id_const("PP:[丸山彩]憧憬的前方"),
        rules_kani::invoke::find_idx("PP:[丸山彩]憧憬的前方").unwrap()
    );
}
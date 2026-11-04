//! G09 -- tile, build, pass and house-removal hooks (design
//! `docs/rulebook/spec/abi/G09-tile-build-pass.md` §9.1).
//!
//! Everything is observed on match state (money, houses, owners, field cards,
//! prompts, log keys), never on engine internals.

mod common;
use common::*;
use game_core::engine::CardRules;
use game_core::msg::Msg;

const CIRCLE: usize = 0;
const GROUP: usize = 28; // 弦卷集团
const BLUE: usize = 21; // DUB MUSIC EXPERIMENT (livehouse, price 2400)
const SPACE: usize = 42; // Space (livehouse, 1200)
const FUJIMI: usize = 11; // 富士见坂 (ordinary deed, 600)
const MARINA: &str = "通用:[月岛麻里奈]今天也要加油工作喔";
const PARADE: &str = "HHW:笑容大游行";
const MIRACLE: &str = "MyGO:难以复刻的奇迹";
const GUERRILLA: &str = "RAS:游击演出";
const BLOOM: &str = "R:（燐子）Ringing Bloom";

// ---------------------------------------------------------------- helpers

/// Skip every open prompt.
fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// Answer the open prompt with its default (a `tile` prompt: none).
fn answer_default(t: &mut Table) {
    let p = t.expect_prompt();
    let who = t.asked()[0];
    let v = if p.kind == "tile" {
        p.items.len() as i32
    } else {
        p.fallback
    };
    let _ = t.answer(who, v);
}

/// Answer the open prompt for `who` alone (a `tile` prompt: none).
fn decline_who(t: &mut Table, who: usize) {
    let p = t.expect_prompt();
    let v = if p.kind == "tile" {
        p.items.len() as i32
    } else {
        p.fallback
    };
    let _ = t.answer(who, v);
}

/// Drive an open walk to rest: rewards take the **money** option (the base
/// reward is the one fact under test), everything else its default.
fn walk_to_rest(t: &mut Table) {
    for _ in 0..12 {
        let Some(p) = t.prompt() else { break };
        if p.title.key().contains("pass_reward") {
            let _ = t.answer_one(0);
        } else {
            answer_default(t);
        }
    }
    drain(t);
}

/// [`walk_to_rest`] while watching for `card`'s [反击] window; `declare` it
/// when offered. Answers whether it was ever offered.
fn walk_watching(t: &mut Table, card: &str, declare: bool) -> bool {
    let mut offered = false;
    for _ in 0..12 {
        let Some(p) = t.prompt() else { break };
        if t.counteract_offered(card) {
            offered = true;
            if declare {
                t.counteract(0, card).unwrap();
            } else {
                answer_default(t);
            }
            continue;
        }
        if p.title.key().contains("pass_reward") {
            let _ = t.answer_one(0);
        } else {
            answer_default(t);
        }
    }
    drain(t);
    offered
}

fn pass(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..20 {
        if t.turn() == who {
            return;
        }
        let cur = t.turn();
        pass(t, cur);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

/// Place `card` on `tile` as `who`, bypassing the play.
fn place_on_tile(t: &mut Table, who: usize, card: &str, tile: usize) {
    let d = data();
    let props = rules().card_props(card);
    t.m.world_mut()
        .place_card_on(&d, who as i32, tile as i32, card, Msg::default(), props);
}

/// Place a **board-owned** instance (owner `-1`) on `tile`.
fn place_board(t: &mut Table, card: &str, tile: i32) -> i32 {
    place_board_with(t, card, tile, &[])
}

/// Board instance with extra props.
fn place_board_with(t: &mut Table, card: &str, tile: i32, props: &[(&str, i32)]) -> i32 {
    let d = data();
    let mut p = rules().card_props(card);
    for (k, v) in props {
        p.insert((*k).into(), *v);
    }
    t.m.world_mut().place_card_on(
        &d,
        game_core::state::BOARD_OWNER,
        tile,
        card,
        Msg::default(),
        p,
    )
}

/// Arm the swap prop on `who`'s placed copy of `card` (the raw-placement
/// equivalent of `ctx::set_swapped_tile`).
fn set_swap(t: &mut Table, who: usize, card: &str, partner: usize) {
    let w = t.m.world_mut();
    let f = w.st.players[who]
        .field
        .iter_mut()
        .find(|f| f.card == card)
        .expect("card placed");
    f.props
        .insert(card_sdk::abi::prop::SWAPPED_TILE.into(), partner as i32);
}

fn keys(t: &Table) -> Vec<String> {
    t.recent_keys(60)
}

fn saw(t: &Table, needle: &str) -> bool {
    keys(t).iter().any(|k| k.contains(needle))
}

/// The `(tile, position)` of every settle-probe line -- the dispatched tile
/// and the physical slot it was raised at.
fn probes(t: &Table) -> Vec<(i64, i64)> {
    t.st()
        .events
        .iter()
        .filter(|e| log_key(e) == "cards:fixture-test-cards.tile_id_seen")
        .map(|e| {
            (
                int_arg(&e.msg, "tile").unwrap_or(-9),
                int_arg(&e.msg, "position").unwrap_or(-9),
            )
        })
        .collect()
}

// ================================================================ item 1

/// The extra: a field instance on CiRCLE whose `Settle` hook logs.
fn extra_on_circle(t: &mut Table) {
    place_on_tile(t, 0, "TEST:tile_id_probe", CIRCLE);
}

#[test]
fn g09_marina_offered_on_a_circle_with_an_extra() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "月岛麻里奈");
    t.give(0, &[MARINA]);
    extra_on_circle(&mut t);
    t.set_pos(0, (CIRCLE + 60 - 1) % 60);
    t.dice(&[2]);
    t.roll(0).unwrap();
    assert!(
        walk_watching(&mut t, MARINA, false),
        "{:?}",
        t.recent_keys(25)
    );
}

#[test]
fn g09_marina_not_offered_on_a_plain_circle() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "月岛麻里奈");
    t.give(0, &[MARINA]);
    t.set_pos(0, (CIRCLE + 60 - 1) % 60);
    t.dice(&[2]);
    t.roll(0).unwrap();
    assert!(
        !walk_watching(&mut t, MARINA, false),
        "no extra -> no window: {:?}",
        t.recent_keys(25)
    );
}

#[test]
fn g09_marina_window_opens_when_the_reward_does_not() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "月岛麻里奈");
    t.give(0, &[MARINA]);
    extra_on_circle(&mut t);
    place_board_with(&mut t, "tile:circle", CIRCLE as i32, &[("noReward", 1)]);
    t.set_pos(0, (CIRCLE + 60 - 1) % 60);
    t.dice(&[2]);
    t.roll(0).unwrap();
    assert!(
        walk_watching(&mut t, MARINA, false),
        "the window is on the tile, not the reward"
    );
}

/// The extra is a `Settle` hook; with marina declared it must not run at the
/// pass step or the settle, while the base reward still pays.
#[test]
fn g09_marina_suppresses_pass_and_settle_extras() {
    for counter in [false, true] {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "月岛麻里奈");
        t.give(0, &[MARINA]);
        extra_on_circle(&mut t);
        t.set_pos(0, (CIRCLE + 60 - 1) % 60);
        t.dice(&[1]); // land on CiRCLE
        t.roll(0).unwrap();
        walk_watching(&mut t, MARINA, counter);
        assert_eq!(
            saw(&t, "tile_id_seen"),
            !counter,
            "extra {}: {:?}",
            if counter { "suppressed" } else { "runs" },
            t.recent_keys(25)
        );
        assert!(t.money(0) > 10_000, "the base reward still pays");
    }
}

#[test]
fn g09_marina_does_not_suppress_the_base_rule() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "月岛麻里奈");
    t.give(0, &[MARINA]);
    extra_on_circle(&mut t);
    t.set_draw(0, &["通用:GREAT", "通用:安可", "通用:登上武道馆"]);
    t.set_pos(0, (CIRCLE + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    let hand = t.hand(0).len();
    for _ in 0..12 {
        let Some(p) = t.prompt() else { break };
        if t.counteract_offered(MARINA) {
            t.counteract(0, MARINA).unwrap();
        } else if p.title.key().contains("pass_reward") {
            // Option 1 = the draw.
            let _ = t.answer_one(1);
        } else {
            answer_default(&mut t);
        }
    }
    drain(&mut t);
    assert!(
        t.hand(0).len() >= hand + 1,
        "base draw: hand {} -> {}: {:?}",
        hand,
        t.hand(0).len(),
        t.recent_keys(25)
    );
}

// ================================================================ item 2

#[test]
fn g09_parade_pass_runs_the_swapped_rule() {
    let mut t = Table::vanilla(2);
    place_on_tile(&mut t, 0, PARADE, CIRCLE);
    set_swap(&mut t, 0, PARADE, GROUP);
    // Passing 弦卷集团 runs CiRCLE's pass rule.
    t.set_pos(0, (GROUP + 60 - 1) % 60);
    t.dice(&[2]);
    t.roll(0).unwrap();
    walk_to_rest(&mut t);
    assert!(
        t.money(0) > 10_000,
        "the CiRCLE reward paid: {:?}",
        t.recent_keys(25)
    );
    // Passing the card's own slot runs 弦卷集团 (no reward).
    let mut t = Table::vanilla(2);
    place_on_tile(&mut t, 0, PARADE, CIRCLE);
    set_swap(&mut t, 0, PARADE, GROUP);
    t.set_pos(0, (CIRCLE + 60 - 1) % 60);
    t.dice(&[2]);
    t.roll(0).unwrap();
    walk_to_rest(&mut t);
    assert_eq!(t.money(0), 10_000, "no CiRCLE reward on the card's slot");
}

#[test]
fn g09_parade_settle_runs_the_swapped_rule() {
    let mut t = Table::vanilla(2);
    place_on_tile(&mut t, 0, PARADE, CIRCLE);
    place_on_tile(&mut t, 0, "TEST:tile_id_probe", CIRCLE);
    set_swap(&mut t, 0, PARADE, GROUP);
    // Settling 弦卷集团's home slot, which now displays CiRCLE, runs CiRCLE:
    // the governing tile is resolved off the slot, and the settle probe reads
    // it at the physical slot.
    t.set_pos(0, (GROUP + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    walk_to_rest(&mut t);
    assert_eq!(probes(&t), vec![(CIRCLE as i64, GROUP as i64)]);
    // Settling the card's own slot runs 弦卷集团 -- the slot displays it.
    let mut t = Table::vanilla(2);
    place_on_tile(&mut t, 0, PARADE, CIRCLE);
    place_on_tile(&mut t, 0, "TEST:tile_id_probe", CIRCLE);
    set_swap(&mut t, 0, PARADE, GROUP);
    t.set_pos(0, (CIRCLE + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    walk_to_rest(&mut t);
    assert_eq!(probes(&t), vec![(GROUP as i64, CIRCLE as i64)]);
}

#[test]
fn g09_parade_discard_only_on_its_own_position() {
    // A settle on the card's own slot discards.
    let mut t = Table::vanilla(2);
    place_on_tile(&mut t, 0, PARADE, SPACE);
    set_swap(&mut t, 0, PARADE, GROUP);
    t.set_crystals(0, PARADE, 5);
    until_turn(&mut t, 1);
    t.set_pos(1, (SPACE + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    walk_to_rest(&mut t);
    assert!(!t.on_field(0, PARADE), "{:?}", t.field_ids(0));
    // A settle on 弦卷集团's own slot does not.
    let mut t = Table::vanilla(2);
    place_on_tile(&mut t, 0, PARADE, SPACE);
    set_swap(&mut t, 0, PARADE, GROUP);
    t.set_crystals(0, PARADE, 5);
    until_turn(&mut t, 1);
    t.set_pos(1, (GROUP + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    walk_to_rest(&mut t);
    assert!(t.on_field(0, PARADE), "{:?}", t.field_ids(0));
}

/// The identity follows the resolution, not the start prop: CiRCLE is
/// reachable through the swapped slot while its `START` prop stays put.
#[test]
fn g09_parade_identity_stays() {
    let mut t = Table::vanilla(2);
    place_on_tile(&mut t, 0, PARADE, CIRCLE);
    set_swap(&mut t, 0, PARADE, GROUP);
    assert_eq!(t.st().tile_at[CIRCLE], GROUP as i32);
    assert_eq!(t.st().tile_at[GROUP], CIRCLE as i32);
    assert_eq!(t.tile_prop(CIRCLE, "start"), 1, "the prop stays on CiRCLE");
    assert_eq!(t.tile_prop(GROUP, "start"), 0, "never copied to the slot");
    // A settle at the group slot runs CiRCLE's base rule (the draw).
    t.set_draw(0, &["通用:GREAT"]);
    t.set_pos(0, (GROUP + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    let hand = t.hand(0).len();
    walk_to_rest(&mut t);
    assert!(t.hand(0).len() >= hand + 1, "{:?}", t.recent_keys(25));
}

#[test]
fn g09_parade_swap_ends_on_discard() {
    let mut t = Table::vanilla(2);
    place_on_tile(&mut t, 0, PARADE, SPACE);
    set_swap(&mut t, 0, PARADE, GROUP);
    t.set_crystals(0, PARADE, 5);
    assert_eq!(t.st().tile_at[SPACE], GROUP as i32);
    until_turn(&mut t, 1);
    t.set_pos(1, (SPACE + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    walk_to_rest(&mut t);
    assert!(!t.on_field(0, PARADE));
    assert_eq!(t.st().tile_at[SPACE], SPACE as i32, "identity is back");
    assert_eq!(t.st().tile_at[GROUP], GROUP as i32);
}

#[test]
fn g09_parade_view_draws_the_tile_at_each_slot() {
    let mut t = Table::vanilla(2);
    place_on_tile(&mut t, 0, PARADE, SPACE);
    set_swap(&mut t, 0, PARADE, GROUP);
    let st = t.st();
    assert_eq!(st.tile_at[SPACE], GROUP as i32);
    assert_eq!(st.tile_at[GROUP], SPACE as i32);
    let f = t.field(0).into_iter().find(|f| f.card == PARADE).unwrap();
    assert_eq!(f.position(), SPACE as i32, "the card draws at its slot");
}

// ================================================================ item 3

/// The `card_build_as` gate refuses exactly the tiles the build step refuses.
#[test]
fn g09_card_build_gate_matches_the_build_step() {
    for (name, refuser) in [
        ("ring", 0usize),
        ("full", 1),
        ("mortgaged", 2),
        ("no_build", 3),
        ("no_build_above", 4),
    ] {
        let mut t = Table::vanilla(2);
        let tgt = if refuser == 0 { CIRCLE } else { FUJIMI };
        t.own(0, &[tgt]);
        match refuser {
            1 => t.set_houses(tgt, 4),
            2 => t.set_mortgaged(tgt, true),
            3 => {
                place_board_with(&mut t, "tile:deed", tgt as i32, &[("noBuild", 1)]);
            }
            4 => {
                place_board_with(&mut t, "tile:deed", -1, &[("noBuildAbove", 1)]);
            }
            _ => {}
        }
        let before = t.houses(tgt);
        t.set_pos(0, tgt);
        t.give_play(0, "TEST:build_free").unwrap();
        drain(&mut t);
        assert_eq!(t.houses(tgt), before, "{name}: refused");
        // The fixture logs `ok = 0` on a refusal.
        let trial = t
            .st()
            .events
            .iter()
            .rev()
            .find(|e| log_key(e).contains("build_tried"))
            .and_then(|e| int_arg(&e.msg, "ok"));
        assert_eq!(trial, Some(0), "{name}: refused");
    }
    // Positive control.
    let mut t = Table::vanilla(2);
    t.own(0, &[FUJIMI]);
    t.set_pos(0, FUJIMI);
    t.give_play(0, "TEST:build_free").unwrap();
    drain(&mut t);
    assert_eq!(t.houses(FUJIMI), 1, "a legal tile takes the house");
}

#[test]
fn g09_patrol_free_build_respects_tile_vetoes() {
    let mut t = Table::vanilla(2);
    t.own(0, &[BLUE, FUJIMI]);
    place_board_with(&mut t, "tile:deed", BLUE as i32, &[("noBuild", 1)]);
    t.set_pos(0, BLUE);
    t.dice(&[20, 1, 1]); // sum 22 -> position 21 = DUB MUSIC EXPERIMENT
    t.give_play(0, "HHW:微笑巡逻队").unwrap();
    let mut chose = false;
    while t.prompt().is_some() {
        if t.expect_prompt().kind == "tile" {
            chose = true;
            let _ = t.answer_tile(0, FUJIMI);
        } else {
            t.decline();
        }
    }
    assert!(chose, "the paid half still had a target");
    assert_eq!(t.houses(BLUE), 0, "NO_BUILD: {:?}", t.recent_keys(25));
    assert_eq!(t.houses(FUJIMI), 1, "the paid build went to the legal tile");
}

#[test]
fn g09_patrol_affordability_uses_the_effective_price() {
    let mut t = Table::vanilla(2);
    t.own(0, &[BLUE]);
    t.m.world_mut().turn.build_cost_pct = 50;
    let half = {
        let w = t.m.world();
        w.build_price(&data(), &w.turn, BLUE as i32)
    };
    assert!(half > 0);
    t.set_money(0, half);
    t.set_pos(0, BLUE);
    assert_eq!(
        t.give_play(0, "HHW:微笑巡逻队"),
        Ok(()),
        "the effective (half) price is affordable"
    );
    drain(&mut t);
    // Just under the effective price: refused before the play.
    let mut t = Table::vanilla(2);
    t.own(0, &[BLUE]);
    t.m.world_mut().turn.build_cost_pct = 50;
    t.set_money(0, half - 10);
    t.set_pos(0, BLUE);
    assert_eq!(
        t.give_play(0, "HHW:微笑巡逻队"),
        Err("cards:card-hhw.x_no_buildable".to_string()),
        "{half} - 10 is short"
    );
}

#[test]
fn g09_miracle_free_build_spends_one_crystal_not_money() {
    let mut t = Table::vanilla(2);
    t.own(0, &[FUJIMI]);
    place_on_tile(&mut t, 0, MIRACLE, FUJIMI);
    t.set_crystals(0, MIRACLE, 1);
    t.set_pos(0, FUJIMI);
    t.give_play(0, "TEST:build_free").unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000, "the crystal covered the price");
    assert_eq!(t.crystals(0, MIRACLE), Some(0), "one crystal spent");
    assert_eq!(t.houses(FUJIMI), 1);
}

#[test]
fn g09_miracle_already_free_build_keeps_the_crystal() {
    let mut t = Table::vanilla(2);
    t.own(0, &[FUJIMI]);
    place_on_tile(&mut t, 0, MIRACLE, FUJIMI);
    t.set_crystals(0, MIRACLE, 1);
    t.m.world_mut().turn.build_cost_pct = 0; // effective price 0
    t.set_pos(0, FUJIMI);
    t.give_play(0, "TEST:build_free").unwrap();
    drain(&mut t);
    assert_eq!(t.crystals(0, MIRACLE), Some(1), "crystal kept");
    assert_eq!(t.houses(FUJIMI), 1);
}

#[test]
fn g09_to_the_peak_charges_one_and_a_half_and_runs_hooks() {
    let mut t = Table::vanilla(2);
    // Every livehouse bought; P0 holds Space with 0 houses.
    let all: Vec<usize> = [
        "RiNG 1",
        "RiNG 2",
        "DUB MUSIC EXPERIMENT",
        "RiNG 3",
        "武道馆",
        "Space",
        "Live House Galaxy",
        "RiNG 4",
    ]
    .iter()
    .map(|n| tile(n))
    .collect();
    let space = tile("Space");
    t.own(0, &[space]);
    let rest: Vec<usize> = all.into_iter().filter(|&x| x != space).collect();
    t.own(1, &rest);
    place_on_tile(&mut t, 0, "TEST:build_probe", space);
    t.set_pos(0, space);
    let mark = t.mark();
    t.give_play(0, "R:向着顶点").unwrap();
    let mut asked = false;
    for _ in 0..8 {
        let Some(p) = t.prompt() else { break };
        if p.kind == "tile" {
            asked = true;
            let _ = t.answer_tile(0, space);
        } else if !asked {
            let _ = t.answer_one(0); // the 「可」 (yes)
        } else {
            answer_default(&mut t);
        }
    }
    drain(&mut t);
    // build_price(Space) = 500 -> ceil(750, 10) = 750.
    assert_eq!(
        t.money(0),
        10_000 - 750,
        "1.5x the effective price: {}",
        t.dump_prompt()
    );
    assert_eq!(t.houses(space), 1);
    let k = t.keys_since(mark);
    let at = |n: &str| k.iter().position(|x| x.contains(n)).expect(n);
    assert!(at("probe_build_before") < at("probe_house_added"));
    assert!(at("probe_house_added") < at("probe_build_after"));
}

#[test]
fn g09_card_build_house_added_precedes_buildAfter() {
    let mut t = Table::vanilla(2);
    t.own(0, &[FUJIMI]);
    place_on_tile(&mut t, 0, "TEST:build_probe", FUJIMI);
    t.set_pos(0, FUJIMI);
    let mark = t.mark();
    t.give_play(0, "TEST:build_free").unwrap();
    drain(&mut t);
    let k = t.keys_since(mark);
    let at = |n: &str| k.iter().position(|x| x.contains(n)).expect(n);
    assert!(at("probe_build_before") < at("probe_house_added"));
    assert!(at("probe_house_added") < at("probe_build_after"));
}

#[test]
fn g09_build_before_veto_skips_the_build() {
    let mut t = Table::vanilla(2);
    t.own(0, &[FUJIMI]);
    place_on_tile(&mut t, 0, "TEST:build_veto", FUJIMI);
    t.set_pos(0, FUJIMI);
    t.give_play(0, "TEST:build_free").unwrap();
    drain(&mut t);
    assert_eq!(t.houses(FUJIMI), 0, "vetoed");
    assert_eq!(t.money(0), 10_000);
}

// ================================================================ item 4

fn war_setup(t: &mut Table) -> usize {
    let tile = FUJIMI;
    t.own(1, &[tile]);
    t.give(1, &["AG:宣战布告"]);
    t.set_pos(0, tile);
    tile
}

fn war_offered(t: &mut Table) -> bool {
    for _ in 0..4 {
        if t.counteract_offered("AG:宣战布告") {
            return true;
        }
        if t.prompt().is_some() && t.asked().len() == 1 {
            let who = t.asked()[0];
            decline_who(t, who);
        } else {
            return false;
        }
    }
    t.counteract_offered("AG:宣战布告")
}

#[test]
fn g09_declare_war_answers_an_owned_tile_house_add() {
    let mut t = Table::vanilla(2);
    war_setup(&mut t);
    t.set_houses(FUJIMI, 0);
    t.give(0, &["TEST:house_add"]);
    t.play(0, "TEST:house_add").unwrap();
    assert!(war_offered(&mut t), "{}", t.dump_prompt());
}

#[test]
fn g09_declare_war_answers_an_owned_tile_house_removal() {
    let mut t = Table::vanilla(2);
    war_setup(&mut t);
    t.set_houses(FUJIMI, 2);
    t.give(0, &["TEST:house_remove"]);
    t.play(0, "TEST:house_remove").unwrap();
    assert!(war_offered(&mut t), "{}", t.dump_prompt());
}

#[test]
fn g09_declare_war_not_offered_on_an_unowned_tile() {
    let mut t = Table::vanilla(2);
    t.give(1, &["AG:宣战布告"]);
    t.set_pos(0, FUJIMI);
    t.give(0, &["TEST:house_add"]);
    t.play(0, "TEST:house_add").unwrap();
    drain(&mut t);
    assert_eq!(t.houses(FUJIMI), 1, "the write landed unanswered");
}

#[test]
fn g09_declare_war_answers_an_owner_set() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, FUJIMI);
    t.give(1, &["AG:宣战布告"]);
    t.give(0, &["TEST:owner_set"]);
    t.play(0, "TEST:owner_set").unwrap();
    // Owner set to seat 1, so the tile is theirs when the window opens.
    assert!(war_offered(&mut t), "{}", t.dump_prompt());
}

#[test]
fn g09_tile_write_cancel_skips_the_write() {
    let mut t = Table::vanilla(2);
    // P1 owns the tile, so the write's `target` reaches them and their
    // `TEST:counter` (a `ChainKind::Effect` responder) is offered.
    t.own(1, &[FUJIMI]);
    t.set_pos(0, FUJIMI);
    t.give(1, &["TEST:counter"]);
    t.give(0, &["TEST:house_add"]);
    t.play(0, "TEST:house_add").unwrap();
    let mut offered = false;
    for _ in 0..4 {
        if t.counteract_offered("TEST:counter") {
            offered = true;
            break;
        }
        if t.prompt().is_some() {
            let who = t.asked()[0];
            decline_who(&mut t, who);
        } else {
            break;
        }
    }
    assert!(offered, "{}", t.dump_prompt());
    t.counteract(1, "TEST:counter").unwrap();
    drain(&mut t);
    assert_eq!(t.houses(FUJIMI), 0, "the cancelled write never landed");
}

// ================================================================ item 5

#[test]
fn g09_guerrilla_not_offered_on_a_rent_without_a_passed_lh() {
    let mut t = Table::vanilla(2);
    t.own(1, &[FUJIMI]);
    t.give(0, &[GUERRILLA]);
    t.set_pos(0, (FUJIMI + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(
        !walk_watching(&mut t, GUERRILLA, false),
        "{:?}",
        t.recent_keys(25)
    );
}

#[test]
fn g09_guerrilla_offered_after_passing_another_players_lh() {
    let mut t = Table::vanilla(2);
    t.own(1, &[SPACE, FUJIMI]);
    t.give(0, &[GUERRILLA]);
    // Step onto Space (P1's livehouse): the rent opens `Paid` on that step, so
    // the settle's move path already carries the passed livehouse.
    t.set_pos(0, (SPACE + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(
        walk_watching(&mut t, GUERRILLA, false),
        "{} events {:?}",
        t.dump_prompt(),
        t.recent_keys(25)
    );
}

#[test]
fn g09_guerrilla_not_offered_after_passing_own_lh() {
    let mut t = Table::vanilla(2);
    t.own(0, &[SPACE]);
    t.give(0, &[GUERRILLA]);
    t.set_pos(0, (SPACE + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(
        !walk_watching(&mut t, GUERRILLA, false),
        "{:?}",
        t.recent_keys(25)
    );
}

#[test]
fn g09_path_is_every_step() {
    let mut t = Table::vanilla(2);
    place_on_tile(&mut t, 0, "TEST:path_probe", 2);
    t.own(1, &[2]);
    t.set_pos(0, (CIRCLE + 60 - 1) % 60);
    t.dice(&[3]); // 0, 1, 2 -- lands on the rent tile
    t.roll(0).unwrap();
    walk_to_rest(&mut t);
    let full = "cards:fixture-test-cards.path_seen";
    let n = t
        .st()
        .events
        .iter()
        .find(|e| log_key(e) == full)
        .and_then(|e| int_arg(&e.msg, "n"));
    assert_eq!(n, Some(3), "every step: {:?}", t.recent_keys(25));
}

#[test]
fn g09_path_is_empty_outside_a_move() {
    let mut t = Table::vanilla(2);
    t.own(0, &[FUJIMI]);
    place_on_tile(&mut t, 0, "TEST:path_probe", FUJIMI);
    t.set_pos(0, FUJIMI);
    t.give_play(0, "TEST:rent_charge").unwrap();
    drain(&mut t);
    // `charge_rent` is a card rent, not a settle -- its `Paid` carries no move.
    let full = "cards:fixture-test-cards.path_seen";
    let n = t
        .st()
        .events
        .iter()
        .find(|e| log_key(e) == full)
        .and_then(|e| int_arg(&e.msg, "n"));
    assert_eq!(n, Some(0), "{:?}", t.recent_keys(25));
}

// ================================================================ item 6

fn removed_hits(t: &Table) -> usize {
    t.recent_keys(60)
        .into_iter()
        .filter(|k| k.contains("house_removed_seen"))
        .count()
}

#[test]
fn g09_house_removed_on_add_house_fall() {
    let mut t = Table::vanilla(2);
    t.own(0, &[FUJIMI]);
    t.set_houses(FUJIMI, 2);
    place_on_tile(&mut t, 0, "TEST:house_removed_probe", FUJIMI);
    t.set_pos(0, FUJIMI);
    t.give_play(0, "TEST:house_remove").unwrap();
    drain(&mut t);
    assert_eq!(removed_hits(&t), 1, "raised exactly once");
    assert_eq!(t.houses(FUJIMI), 1);
    // `value` is the post count.
    let st = t.st();
    let e = st
        .events
        .iter()
        .rev()
        .find(|e| log_key(e).contains("house_removed_seen"))
        .expect("raised");
    assert_eq!(int_arg(&e.msg, "value"), Some(1));
}

#[test]
fn g09_house_removed_on_set_houses_fall() {
    let mut t = Table::vanilla(2);
    t.own(0, &[FUJIMI]);
    t.set_houses(FUJIMI, 3);
    place_on_tile(&mut t, 0, "TEST:house_removed_probe", FUJIMI);
    t.set_pos(0, FUJIMI);
    t.give_play(0, "TEST:house_set").unwrap();
    drain(&mut t);
    assert!(removed_hits(&t) >= 1, "{:?}", t.recent_keys(25));
    assert_eq!(t.houses(FUJIMI), 1);
}

#[test]
fn g09_house_removed_not_raised_on_equal_clamp() {
    let mut t = Table::vanilla(2);
    t.own(0, &[FUJIMI]);
    t.set_houses(FUJIMI, 0);
    place_on_tile(&mut t, 0, "TEST:house_removed_probe", FUJIMI);
    t.set_pos(0, FUJIMI);
    t.give_play(0, "TEST:house_remove").unwrap();
    drain(&mut t);
    assert_eq!(removed_hits(&t), 0, "{:?}", t.recent_keys(25));
    assert_eq!(t.houses(FUJIMI), 0);
}

#[test]
fn g09_ringing_bloom_recomputes_on_raze() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "白金燐子");
    t.own(0, &[SPACE, BLUE]);
    t.set_houses(BLUE, 2);
    t.set_houses(SPACE, 1);
    t.give_play(0, BLOOM).unwrap();
    drain(&mut t);
    assert_eq!(
        t.tile_prop(SPACE, card_sdk::abi::prop::RENT_FACTOR),
        500,
        "Space is lifted while BLUE is the max"
    );
    // Raze the max tile with a raw write.
    t.set_pos(0, BLUE);
    t.give_play(0, "TEST:house_remove").unwrap();
    drain(&mut t);
    assert_eq!(
        t.tile_prop(SPACE, card_sdk::abi::prop::RENT_FACTOR),
        0,
        "the raze cleared the stale half-charge: {:?}",
        t.recent_keys(25)
    );
}

#[test]
fn g09_ringing_bloom_half_ends_when_the_card_leaves() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "白金燐子");
    t.own(0, &[SPACE, BLUE]);
    t.set_houses(BLUE, 3);
    t.give_play(0, BLOOM).unwrap();
    drain(&mut t);
    assert_eq!(t.tile_prop(SPACE, card_sdk::abi::prop::RENT_FACTOR), 500);
    // P1 collects rent at Space (3): （3） discards the card and clears its
    // half-charges.
    until_turn(&mut t, 1);
    t.set_pos(1, (SPACE + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    walk_to_rest(&mut t);
    assert!(!t.on_field(0, BLOOM), "{:?}", t.field_ids(0));
    assert_eq!(
        t.tile_prop(SPACE, card_sdk::abi::prop::RENT_FACTOR),
        0,
        "the half-charge ended with the card"
    );
}

// ================================================================ static sweep

#[test]
fn g09_no_add_house_build_path_left_raw() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../cards");
    for band in std::fs::read_dir(&root).expect("cards dir") {
        let band = band.expect("band").path().join("src");
        let Ok(entries) = std::fs::read_dir(&band) else {
            continue;
        };
        for e in entries {
            let p = e.expect("entry").path();
            if p.extension().and_then(|x| x.to_str()) != Some("rs") {
                continue;
            }
            let src = std::fs::read_to_string(&p).expect("src");
            for line in src.lines() {
                if !line.contains("add_house(") || line.trim_start().starts_with("//") {
                    continue;
                }
                // `add_house(x, ±1)` is a build path unless the file is a
                // documented pure move (`parking_space` / `hey_kids` /
                // `sunset` / `pareo_far`).
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                let pure_move = ["parking_space.rs", "hey_kids.rs", "sunset.rs", "pareo_far.rs"]
                    .contains(&name.as_str());
                assert!(
                    pure_move,
                    "{}: raw add_house build path -- use card_build_as: {line}",
                    p.display()
                );
            }
        }
    }
}

/// Every new import has a link-name row and a Kani `Op` sibling (the static
/// sweep's ABI half; the link-name test covers `sys.rs`).
#[test]
fn g09_new_imports_are_wired() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let sys = std::fs::read_to_string(root.join("crates/card-sdk/src/ctx/sys.rs")).expect("sys");
    let shims = std::fs::read_to_string(root.join("crates/game-rules/src/native_shims.rs"))
        .expect("shims");
    for name in [
        "tile_at",
        "position_of_tile",
        "tile_extra_count",
        "self_position",
        "build_price",
        "build_cost_percent",
        "build_discount",
        "tile_buildable",
        "card_build_as",
    ] {
        assert!(sys.contains(&format!("ruleset_{name}")), "sys.rs: {name}");
        assert!(shims.contains(&format!("ruleset_{name}")), "shims: {name}");
    }
    // The trigger readers carry the `trig_` prefix.
    for name in ["trig_position", "trig_move_path_length", "trig_move_path"] {
        assert!(sys.contains(&format!("ruleset_{name}")), "sys.rs: {name}");
        assert!(shims.contains(&format!("ruleset_{name}")), "shims: {name}");
    }
}

//! Counteraction eligibility cuts the book names (out / off-board).
//!
//! The engine-side half of the bot-window suite lives in
//! `crates/game-rules/tests/chain_bot.rs` (out / bankrupt). This file holds the
//! off-board half: the engine keeps no named mirror of the layer keys, so the
//! cut is exercised here where `bandori_std::state_key` may name the frozen
//! wire key. "Is a bot" is not a rulebook reason -- an off-board seat is never
//! asked, whatever drives it.

mod common;

use bandori_std::state_key;
use common::*;
use game_core::state::{BotMentality, MatchEvent};

/// Fixture events with this name, oldest first (`key!("…")` in
/// `rules/fixtures/test-cards`).
fn events(t: &Table, name: &str) -> Vec<MatchEvent> {
    let full = format!("cards:fixture-test-cards.{name}");
    t.st()
        .events
        .iter()
        .filter(|e| log_key(e) == full)
        .cloned()
        .collect()
}

/// One integer argument per fixture event, oldest first.
fn ev_args(t: &Table, name: &str, arg: &str) -> Vec<i64> {
    events(t, name)
        .iter()
        .map(|e| int_arg(&e.msg, arg).unwrap_or(-1))
        .collect()
}

/// Integer argument `arg` of each event with the exact log key `key`.
fn key_args(t: &Table, key: &str, arg: &str) -> Vec<i64> {
    t.st()
        .events
        .iter()
        .filter(|e| log_key(e) == key)
        .map(|e| int_arg(&e.msg, arg).unwrap_or(-1))
        .collect()
}

/// The `who` of each `probe_fired` -- the resolution order.
fn who_of(t: &Table, name: &str) -> Vec<i64> {
    events(t, name)
        .iter()
        .map(|e| match e.msg.args.get("who") {
            Some(game_core::msg::Arg::PlayerId(v)) => *v as i64,
            Some(game_core::msg::Arg::Integer(v)) | Some(game_core::msg::Arg::Number(v)) => *v,
            other => panic!("who is not a player arg: {other:?}"),
        })
        .collect()
}

/// An off-board bot holds an eligible counter and is never asked. The live
/// counter at seat 1 is, so the window still opens.
///
/// Seat layout: `TEST:aimer` names `others().first()` = seat 1, so the
/// off-board seat sits at 2 and the live counter at 1 -- the effect link is
/// raised against a live player and the window opens normally.
#[test]
fn off_board_seats_never_get_an_offer() {
    let mut t = Table::vanilla(3);
    t.make_bot(2, BotMentality::Standard);
    t.set_state(2, state_key::EXILE, 1);
    t.give(0, &["TEST:aimer"]);
    t.give(1, &["TEST:probe"]);
    t.give(2, &["TEST:probe"]);
    t.play(0, "TEST:aimer").unwrap();
    let mut asked_ever = Vec::new();
    while t.prompt().is_some() {
        let a = t.asked();
        assert!(
            !a.contains(&2),
            "the off-board seat was offered: {:?} {}",
            a,
            t.dump_prompt()
        );
        asked_ever.extend(a);
        t.decline();
    }
    assert!(!asked_ever.is_empty(), "the window never opened at all");
    assert_eq!(
        who_of(&t, "probe_fired"),
        Vec::<i64>::new(),
        "nobody declared"
    );
    assert_eq!(
        ev_args(&t, "aimer_done", "got"),
        vec![1],
        "X named its live target"
    );
}

// ------------------------------------------------- G09 suppression matrix

use card_sdk::abi::prop::SWAPPED_TILE;
use game_core::engine::CardRules;
use game_core::msg::Msg;
use game_core::state::{TileMark, BOARD_OWNER};

const CIRCLE: usize = 0;
const GROUP: usize = 28; // 弦卷集团
const SUPPRESSER: &str = "TEST:tile_effect_counter";

fn place_on_tile(t: &mut Table, who: i32, card: &str, tile: i32) {
    let d = data();
    let props = rules().card_props(card);
    t.m.world_mut()
        .place_card_on(&d, who, tile, card, Msg::default(), props);
}

/// An **active event** is a board-wide rule instance (`tile = -1`) plus its
/// `ActiveEvent` row -- what `event_rule_instances` walks.
fn place_event(t: &mut Table, id: &str, driver: i32) {
    place_on_tile(t, BOARD_OWNER, &format!("event:{id}"), -1);
    t.m.world_mut().st.event_active.push(game_core::state::ActiveEvent {
        id: id.into(),
        player_id: driver,
        ..Default::default()
    });
}

fn push_ember(t: &mut Table, tile: i32) {
    let st = &mut t.m.world_mut().st;
    let uid = st.next_mark_uid;
    st.next_mark_uid += 1;
    st.marks.push(TileMark {
        uid,
        tile,
        kind: "ember".into(),
        ..Default::default()
    });
}

/// Drive the open walk/settle: declare [`SUPPRESSER`] when offered, take the
/// money option at a reward, decline otherwise.
fn walk_answering(t: &mut Table, who: usize, declare: bool) -> bool {
    let mut offered = false;
    for _ in 0..16 {
        let Some(p) = t.prompt() else { break };
        if t.counteract_offered(SUPPRESSER) {
            offered = true;
            if declare {
                t.counteract(who, SUPPRESSER).unwrap();
            } else {
                t.decline();
            }
            continue;
        }
        // A reward: option 0 is the money half.
        let _ = t.answer_one(0);
        let _ = p;
    }
    while t.prompt().is_some() {
        t.decline();
    }
    offered
}

/// G09 §9.2: the dispatch set at a settle is exactly the tile's own set --
/// (base rule / board extra / player extra / event-on-the-tile / tile mark /
/// status) x (tag set / unset) x (the settle slot bound to a partner / not).
///
/// The rows settle the slot `x` while its *tile* `g` carries the whole set, so
/// a dispatcher keyed on the position would reach the binding card alone. The
/// tag is armed with `trigger.tile` (the marina gesture), never the position.
#[test]
fn g09_suppression_matrix() {
    for swapped in [false, true] {
        for tagged in [false, true] {
            let mut t = Table::vanilla(2);
            let (g, x) = if swapped { (CIRCLE, GROUP) } else { (CIRCLE, CIRCLE) };
            if swapped {
                // The binding card sits on `x` and names `g`: `x` displays `g`.
                place_on_tile(&mut t, 0, "TEST:swap_probe", GROUP as i32);
                let w = t.m.world_mut();
                let f = w.st.players[0]
                    .field
                    .iter_mut()
                    .find(|f| f.card == "TEST:swap_probe")
                    .expect("binding card");
                f.props.insert(SWAPPED_TILE.into(), g as i32);
            }
            // The set, all of it on `g`: a board extra, a player extra with the
            // event it backs, and a status (player-scoped, so it rides every
            // settle). `tile:circle` is the base rule the board already placed.
            place_on_tile(&mut t, BOARD_OWNER, "TEST:tile_id_probe", g as i32);
            place_on_tile(&mut t, 0, "TEST:tile_id_probe", g as i32);
            place_event(&mut t, "火种燃尽之后会怎么样呢？", 0);
            push_ember(&mut t, g as i32);
            place_on_tile(&mut t, 0, "status:no_hand", -1);
            if tagged {
                t.give(1, &[SUPPRESSER]);
            }
            t.set_pos(0, (x + 60 - 1) % 60);
            t.dice(&[1]);
            t.roll(0).unwrap();
            let offered = walk_answering(&mut t, 1, tagged);
            assert_eq!(
                offered,
                tagged,
                "window offered (swapped={swapped}, tagged={tagged}): {:?}",
                t.recent_keys(30)
            );
            assert_eq!(
                events(&t, "tile_id_seen").len(),
                if tagged { 0 } else { 2 },
                "the extras (swapped={swapped}, tagged={tagged}): {:?}",
                t.recent_keys(30)
            );
            // `ctx::pay` is `log.lose`; the only loss in this setup is the burn.
            let burned = key_args(&t, "log.lose", "who").len();
            // An event governs no tile (`tile = -1`), so the tile-valued tag
            // never hides it; the mark it burns is read off the settle's tile.
            assert_eq!(
                burned, 1,
                "the event's tile mark (swapped={swapped}, tagged={tagged})"
            );
            assert!(
                t.money(0) > 10_000,
                "the base rule always runs (swapped={swapped}, tagged={tagged}): {}",
                t.money(0)
            );
            assert!(
                t.on_field(0, "status:no_hand"),
                "the status is untouched (swapped={swapped}, tagged={tagged})"
            );
        }
    }
}

/// The extra is placed on GROUP, **then** the slot is bound to CiRCLE (the
/// order the matrix above does not exercise: it binds before placing, so the
/// instance already carries the governing tile). The extra now governs the
/// tile standing at its slot, so the tag (raised with that tile) suppresses
/// it. `extra_suppressed` used to read the placement `tile` and let it run.
#[test]
fn g09_suppression_follows_the_standing_tile() {
    let mut t = Table::vanilla(2);
    place_on_tile(&mut t, 0, "TEST:tile_id_probe", GROUP as i32);
    place_on_tile(&mut t, 0, "TEST:swap_probe", GROUP as i32);
    {
        let w = t.m.world_mut();
        let f = w.st.players[0]
            .field
            .iter_mut()
            .find(|f| f.card == "TEST:swap_probe")
            .expect("binding card");
        f.props.insert(SWAPPED_TILE.into(), CIRCLE as i32);
    }
    t.give(1, &[SUPPRESSER]);
    t.set_pos(0, (GROUP + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    let offered = walk_answering(&mut t, 1, true);
    assert!(offered, "the window opened: {:?}", t.recent_keys(30));
    assert_eq!(
        events(&t, "tile_id_seen").len(),
        0,
        "the extra governs the standing tile and is suppressed: {:?}",
        t.recent_keys(30)
    );
}

/// The settle trigger reports the slot the player stands on even when a
/// `tileEffectBefore` counter swaps the tile out from under them. The frame
/// pins the tile identity, but `position` is physical: `trigger.position`
/// readers (smile_parade's own-slot check) must see the slot, not the tile's
/// new home.
///
/// A card-driven settle opens the frame outright, so the window answered below
/// is the frame's own `tileEffectBefore` -- a roll landing raises a walk
/// window first, and the settle re-resolves its tile after that one.
#[test]
fn g09_settle_position_survives_a_mid_window_swap() {
    let mut t = Table::vanilla(2);
    place_on_tile(&mut t, 0, "TEST:tile_id_probe", GROUP as i32);
    // The counter sits elsewhere and names GROUP: once declared, GROUP stands
    // at its slot, away from the player.
    place_on_tile(&mut t, 1, "TEST:tile_swap_counter", 40);
    t.set_pos(0, GROUP);
    t.give(0, &["TEST:settle_here"]);
    t.play(0, "TEST:settle_here").unwrap();
    assert!(
        t.counteract_offered("TEST:tile_swap_counter"),
        "{}",
        t.dump_prompt()
    );
    t.counteract(1, "TEST:tile_swap_counter").unwrap();
    // A field counter stays offered on every re-opened window; decline it and
    // everything else to the end of the turn.
    while t.prompt().is_some() {
        t.decline();
    }
    assert_eq!(
        ev_args(&t, "tile_id_seen", "position"),
        vec![GROUP as i64],
        "the physical slot, not the tile's new home: {:?}",
        t.recent_keys(30)
    );
    assert_eq!(
        ev_args(&t, "tile_id_seen", "tile"),
        vec![GROUP as i64],
        "the frame's pinned tile identity"
    );
}

//! G08: the zone and attachment lifecycle (design
//! `docs/rulebook/spec/abi/G08-zone-lifecycle.md` §10.1).
//!
//! Every card move is one transition through the single writer: the before
//! hook, then the card's own destination rewrite, then the attachment and
//! creator cascades, then the row change, then the after hooks. The
//! `TEST:zoner` fixture logs every zone raise with its payload, so each test
//! reads the pair, the cause and the two bits as the writer produced them.

mod common;
use common::*;
use game_core::state::MatchEvent;

// Zone wire codes (`game_kinds::Zone`).
const FIELD: i64 = 3;
const HAND: i64 = 2;
const GRAVEYARD: i64 = 4;
const ATTACHED: i64 = 9;
// Cause wire codes (`game_kinds::ZoneChangeCause`).
const EFFECT: i64 = 3;
const HOST_LEFT: i64 = 8;
const CREATOR_LEFT: i64 = 9;
const BANKRUPTCY: i64 = 10;

// `ctx::Dest` wire codes (the state payloads the fixtures read).
const DEST_HAND: i32 = 1;
const DEST_BANISHED: i32 = 3;

fn arg(e: &MatchEvent, name: &str) -> i64 {
    int_arg(&e.msg, name).unwrap_or(-99)
}

fn keys(t: &Table, mark: i32) -> Vec<String> {
    t.keys_since(mark)
}

/// The `key` events about instance `uid`, in order.
fn about<'a>(ev: &'a [MatchEvent], key: &str, uid: i32) -> Vec<&'a MatchEvent> {
    ev.iter()
        .filter(|e| log_key(e) == key && arg(e, "uid") == uid as i64)
        .collect()
}

fn first_named<'a>(ev: &'a [MatchEvent], key: &str) -> &'a MatchEvent {
    ev.iter()
        .find(|e| log_key(e) == key)
        .unwrap_or_else(|| panic!("no {key} event"))
}

fn connected(ks: &[String], key: &str) -> usize {
    ks.iter()
        .position(|k| k == key)
        .unwrap_or_else(|| panic!("no {key} in {ks:?}"))
}

/// Keep the draw piles non-empty so a Graveyard arrival does not immediately
/// reshuffle the discard heap back into the deck.
fn deep_decks(t: &mut Table) {
    t.set_draw(0, &["通用:GREAT", "通用:GREAT", "通用:GREAT"]);
    t.set_draw(1, &["通用:GREAT", "通用:GREAT", "通用:GREAT"]);
}

/// The first field uid of `card` at `who`.
fn uid_of(t: &Table, who: usize, card: &str) -> i32 {
    t.field(who)
        .iter()
        .find(|f| f.card == card)
        .unwrap_or_else(|| panic!("{card} not on the field: {:?}", t.field(who)))
        .uid
}

// 规则书: 「此卡离场时」 -- the before hook runs on the live row and the after
// one on the landed row, both carrying the real pair and the cause.
#[test]
fn leave_raises_before_and_after_with_cause() {
    let mut t = Table::vanilla(2);
    deep_decks(&mut t);
    t.give(0, &["TEST:zoner", "TEST:drop"]);
    t.play(0, "TEST:zoner").unwrap();
    let uid = uid_of(&t, 0, "TEST:zoner");
    let mark = t.mark();
    t.play(0, "TEST:drop").unwrap();
    let ev = t.events_since(mark);
    let ks = keys(&t, mark);
    let left = connected(&ks, "zoner_left");
    let after = connected(&ks, "zoner_left_after");
    assert!(left < after, "{ks:?}");
    let before = about(&ev, "zoner_left", uid)[0];
    let post = about(&ev, "zoner_left_after", uid)[0];
    assert_eq!(arg(before, "from"), FIELD);
    assert_eq!(arg(before, "to"), GRAVEYARD);
    assert_eq!(arg(before, "cause"), EFFECT);
    assert_eq!(arg(post, "from"), FIELD);
    assert_eq!(arg(post, "to"), GRAVEYARD);
    assert_eq!(arg(post, "cause"), EFFECT);
    assert!(t.discard(0).contains(&"TEST:zoner".to_string()));
}

// 规则书: 「代替放入弃牌堆，加入手牌」-style destination clauses -- the body's
// rewrite wins over the destination the caller named.
#[test]
fn leave_destination_rewrite_wins() {
    let mut t = Table::vanilla(2);
    deep_decks(&mut t);
    t.set_state(0, "TEST.zoner.dest", DEST_HAND);
    t.give(0, &["TEST:zoner", "TEST:drop"]);
    t.play(0, "TEST:zoner").unwrap();
    t.play(0, "TEST:drop").unwrap();
    assert!(t.hand(0).contains(&"TEST:zoner".to_string()));
    assert!(!t.discard(0).contains(&"TEST:zoner".to_string()));
}

// The prop is only the *pending* destination: a value left by an earlier leave
// (here `Hand`) must not override a destination the caller names on a later
// one -- the body writes nothing this time, so the caller's `Banished` stands.
#[test]
fn stale_leave_destination_does_not_override_the_caller() {
    let mut t = Table::vanilla(2);
    deep_decks(&mut t);
    t.set_state(0, "TEST.zoner.dest", DEST_HAND);
    t.give(0, &["TEST:zoner", "TEST:drop"]);
    t.play(0, "TEST:zoner").unwrap();
    t.play(0, "TEST:drop").unwrap();
    assert!(t.hand(0).contains(&"TEST:zoner".to_string()));
    // Again, this time the caller banishes it and the body leaves the prop be.
    t.set_state(0, "TEST.zoner.dest", 0);
    t.set_state(0, "TEST.drop.dest", DEST_BANISHED);
    t.give(0, &["TEST:drop"]);
    t.play(0, "TEST:zoner").unwrap();
    t.play(0, "TEST:drop").unwrap();
    assert!(
        !t.hand(0).contains(&"TEST:zoner".to_string()),
        "the stale `Hand` did not win: {:?}",
        t.hand(0)
    );
    assert!(
        !t.discard(0).contains(&"TEST:zoner".to_string()),
        "banished, not discarded: {:?}",
        t.discard(0)
    );
}

// 规则书: 「放置于此卡上」 -- an attachment leaves before its host, with the
// host-leave cause.
#[test]
fn attachment_cascades_before_host() {
    let mut t = Table::vanilla(2);
    deep_decks(&mut t);
    t.give(0, &["TEST:holder", "TEST:zoner", "TEST:drop"]);
    t.play(0, "TEST:holder").unwrap();
    let host = uid_of(&t, 0, "TEST:holder");
    let held = t
        .field(0)
        .iter()
        .find(|f| f.on == host)
        .expect("attached")
        .uid;
    let mark = t.mark();
    t.play(0, "TEST:drop").unwrap();
    let ev = t.events_since(mark);
    let held_left = about(&ev, "zoner_left", held);
    let host_left = about(&ev, "zoner_left", host);
    // Both rows hear each raise (every placed card does), so assert on the
    // first occurrence of each subject and on their order.
    assert!(!held_left.is_empty(), "{:?}", keys(&t, mark));
    assert!(!host_left.is_empty(), "{:?}", keys(&t, mark));
    assert!(
        held_left.iter().all(|e| arg(e, "cause") == HOST_LEFT),
        "{:?}",
        keys(&t, mark)
    );
    assert_eq!(
        arg(held_left[0], "from"),
        ATTACHED,
        "the held row is Attached"
    );
    assert_eq!(arg(host_left[0], "from"), FIELD);
    assert_eq!(arg(held_left[0], "to"), GRAVEYARD);
    let at = |e: &MatchEvent| ev.iter().position(|x| std::ptr::eq(x, e)).unwrap();
    // The host's own `zoneLeft` before-hook sees the row first (I4: it may
    // rewrite the destination); the cascade then files the attachment before
    // the host row moves (I5).
    assert!(at(host_left[0]) < at(held_left[0]));
    let d = t.discard(0);
    let held_at = d
        .iter()
        .position(|c| c == "TEST:zoner")
        .expect("held filed");
    let host_at = d
        .iter()
        .position(|c| c == "TEST:holder")
        .expect("host filed");
    assert!(held_at < host_at, "attachment file first: {d:?}");
    assert!(t.hand(0).is_empty());
}

// 规则书: 「由该卡生成的卡」 dies with its creator, depth first.
#[test]
fn creator_child_dies_with_creator() {
    let mut t = Table::vanilla(2);
    deep_decks(&mut t);
    t.give(0, &["TEST:creator", "TEST:drop"]);
    t.play(0, "TEST:creator").unwrap();
    assert_eq!(
        t.field(0).len(),
        3,
        "root and two descendants: {:?} {:?}",
        t.field(0),
        t.recent_keys(40)
    );
    // Root, its child and the grandchild, in creation order.
    let rows = t.field(0);
    let mark = t.mark();
    t.play(0, "TEST:drop").unwrap();
    let ev = t.events_since(mark);
    let root = rows.iter().find(|f| f.created_by < 0).expect("root").uid;
    let child = rows
        .iter()
        .find(|f| f.created_by == root)
        .expect("child")
        .uid;
    let grandchild = rows
        .iter()
        .find(|f| f.created_by == child)
        .expect("grandchild")
        .uid;
    let e = |uid: i32| about(&ev, "zoner_left", uid);
    assert!(!e(child).is_empty(), "{:?}", keys(&t, mark));
    assert!(e(child).iter().all(|x| arg(x, "cause") == CREATOR_LEFT));
    assert_eq!(arg(e(child)[0], "from"), FIELD);
    assert_eq!(arg(e(child)[0], "to"), GRAVEYARD);
    assert!(!e(grandchild).is_empty(), "{:?}", keys(&t, mark));
    assert!(e(grandchild)
        .iter()
        .all(|x| arg(x, "cause") == CREATOR_LEFT));
    assert_eq!(arg(e(root)[0], "cause"), EFFECT);
    // Preorder: the root's own leave, then the cascade down its children.
    let at = |uid: i32| ev.iter().position(|x| std::ptr::eq(x, e(uid)[0])).unwrap();
    assert!(at(root) < at(child) && at(child) < at(grandchild));
    assert!(t.field(0).is_empty(), "{:?}", t.field(0));
}

// A card already on the field is re-parented; the last hop back would close a
// cycle and is refused.
#[test]
fn attach_cycle_rejected() {
    let mut t = Table::vanilla(2);
    t.give(0, &["TEST:cycler"]);
    t.play(0, "TEST:cycler").unwrap();
    let ev = t.events_since(0);
    let e = first_named(&ev, "cycler_attach");
    assert!(arg(e, "first") >= 0, "the row attach is admitted");
    assert_eq!(arg(e, "second"), -1, "the cycle back is refused");
    assert_eq!(t.field(0).len(), 2);
    assert_eq!(t.field(0).iter().filter(|f| f.on >= 0).count(), 1);
}

// 「加入手牌时」 fires for an uncounted arrival only.
#[test]
fn hand_changed_counted_only_for_draws() {
    let mut t = Table::vanilla(2);
    deep_decks(&mut t);
    t.set_draw(0, &["TEST:zoner", "TEST:zoner"]);
    t.give(
        0,
        &[
            "TEST:zoner",
            "TEST:drawer",
            "TEST:tutor",
            "TEST:zoner",
            "TEST:drop",
        ],
    );
    // 1) a draw: counted.
    t.play(0, "TEST:drawer").unwrap();
    // 2) a tutor from the deck: uncounted.
    t.play(0, "TEST:tutor").unwrap();
    // 3) a bounce off the field: uncounted.
    t.play(0, "TEST:zoner").unwrap();
    t.set_state(0, "TEST.drop.dest", DEST_HAND);
    t.play(0, "TEST:drop").unwrap();
    let ev = t.events_since(0);
    let changed: Vec<i64> = ev
        .iter()
        .filter(|e| log_key(e) == "zoner_hand_changed")
        .map(|e| arg(e, "counted"))
        .collect();
    assert!(changed.len() >= 2, "{:?}", keys(&t, 0));
    // The opener ZONER hears each arrival of its own name: the draw (counted)
    // and the tutor (not); the bounce lands on the played ZONER instead.
    assert_eq!(changed[0], 1, "the draw is counted: {changed:?}");
    assert_eq!(changed[1], 0, "the tutor is not: {changed:?}");
}

// A waste arrival carries the bit into `discarded`; a plain one does not.
#[test]
fn wasted_only_when_marked() {
    let mut t = Table::vanilla(2);
    deep_decks(&mut t);
    t.give(0, &["TEST:zoner", "TEST:drop", "TEST:waster"]);
    t.play(0, "TEST:zoner").unwrap();
    let mark = t.mark();
    // The waster marks itself and leaves for the Graveyard in its own play.
    t.play(0, "TEST:waster").unwrap();
    let zoner_uid = t.state(0, "TEST.drop.uid");
    let waster_uid = t.state(0, "TEST.waster.uid");
    t.play(0, "TEST:drop").unwrap();
    let ev = t.events_since(mark);
    let marks_of = |uid: i64| -> Vec<i64> {
        ev.iter()
            .filter(|e| log_key(e) == "zoner_discarded" && arg(e, "uid") == uid)
            .map(|e| arg(e, "wasted"))
            .collect()
    };
    let plain = marks_of(zoner_uid as i64);
    let waste = marks_of(waster_uid as i64);
    assert!(
        plain.contains(&0),
        "the plain discard: {:?}",
        keys(&t, mark)
    );
    assert!(
        waste.contains(&1),
        "the waster is wasted: {:?}",
        keys(&t, mark)
    );
}

// A negated play still announces itself, with the cancelled outcome.
#[test]
fn play_outcome_cancelled() {
    let mut t = Table::vanilla(2);
    t.place_raw(1, "TEST:observer");
    t.give(1, &["TEST:negator"]);
    t.give(0, &["TEST:outcomer"]);
    t.play(0, "TEST:outcomer").unwrap();
    // Other prompts (a bound skill's turn-start choice) can stand in front of
    // the counteraction window; decline through them.
    for _ in 0..6 {
        if t.counteract_offered("TEST:negator") {
            break;
        }
        assert!(t.prompt().is_some(), "{}", t.dump_prompt());
        t.decline();
    }
    assert!(t.counteract_offered("TEST:negator"), "{}", t.dump_prompt());
    t.counteract(1, "TEST:negator").unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    let ev = t.events_since(0);
    let outs: Vec<i64> = ev
        .iter()
        .filter(|e| log_key(e) == "observer_outcome")
        .map(|e| arg(e, "outcome"))
        .collect();
    assert!(!outs.is_empty(), "{:?}", keys(&t, 0));
    assert!(outs.iter().all(|&o| o == 1), "cancelled: {outs:?}");
    assert!(!t.on_field(0, "TEST:outcomer"), "the play was negated");
}

// The opening deal raises one `openingHandDealt` per dealt card, before the
// mulligan ask.
#[test]
fn opening_deal_events() {
    let t = Table::with_opening_draw(&["户山香澄", "美竹兰"], &["TEST:zoner"]);
    let ev = t.events_since(0);
    let dealt: Vec<&MatchEvent> = ev.iter().filter(|e| log_key(e) == "zoner_dealt").collect();
    assert_eq!(dealt.len(), 1, "{:?}", t.recent_keys(30));
    assert_eq!(arg(dealt[0], "from"), 1, "from the deck");
    assert_eq!(arg(dealt[0], "to"), HAND);
    assert_eq!(arg(dealt[0], "counted"), 1);
    let keys_now = t.recent_keys(40);
    let dealt_pos = keys_now.iter().position(|k| k == "zoner_dealt").unwrap();
    let mulligan = keys_now
        .iter()
        .position(|k| k.contains("mulligan"))
        .expect("a mulligan line");
    assert!(dealt_pos < mulligan, "the deal precedes the ask");
}

// A face-down card still hears its own flip hooks.
#[test]
fn flip_window_prompts_and_face_down_self_runs() {
    let mut t = Table::vanilla(2);
    t.give(0, &["TEST:flipper"]);
    t.play(0, "TEST:flipper").unwrap();
    let uid = uid_of(&t, 0, "TEST:flipper");
    let ev = t.events_since(0);
    // The card's own def answers by its name as well as by its instance while
    // the play that placed it is still on the stack; the instance runs are the
    // ones that matter.
    let flips: Vec<i64> = about(&ev, "zoner_flipped", uid)
        .iter()
        .map(|e| arg(e, "face"))
        .collect();
    assert_eq!(
        flips,
        vec![1, 0],
        "down first, up second: {:?}",
        keys(&t, 0)
    );
    let afters = about(&ev, "zoner_flipped_after", uid).len();
    assert_eq!(afters, 2);
}

// A bankruptcy's field clear is one leave per card, cause `Bankruptcy`.
#[test]
fn bankruptcy_field_clear_is_per_card() {
    let mut t = Table::vanilla(2);
    deep_decks(&mut t);
    t.give(0, &["TEST:zoner", "TEST:zoner"]);
    t.play(0, "TEST:zoner").unwrap();
    t.play(0, "TEST:zoner").unwrap();
    assert_eq!(t.field(0).len(), 2);
    t.set_money(0, 0);
    t.own(1, &[tile("购物中心")]);
    t.set_houses(tile("购物中心"), 5);
    t.begin_turn(0);
    t.set_pos(0, tile("购物中心") - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    assert!(t.p(0).bankrupt, "{:?}", t.recent_keys(20));
    let ev = t.events_since(0);
    let causes: Vec<i64> = ev
        .iter()
        .filter(|e| log_key(e) == "zoner_left")
        .map(|e| arg(e, "cause"))
        .collect();
    assert!(causes.len() >= 2, "one per field card: {causes:?}");
    assert!(
        causes.iter().all(|&c| c == BANKRUPTCY),
        "bankruptcy cause: {causes:?}"
    );
    assert!(t.field(0).is_empty());
}

// A rehome keeps every row on the tile: the rule's uid, its attachment and
// the attachment's bits all survive the owner change.
#[test]
fn rehome_keeps_row_identity() {
    let mut t = Table::vanilla(2);
    t.give(0, &["TEST:brd", "TEST:zoner", "TEST:rehomer"]);
    t.play(0, "TEST:brd").unwrap();
    t.m.world_mut().set_tile_owner(0, 0);
    let board = t.m.world().field_instances(game_core::state::BOARD_OWNER);
    let host = board.iter().find(|(_, c)| c == "TEST:brd").unwrap().0;
    let held = t.m.world().attached_cards(host)[0];
    t.play(0, "TEST:rehomer").unwrap();
    assert_eq!(t.owner(0), Some(1), "the tile changed hands");
    let after =
        t.m.world()
            .field_by_uid(held)
            .expect("the held row survives");
    assert_eq!(after.uid, held);
    assert_eq!(after.on, host, "still attached");
    let rule = t.m.world().field_by_uid(host).expect("the rule survives");
    assert_eq!(rule.uid, host);
}

// 「你最早获得的格子」 follows the acquisition order, not the tile order.
#[test]
fn earliest_owned_tile_follows_acquisition() {
    let mut t = Table::vanilla(2);
    t.give(0, &["TEST:tileProbe"]);
    let b = tile("富士见坂");
    let a = tile("星之鼓动山丘");
    t.m.world_mut().set_tile_owner(b as i32, 0);
    t.m.world_mut().set_tile_owner(a as i32, 0);
    t.m.world_mut().set_tile_owner(b as i32, -1);
    t.play(0, "TEST:tileProbe").unwrap();
    let ev = t.events_since(0);
    let e = first_named(&ev, "tile_probe");
    assert_eq!(arg(e, "tile"), a as i64);
}

// A board-owned host's attachments fall to the Graveyard.
#[test]
fn board_host_attachment_falls_to_graveyard() {
    let mut t = Table::vanilla(2);
    deep_decks(&mut t);
    t.give(0, &["TEST:brd", "TEST:zoner", "TEST:brdOff"]);
    t.play(0, "TEST:brd").unwrap();
    let board = t.m.world().field_instances(game_core::state::BOARD_OWNER);
    let host = board.iter().find(|(_, c)| c == "TEST:brd").unwrap().0;
    let held = t.m.world().attached_cards(host)[0];
    let mark = t.mark();
    t.play(0, "TEST:brdOff").unwrap();
    let ev = t.events_since(mark);
    let left = about(&ev, "zoner_left", held);
    assert_eq!(left.len(), 1, "{:?}", keys(&t, mark));
    assert_eq!(arg(left[0], "to"), GRAVEYARD);
    assert_eq!(arg(left[0], "cause"), HOST_LEFT);
    assert!(t.discard(0).contains(&"TEST:zoner".to_string()));
    assert!(
        !t.m.world()
            .field_instances(game_core::state::BOARD_OWNER)
            .iter()
            .any(|(u, _)| *u == host),
        "the board rule is gone"
    );
}

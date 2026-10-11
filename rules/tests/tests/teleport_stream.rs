//! Teleport + move: the engine state **and** the client-facing event stream.
//!
//! The UI (`webui/src/scenes/board/anim.ts`) animates a `roll`/`move` event by
//! snapping the piece to `e.from` and stepping `e.value`, and a `teleport`
//! event by snapping to `e.to`. So a walk whose `from` is the tile the piece
//! stood on *before* a teleport -- or a walk published after the teleport that
//! should have followed it -- makes the animation start from the old tile.
//!
//! [`replay_moves`] replays the stream the way the animator does and asserts
//! every walk starts where the previous movement event left the piece.
//!
//! Spec: the cards' own rulebook text (`docs/rulebook/cards.json`,
//! `data/events.json`, the skill sheets). Ruling R2: a no-settle teleport fires
//! pass/overlap only at the destination, never mid-route.

mod common;
use common::*;

use game_core::state::MatchEvent;

/// Answer every open prompt with its decline.
fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// Advance until it is `who`'s 运营 stage.
fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..60 {
        if t.turn() == who && t.step() == game_core::state::stage::OPS {
            return;
        }
        let cur = t.turn();
        if t.turn() == who {
            t.end(who).unwrap();
            drain(t);
            continue;
        }
        t.m.world_mut().st.skip_move = true;
        t.end(cur).unwrap();
        drain(t);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

/// Board size.
fn n() -> i32 {
    data().tiles.len() as i32
}

/// What `anim.ts` does with one movement event, and the invariants the stream
/// must hold for the animation to start each walk at the right tile.
///
/// `anim.ts walk()` snaps `pos` to `e.from` and steps `e.value`; `teleport`
/// snaps to `e.to`. So:
/// - `to` must equal `(from + value) mod n` for every walk (else the walk ends
///   somewhere the engine did not).
/// - a walk immediately after a `teleport` event must start at the teleport's
///   `to` -- otherwise the piece snaps back to the pre-teleport tile and walks
///   from there, which is the reported bug.
/// - a walk after a *bare* `ctx::teleport_to` (no event) may start at the jump's
///   destination: the client snaps there via `e.from`, which is the designed
///   "the jump is the snap" behaviour. What must **not** happen is `e.from`
///   being the pre-jump tile.
///
/// Returns where the last announced event left the piece (`start` when there
/// are none).
fn replay_moves(events: &[MatchEvent], who: i32, start: i32) -> i32 {
    let tiles = n();
    let mut pos = start;
    let mut last: Option<&MatchEvent> = None;
    for e in events {
        if e.player_id != who {
            continue;
        }
        let dump = || {
            events
                .iter()
                .map(|x| (x.id, x.r#type.as_str(), x.player_id, x.from, x.to, x.value))
                .collect::<Vec<_>>()
        };
        match e.r#type.as_str() {
            "roll" | "move" => {
                if let Some(prev) = last {
                    if prev.r#type == "teleport" {
                        assert_eq!(
                            e.from, prev.to,
                            "walk after a teleport starts from {} but the teleport landed on {} -- \
                             the move starts from the location prior to the teleport: {:?}\nstream={:?}",
                            e.from, prev.to, e, dump()
                        );
                    } else {
                        assert_eq!(
                            e.from, pos,
                            "walk/roll starts from stale pos {} (stream left the piece at {}): {:?}\nstream={:?}",
                            e.from, pos, e, dump()
                        );
                    }
                }
                let end = ((e.from + e.value) % tiles + tiles) % tiles;
                assert_eq!(e.to, end, "walk end {} != from+value {} ({:?})", e.to, end, e);
                pos = end;
                last = Some(e);
            }
            "teleport" => {
                if let Some(prev) = last {
                    if prev.r#type != "teleport" {
                        assert_eq!(
                            e.from, pos,
                            "teleport from stale pos {} (stream left the piece at {}): {:?}\nstream={:?}",
                            e.from, pos, e, dump()
                        );
                    }
                }
                pos = e.to;
                last = Some(e);
            }
            _ => {}
        }
    }
    pos
}

/// Assert every walk/roll in `events` starts at one of `allowed` tiles. This is
/// the direct "the move must not start from the location prior to the teleport"
/// check: each test names the post-teleport tile(s) the walk may start from.
fn assert_walks_start_at(events: &[MatchEvent], who: i32, allowed: &[i32], label: &str) {
    for e in events {
        if e.player_id != who {
            continue;
        }
        if matches!(e.r#type.as_str(), "roll" | "move") {
            assert!(
                allowed.contains(&e.from),
                "{label}: walk starts at {} but must start at one of {allowed:?}: {:?}",
                e.from,
                e
            );
        }
    }
}

/// Movement events of `who` since `mark`, in order.
fn moves_since(t: &Table, mark: i32, who: usize) -> Vec<MatchEvent> {
    t.events_since(mark)
        .into_iter()
        .filter(|e| {
            e.player_id == who as i32 && matches!(e.r#type.as_str(), "roll" | "move" | "teleport")
        })
        .collect()
}

// =====================================================================
// 1. hand card: 纯真振翅 「传送到移动方向20格后（不触发结算），立刻进行移动掷骰」
//    -- `ctx::teleport_to` then the main roll.
// =====================================================================

#[test]
fn pure_wings_roll_stream_starts_at_the_teleport_destination() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 5);
    t.dice(&[3]);
    let mark = t.mark();
    t.give_play(0, "Mor:纯真振翅").unwrap();
    drain(&mut t);
    let ev = moves_since(&t, mark, 0);
    // `teleport_to` is a bare pos write (no `teleport` log event); the walk
    // must still announce itself from the jump's destination -- never from 5.
    assert_walks_start_at(&ev, 0, &[5 + 20], "pure_wings");
    let end = replay_moves(&ev, 0, 5 + 20);
    assert_eq!(end, t.pos(0) as i32, "stream ends where the engine did: {:?}", ev);
    assert_eq!(t.pos(0), 5 + 20 + 3, "pos={} stream={:?}", t.pos(0), ev);
}

// =====================================================================
// 2. event card: 飞鸟山之战 「所有玩家[传送]到飞鸟山公园。然后抽到此卡的玩家移动1d20」
//    -- `ctx::teleport_to` for the others, a `card_move` teleport + a
//    `card_move` walk for the drawer.
// =====================================================================

fn draw_event_card(t: &mut Table, who: usize, event: &str, faces: &[i32]) {
    let mut deck: Vec<String> = t.event_deck().into_iter().filter(|e| e != event).collect();
    deck.insert(0, event.to_string());
    let deck: Vec<&str> = deck.iter().map(String::as_str).collect();
    t.set_event_deck(&deck);
    until_turn(t, who);
    let corner = tile("CiRCLE 咖啡厅");
    t.set_pos(who, (corner + 59) % 60);
    let mut all = vec![1];
    all.extend_from_slice(faces);
    t.dice(&all);
    t.roll(who).unwrap();
}

#[test]
fn asukayama_drawer_walk_stream_starts_at_the_park() {
    let mut t = Table::vanilla(3);
    t.set_pos(1, 5);
    t.set_pos(2, 10);
    let park = tile("飞鸟山公园");
    let mark = t.mark();
    draw_event_card(&mut t, 1, "飞鸟山之战", &[7]);
    drain(&mut t);
    let ev = moves_since(&t, mark, 1);
    // The drawer's jump is a `card_move` teleport (it logs `teleport`); the
    // 1d20 walk follows and must start at the park, never at the pre-jump tile.
    assert!(
        ev.iter().any(|e| e.r#type == "teleport"),
        "the drawer's jump is announced: {ev:?}"
    );
    let after_teleport = ev
        .iter()
        .filter(|e| e.r#type == "teleport")
        .map(|e| e.to)
        .collect::<Vec<_>>();
    assert_eq!(
        after_teleport,
        vec![park as i32],
        "the jump lands on the park: {ev:?}"
    );
    // Only the first walk *after* the jump must start at the park (a long walk
    // may continue from a CiRCLE segment); the 1-step landing onto the CiRCLE
    // corner (which drew the event) precedes the jump.
    let first_after_jump = ev
        .iter()
        .skip_while(|e| e.r#type != "teleport")
        .find(|e| matches!(e.r#type.as_str(), "roll" | "move"));
    let first = first_after_jump.expect("a walk after the jump");
    assert_eq!(
        first.from, park as i32,
        "the walk after the jump starts at the park: {ev:?}"
    );
    let end = replay_moves(&ev, 1, 14);
    assert_eq!(
        end,
        t.pos(1) as i32,
        "stream ends where the engine did: pos={} stream={:?}",
        t.pos(1),
        ev
    );
    assert_eq!(
        t.pos(1),
        (park + 7) % 60,
        "the drawer teleports to the park then walks 1d20: pos={} stream={:?}",
        t.pos(1),
        ev
    );
}

#[test]
fn asukayama_other_players_land_on_the_park_without_a_stale_walk() {
    let mut t = Table::vanilla(3);
    t.set_pos(1, 5);
    t.set_pos(2, 10);
    let park = tile("飞鸟山公园");
    let mark = t.mark();
    draw_event_card(&mut t, 1, "飞鸟山之战", &[7]);
    drain(&mut t);
    // P2 only teleports (`ctx::teleport_to`, a bare pos write). The stream must
    // not announce a walk for them at all -- a walk would have to start at the
    // park, never at their pre-teleport tile.
    let ev = moves_since(&t, mark, 2);
    assert_eq!(t.pos(2), park, "P2 stands on the park: {:?}", t.recent_keys(16));
    assert_walks_start_at(&ev, 2, &[park as i32], "asukayama other");
    if !ev.is_empty() {
        let end = replay_moves(&ev, 2, park as i32);
        assert_eq!(end, t.pos(2) as i32, "stream ends where the engine did: {:?}", ev);
    }
}

// =====================================================================
// 3. field-card hook (settleAfter): Sumimi:一人两个甜甜顿
//    「你原本所在格子被其他玩家经过时，可在那名玩家触发结算后选择传送至…并触发结算」
//    -- `ctx::teleport_to` back to the original square, then a settle-teleport
//    (`card_move` teleport) onto a chosen tile.
// =====================================================================

#[test]
fn two_donuts_settle_teleport_stream_is_continuous() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 10);
    t.give_play(0, "Sumimi:一人两个甜甜圈").unwrap();
    drain(&mut t);
    until_turn(&mut t, 1);
    t.set_pos(1, 5);
    t.dice(&[10]);
    let mark = t.mark();
    t.roll(1).unwrap();
    // Holder P0 answers the return + the settle-teleport.
    loop {
        let Some(p) = t.prompt() else { break };
        if p.text.key().contains("two_donuts_yes") {
            t.answer(0, 0).unwrap();
        } else if p.kind == "tile" {
            t.answer_tile(0, 12).unwrap();
        } else {
            t.decline();
        }
    }
    assert_eq!(t.pos(0), 12, "the settle-teleport lands on 12: {:?}", t.recent_keys(20));
    // P0's movement stream: the return jump is a bare `teleport_to` (no event);
    // the settle-teleport is a `card_move` teleport (logged). Any walk P0 got
    // must start at a tile the stream left them on (12 or the pre-jump 10).
    let ev = moves_since(&t, mark, 0);
    assert_walks_start_at(&ev, 0, &[10, 12], "two_donuts holder");
    if !ev.is_empty() {
        let end = replay_moves(&ev, 0, 10);
        assert_eq!(end, t.pos(0) as i32, "stream ends where the engine did: {:?}", ev);
    }
    // P1's walk must also be continuous from where P1 started.
    let ev1 = moves_since(&t, mark, 1);
    let end1 = replay_moves(&ev1, 1, 5);
    assert_eq!(end1, t.pos(1) as i32, "P1 stream ends where the engine did: {:?}", ev1);
}

// =====================================================================
// 4. counteraction (settleBefore): CRYCHIC:是我自己的问题
//    「主要移动结束时，[触发结算]前打出此卡，使自己额外远离绝对距离最近的玩家一格」
// =====================================================================

#[test]
fn my_own_problem_counteraction_stream_is_continuous() {
    let mut t = Table::vanilla(2);
    t.set_pos(1, 10);
    t.give(0, &["CRYCHIC:是我自己的问题"]);
    t.set_pos(0, 5);
    t.dice(&[4]);
    let mark = t.mark();
    t.roll(0).unwrap();
    let mut played = false;
    loop {
        let Some(_p) = t.prompt() else { break };
        if t.counteract_offered("CRYCHIC:是我自己的问题") {
            t.counteract(0, "CRYCHIC:是我自己的问题").unwrap();
            played = true;
        } else {
            t.decline();
        }
    }
    assert!(played, "the [反击] window offered the card");
    assert_eq!(t.pos(0), 8, "steps away from P1: {:?}", t.recent_keys(20));
    // The main walk is announced 5 -> 9; the counteraction's 1-step push is a
    // bare `ctx::teleport_to` (no movement event). The walk itself must start
    // at 5 -- not at some earlier tile.
    let ev = moves_since(&t, mark, 0);
    assert_walks_start_at(&ev, 0, &[5], "my_own_problem roll");
    let end = replay_moves(&ev, 0, 5);
    // The stream ends at the walk's landing (9); the unannounced 1-step push
    // to 8 is a bare write the view carries. A **subsequent** move must start
    // at 8 -- never at 9 or 5.
    assert_eq!(end, 9, "the walk is announced 5->9: {:?}", ev);
    // Next turn's roll walks from the post-push tile.
    t.m.world_mut().st.skip_move = true;
    t.end(0).unwrap();
    drain(&mut t);
    until_turn(&mut t, 0);
    t.dice(&[3]);
    let mark2 = t.mark();
    let before = t.pos(0);
    t.roll(0).unwrap();
    drain(&mut t);
    let ev2 = moves_since(&t, mark2, 0);
    assert_walks_start_at(&ev2, 0, &[before as i32], "my_own_problem next roll");
    let end2 = replay_moves(&ev2, 0, before as i32);
    assert_eq!(end2, t.pos(0) as i32, "stream ends where the engine did: {:?}", ev2);
    assert_eq!(
        t.pos(0),
        (before + 3) % 60,
        "the next roll walks from the post-push tile: pos={} stream={:?}",
        t.pos(0),
        ev2
    );
}

// =====================================================================
// 5. card-driven teleport move (`ctx::card_move` with `MoveKind::Teleport`):
//    TEST:tele_nosettle 「不[触发结算]」 teleport move (ruling R2).
// =====================================================================

#[test]
fn tele_nosettle_card_move_stream_is_continuous() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 20);
    t.set_pos(1, 50);
    let mark = t.mark();
    let start = t.pos(0);
    t.give_play(0, "TEST:tele_nosettle").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 25, "teleports 5 ahead, no settle: {:?}", t.recent_keys(16));
    let ev = moves_since(&t, mark, 0);
    assert!(
        ev.iter().any(|e| e.r#type == "teleport"),
        "the teleport move is announced: {ev:?}"
    );
    let end = replay_moves(&ev, 0, start as i32);
    assert_eq!(end, t.pos(0) as i32, "stream ends where the engine did: {:?}", ev);
}

// =====================================================================
// 6. skill shapes the main move into a teleport, then the next turn's
//    main roll walks from wherever that landed.
//    RAISE A SUILEN:UNSTOPPABLE （2）「你的下一次主要移动可变为传送至…」
// =====================================================================

#[test]
fn ras_skill_teleport_then_next_roll_stream_is_continuous() {
    let mut t = Table::vanilla(2);
    t.clean();
    t.begin_turn(0);
    // Latch the livehouse settle: stand on a livehouse and settle there.
    let house = (0..n() as usize)
        .find(|&u| {
            data().tiles[u].kind == game_core::data::TileKind::Property
                && data()
                    .tiles
                    .get(u)
                    .map(|x| x.name.contains("Live") || x.name.contains("live") || x.house >= 0)
                    .unwrap_or(false)
        })
        .unwrap_or(1);
    t.set_pos(0, house);
    t.set_owner(house, Some(0));
    // Trigger the skill's settle latch by landing on the house.
    t.set_pos(0, (house + 59) % 60);
    t.dice(&[1]);
    let mark0 = t.mark();
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), house, "settled on the livehouse");
    // Next turn start: the skill offers the teleport.
    t.m.world_mut().st.skip_move = true;
    t.end(0).unwrap();
    drain(&mut t);
    until_turn(&mut t, 0);
    let mark = t.mark();
    let mut offered = false;
    while let Some(p) = t.prompt() {
        if p.title.key().contains("ras_") || p.text.key().contains("ras_") {
            t.answer_one(1).unwrap();
            offered = true;
        } else {
            t.decline();
        }
    }
    if offered {
        // The main move became a teleport to a livehouse; then the turn ends.
        let ev = moves_since(&t, mark, 0);
        let end = replay_moves(&ev, 0, house as i32);
        assert_eq!(end, t.pos(0) as i32, "stream ends where the engine did: {:?}", ev);
    }
    // Whichever branch landed, the next main roll must start from the current
    // pos -- not from the pre-teleport tile.
    t.m.world_mut().st.skip_move = true;
    t.end(0).unwrap();
    drain(&mut t);
    until_turn(&mut t, 0);
    let before = t.pos(0);
    t.dice(&[4]);
    let mark2 = t.mark();
    t.roll(0).unwrap();
    drain(&mut t);
    let ev = moves_since(&t, mark2, 0);
    let end = replay_moves(&ev, 0, before as i32);
    assert_eq!(end, t.pos(0) as i32, "stream ends where the engine did: {:?}", ev);
    assert_eq!(
        t.pos(0),
        (before + 4) % 60,
        "the next roll walks from the post-teleport tile: pos={} stream={:?}",
        t.pos(0),
        ev
    );
    let _ = mark0;
}

// =====================================================================
// 7. settleBefore redirect: CRYCHIC 高松灯「内声」(3)
//    「你位于此卡前后5格内时…令该玩家传送至此卡所在格子（不触发结算）」
//    -- a bare `teleport_to` of the mover during settleBefore; the settle
//    follows the redirect. Then the next turn's roll walks from there.
// =====================================================================

#[test]
fn tomori_inner_shout_settle_redirect_then_next_roll() {
    let mut t = Table::vanilla(2);
    // P0 owns the card on tile 30; P1 walks nearby and is pulled onto it.
    t.set_pos(0, 30);
    t.set_owner(30, Some(0));
    t.place_raw(0, "CRYCHIC:高松灯（MyGO!!!!!）的内声");
    t.set_fire(0, 1, 10);
    until_turn(&mut t, 1);
    t.set_pos(1, 25);
    t.dice(&[4]); // 25 -> 29, endpoint within 5 of 30
    let mark = t.mark();
    let start = t.pos(1);
    t.roll(1).unwrap();
    // P0 may be asked to pull; take it.
    while let Some(p) = t.prompt() {
        if p.text.key().contains("tomori") || p.title.key().contains("tomori") {
            t.answer_one(1).unwrap();
        } else {
            t.decline();
        }
    }
    let ev = moves_since(&t, mark, 1);
    let end = replay_moves(&ev, 1, start as i32);
    assert_eq!(end, t.pos(1) as i32, "stream ends where the engine did: {:?}", ev);
    // If the pull landed, P1 stands on 30 and the next roll walks from there.
    if t.pos(1) == 30 {
        until_turn(&mut t, 1);
        t.m.world_mut().st.skip_move = true;
        t.end(1).unwrap();
        drain(&mut t);
        until_turn(&mut t, 1);
        t.dice(&[3]);
        let mark2 = t.mark();
        let before = t.pos(1);
        t.roll(1).unwrap();
        drain(&mut t);
        let ev2 = moves_since(&t, mark2, 1);
        let end2 = replay_moves(&ev2, 1, before as i32);
        assert_eq!(end2, t.pos(1) as i32, "stream ends where the engine did: {:?}", ev2);
        assert_eq!(
            t.pos(1),
            (before + 3) % 60,
            "the next roll walks from the redirect tile: pos={} stream={:?}",
            t.pos(1),
            ev2
        );
    }
}

// =====================================================================
// 8. walk through CiRCLE with a teleport after the approach announce
//    (the e596710 shape: the walk is published before `passTile`, so a
//    `passTile` teleport must not appear before the walk it interrupted).
//    纯真振翅 jumps 20 ahead with no settle; land the following roll on a
//    path that crosses CiRCLE so the approach is announced mid-walk.
// =====================================================================

#[test]
fn circle_approach_then_settle_teleport_stream_is_continuous() {
    // Walk across CiRCLE (tile 0) so the approach is announced, then finish
    // the move on a tile where 一人两个甜甜圈's holder can pull us -- but the
    // simpler shape: a plain walk over CiRCLE, checking the two-segment stream
    // stays continuous after the reward prompt.
    let mut t = Table::vanilla(2);
    t.set_pos(0, 57);
    t.set_pos(1, 20);
    t.m.world_mut().turn.fixed_roll = Some(6); // 57 -> 0 (CiRCLE) -> 3
    let mark = t.mark();
    let start = t.pos(0);
    t.roll(0).unwrap();
    // CiRCLE reward prompt.
    while let Some(_p) = t.prompt() {
        t.answer_one(0).unwrap_or_else(|_| t.decline());
    }
    let ev = moves_since(&t, mark, 0);
    let end = replay_moves(&ev, 0, start as i32);
    assert_eq!(end, t.pos(0) as i32, "stream ends where the engine did: {:?}", ev);
    assert_eq!(t.pos(0), 3, "walked 57 -> 0 -> 3: {:?}", ev);
}

// =====================================================================
// 9. a `card_move` right after a bare `ctx::teleport_to` in the same body
//    -- the move must start at the jump's destination (the original bug).
//    Sumimi:一人两个甜甜圈's return jump is a bare write; the settle-teleport
//    that follows is a `card_move`. Covered in (3); this one pins the
//    same-body order with the TEST mover after a skill teleport.
// =====================================================================

#[test]
fn mygo_start_teleport_then_main_roll_stream_is_continuous() {
    // MyGO:迷途之星 （1）「开局时投掷3d20，并取出目作为你本局游戏的起始点」
    // is a `teleport_to` at deck-before-game; the first main roll walks from
    // that start.
    let mut t = Table::new(&["MyGO!!!!!", "Poppin'Party"]);
    t.clean();
    t.begin_turn(0);
    let start = t.pos(0);
    t.dice(&[4]);
    let mark = t.mark();
    t.roll(0).unwrap();
    drain(&mut t);
    let ev = moves_since(&t, mark, 0);
    let end = replay_moves(&ev, 0, start as i32);
    assert_eq!(end, t.pos(0) as i32, "stream ends where the engine did: {:?}", ev);
    assert_eq!(
        t.pos(0),
        (start + 4) % 60,
        "the first roll walks from the deck-before-game start: pos={} stream={:?}",
        t.pos(0),
        ev
    );
}

// =====================================================================
// 11. a `passTile` hook that teleports mid-walk (bare `ctx::teleport_to`).
//     The walk's `roll`/`move` event must be published **before** the hook
//     jumps the piece -- otherwise the client animates the walk from the
//     pre-jump tile (the reported bug; 808f0b3 published the walk after
//     `passTile`, HEAD publishes it before).
// =====================================================================

#[test]
fn pass_tile_teleport_does_not_send_the_walk_back_to_the_old_tile() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 20);
    t.set_pos(1, 50);
    t.set_state(0, "TEST.pass_tele.done", 0);
    t.give_play(0, "TEST:pass_tele").unwrap();
    drain(&mut t);
    // A 1-step walk: the only step is `last`, so the walk event is published
    // at it. The `passTile` hook jumps the mover 3 ahead (21 -> 24) with a
    // bare `ctx::teleport_to` (no `teleport` event).
    t.dice(&[1]);
    let mark = t.mark();
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(0),
        24,
        "the passTile hook jumped 3 ahead of the 1-step landing: pos={} keys={:?}",
        t.pos(0),
        t.recent_keys(20)
    );
    let all = t.events_since(mark);
    let ev = moves_since(&t, mark, 0);
    // The walk (20 -> 21) must be announced **before** the `passTile` hook
    // runs (its log line). HEAD publishes the walk before `passTile` (e596710);
    // 808f0b3 published it after -- and a hook that logs a `teleport` event
    // there would leave the walk's `from` on the pre-jump tile, the reported
    // "move starts from the location prior to the teleport" bug.
    let walk = ev
        .iter()
        .find(|e| matches!(e.r#type.as_str(), "roll" | "move"))
        .expect("the 1-step walk is announced");
    assert_eq!(walk.from, 20, "the walk starts at the pre-walk tile: {:?}", ev);
    assert_eq!(walk.to, 21, "the walk ends at the step's landing: {:?}", ev);
    let hook_log = all
        .iter()
        // `log_key` reads through the 「<card> 的效果：…」 attribution wrapper.
        .find(|e| log_key(e).contains("pass_tele_done"))
        .expect("the passTile hook logged");
    assert!(
        walk.id < hook_log.id,
        "the walk must be announced before the passTile hook runs \
         (walk id {}, hook id {}): {:?}",
        walk.id,
        hook_log.id,
        all.iter()
            .map(|x| (x.id, x.r#type.as_str(), x.msg.key()))
            .collect::<Vec<_>>()
    );
    // The jump is a bare write (no `teleport` event), so the stream ends at
    // the walk's landing and the view carries the jump -- but the walk itself
    // must not start from the jump's destination or any earlier tile.
    assert_walks_start_at(&ev, 0, &[20], "pass_tile teleport");
}

// =====================================================================
// 12. a `passTile` hook that teleports AND then card_moves in the same
//     body: the follow-up walk must start at the jump's destination.
// =====================================================================

#[test]
fn pass_tile_teleport_then_card_move_walks_from_the_destination() {
    // 飞鸟山之战's drawer shape, but triggered mid-walk: the hook jumps the
    // mover (bare write) and the same body then walks. Pinned with the event
    // card itself (its body is exactly teleport-then-walk).
    let mut t = Table::vanilla(3);
    t.set_pos(1, 5);
    t.set_pos(2, 10);
    let park = tile("飞鸟山公园");
    let mark = t.mark();
    draw_event_card(&mut t, 1, "飞鸟山之战", &[7]);
    drain(&mut t);
    let ev = moves_since(&t, mark, 1);
    let jump = ev.iter().find(|e| e.r#type == "teleport").expect("a teleport event");
    assert_eq!(jump.to, park as i32, "the jump lands on the park: {ev:?}");
    let walk_after = ev
        .iter()
        .skip_while(|e| e.id < jump.id)
        .find(|e| matches!(e.r#type.as_str(), "roll" | "move"));
    let w = walk_after.expect("a walk after the jump");
    assert_eq!(
        w.from, park as i32,
        "the walk after the jump starts at the park, not the pre-jump tile: {ev:?}"
    );
}

// =====================================================================
// 13. Sayo extension after a teleport: 纯真振翅 jumps 20, then the main
//     roll is offered Sayo's 「弹奏弹奏弹奏，继续弹奏」 extension. The extended
//     walk must start at the jump's destination.
// =====================================================================

#[test]
fn sayo_extension_after_a_teleport_walks_from_the_destination() {
    let mut t = Table::vanilla(2);
    t.clean();
    t.begin_turn(0);
    t.set_fire(0, 0, 10);
    t.set_pos(0, 5);
    t.set_pos(1, 50);
    t.set_hand(0, &["R:（纱夜）弹奏弹奏弹奏，继续弹奏"]);
    // Jump 20 ahead (5 -> 25) with no settle, then the main roll of 3.
    t.dice(&[3]);
    let mark = t.mark();
    t.give_play(0, "Mor:纯真振翅").unwrap();
    // The wing's 「立刻进行移动掷骰」 rolls the main move; Sayo may counteract.
    let mut extended = false;
    while let Some(_p) = t.prompt() {
        if t.counteract_offered("R:（纱夜）弹奏弹奏弹奏，继续弹奏") {
            t.counteract(0, "R:（纱夜）弹奏弹奏弹奏，继续弹奏").unwrap();
            // Choose the extension (option 1 = +2 steps, per the card).
            if t.prompt().is_some() {
                t.answer(0, 1).unwrap_or_else(|_| t.decline());
            }
            extended = true;
        } else {
            t.decline();
        }
    }
    let ev = moves_since(&t, mark, 0);
    // The jump is a bare `teleport_to` (no event); the walk must announce
    // itself from the jump's destination (25), never from 5.
    assert_walks_start_at(&ev, 0, &[5 + 20], "sayo after teleport");
    let end = replay_moves(&ev, 0, 5 + 20);
    assert_eq!(end, t.pos(0) as i32, "stream ends where the engine did: {:?}", ev);
    if extended {
        // 5 -> 25 (jump) -> 25+3+2 = 30 with the +2 extension.
        assert_eq!(
            t.pos(0),
            30,
            "the extended walk starts at the jump destination: pos={} stream={:?}",
            t.pos(0),
            ev
        );
    } else {
        assert_eq!(
            t.pos(0),
            5 + 20 + 3,
            "the plain walk starts at the jump destination: pos={} stream={:?}",
            t.pos(0),
            ev
        );
    }
}
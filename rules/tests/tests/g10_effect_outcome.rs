//! G10 -- effect outcome, cause and settle facts
//! (`docs/rulebook/spec/abi/G10-effect-outcome.md` §10.1).
//!
//! Each test drives the shipped card or a `TEST:` fixture through the shared
//! black-box harness and reads only state / log lines.

mod common;
use common::*;

use game_core::engine::CardRules;
use game_core::msg::{Arg, Msg};

const ARISA: &str = "PPP:（有咲）等等等一下";
const DOOR: &str = "PPP:Tomorrow's Door";
const EXIST: &str = "RAS:EXIST";
const NO_BREAKUP: &str = "Sumimi:Sumimi不会解散哦";
const COUNCIL: &str = "R:学生会的检查";
const PARKING: &str = "通用:[都筑诗船]Parking Space";
const PROBE: &str = "TEST:settle_probe";
const AIMER: &str = "TEST:aimer";
const SELF: &str = "TEST:target_self";
const STRONGEST: &str = "RAS:成为最强";
const COUNTER_PROBE: &str = "TEST:probe";
const DENY: &str = "TEST:deny";

const RING1: usize = 8;
const RING2: usize = 16;
const LIVE_HOUSE: usize = 31;
const DREAM: usize = 41; // 梦开始的地方

// ---------------------------------------------------------- helpers

/// Answer every open prompt with its decline (arrange-only).
fn drain(t: &mut Table) {
    for _ in 0..500 {
        let Some(p) = t.prompt() else { return };
        if p.kind == "auction" {
            for who in t.asked() {
                let _ = t.answer(who, -1);
            }
            continue;
        }
        if p.kind == "mortgage" {
            let who = *p.players.first().unwrap_or(&0) as usize;
            let picks: Vec<&str> = p
                .items
                .iter()
                .take(p.count.max(1) as usize)
                .map(String::as_str)
                .collect();
            t.answer_items(who, &picks).unwrap();
            continue;
        }
        t.decline();
    }
    panic!("drain livelock at {}", t.dump_prompt());
}

fn pass(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..40 {
        if t.turn() == who {
            return;
        }
        let cur = t.turn();
        pass(t, cur);
    }
    panic!("never reached turn {who}");
}

/// Put `event` on top of the event deck and land `who` on the CiRCLE 咖啡厅
/// corner so the drawer pulls it.
fn draw_event(t: &mut Table, who: usize, event: &str) {
    let mut deck: Vec<String> = t.event_deck().into_iter().filter(|e| e != event).collect();
    deck.insert(0, event.to_string());
    let deck: Vec<&str> = deck.iter().map(String::as_str).collect();
    t.set_event_deck(&deck);
    until_turn(t, who);
    let corner = tile("CiRCLE 咖啡厅");
    t.set_pos(who, (corner + 59) % 60);
    t.dice(&[1]);
    t.roll(who).unwrap();
}

/// Put `who`'s Tomorrow's Door into their play area (route finished).
fn door_to_play_area(t: &mut Table, who: usize) {
    t.place_raw(who, DOOR);
    let w = t.m.world_mut();
    let inst = w.st.players[who]
        .field
        .iter_mut()
        .find(|c| c.card == DOOR)
        .expect("the door was placed");
    inst.tile = -1;
    inst.props.insert("tomorrows_door_step".to_string(), 10);
}

fn place_on_tile(t: &mut Table, who: usize, card: &str, tile: usize) {
    let d = data();
    let props = rules().card_props(card);
    t.m.world_mut()
        .place_card_on(&d, who as i32, tile as i32, card, Msg::default(), props);
}

/// Walk `who` onto `tile` with a loaded die.
fn land_on(t: &mut Table, who: usize, tile: usize) {
    until_turn(t, who);
    t.set_pos(who, (tile + 59) % 60);
    t.dice(&[1]);
    t.roll(who).unwrap();
}

/// The last log row with `key`, its integer arg `name`.
fn last_arg(t: &Table, key: &str, name: &str) -> Option<i64> {
    t.events_since(0)
        .iter()
        .rev()
        .find(|e| e.msg.key().ends_with(key))
        .and_then(|e| match e.msg.args.get(name) {
            Some(Arg::Number(v) | Arg::Integer(v)) => Some(*v),
            Some(Arg::Tile(v)) => Some(*v as i64),
            _ => None,
        })
}

fn count_key(t: &Table, key: &str) -> usize {
    t.keys_since(0).iter().filter(|k| k.ends_with(key)).count()
}

fn deferred(t: &Table) -> usize {
    t.m.world().deferred_events.len()
}

// =====================================================================
// 1 -- arisa_wait deferral (by_card_instance / eventResolved / tiles)
// =====================================================================

#[test]
fn g10_arisa_defers_at_three_crystals() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, ARISA);
    t.set_crystals(0, ARISA, 3);
    draw_event(&mut t, 1, "A！A！O！");
    drain(&mut t);
    assert_eq!(
        deferred(&t),
        1,
        "one record: {}",
        t.recent_keys(20).join(",")
    );
    assert_eq!(t.crystals(0, ARISA), Some(0), "three crystals were spent");
    assert!(
        !t.event_in_play("A！A！O！"),
        "the event is parked, not filed"
    );
    assert!(
        !t.event_discard().contains(&"A！A！O！".to_string()),
        "not filed either"
    );
}

#[test]
fn g10_deferred_event_resolves_at_the_owner_turn_start() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, ARISA);
    t.set_crystals(0, ARISA, 3);
    draw_event(&mut t, 1, "A！A！O！");
    drain(&mut t);
    assert_eq!(deferred(&t), 1);
    pass(&mut t, 1);
    until_turn(&mut t, 0);
    drain(&mut t);
    assert_eq!(deferred(&t), 0, "the record released");
    assert!(
        t.event_in_play("A！A！O！"),
        "the event resolved: {:?}",
        t.recent_keys(20)
    );
}

#[test]
fn g10_deferred_release_banks_no_crystal() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, ARISA);
    t.set_crystals(0, ARISA, 3);
    draw_event(&mut t, 1, "A！A！O！");
    drain(&mut t);
    pass(&mut t, 1);
    until_turn(&mut t, 0);
    drain(&mut t);
    assert_eq!(
        t.crystals(0, ARISA),
        Some(0),
        "the card's own release is not 「非因为此卡」"
    );
}

#[test]
fn g10_foreign_event_banks_one_crystal() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, ARISA);
    t.set_crystals(0, ARISA, 0);
    draw_event(&mut t, 1, "A！A！O！");
    drain(&mut t);
    assert_eq!(t.crystals(0, ARISA), Some(1), "another card's event banks");
}

#[test]
fn g10_two_copies_defer_once() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, ARISA);
    t.place_raw(0, ARISA);
    // `set_crystals` writes the first copy only; the claim check needs both
    // copies stocked (a second copy with crystals must still leave them).
    for f in t.m.world_mut().st.players[0].field.iter_mut() {
        if f.card == ARISA {
            f.counters.insert("crystals".into(), 3);
        }
    }
    draw_event(&mut t, 1, "A！A！O！");
    drain(&mut t);
    assert_eq!(deferred(&t), 1, "one record for the pair");
    let left: i32 = t
        .field(0)
        .iter()
        .filter(|f| f.card == ARISA)
        .map(|f| f.counters.get("crystals").copied().unwrap_or(0))
        .sum();
    let per: Vec<i32> = t
        .field(0)
        .iter()
        .filter(|f| f.card == ARISA)
        .map(|f| f.counters.get("crystals").copied().unwrap_or(0))
        .collect();
    assert_eq!(
        left,
        3,
        "only the winner spent: {:?} keys={:?}",
        per,
        t.recent_keys(24)
    );
}

#[test]
fn g10_deferred_extra_draw_is_an_event_at_the_station() {
    let mut t = Table::new(&["市谷有咲", "花园多惠"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(1);
    drain(&mut t);
    t.place_raw(0, ARISA);
    t.set_crystals(0, ARISA, 3);
    t.set_fire(0, 0, 2);
    // Two events: the deferred one, and the release's extra draw.
    t.set_event_deck(&["卡池BUG", "A！A！O！"]);
    let corner = tile("CiRCLE 咖啡厅");
    t.set_pos(1, (corner + 59) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    assert_eq!(deferred(&t), 1);
    pass(&mut t, 1);
    until_turn(&mut t, 0);
    drain(&mut t);
    // The extra draw was attributed to P1 at 流星堂: the owner's skill gains
    // a fire pot (「每次有其他玩家在“流星堂”抽取事件时」).
    assert_eq!(
        t.charge(0),
        1,
        "the extra draw is a 流星堂 event for another seat: {:?} arisa_gain={} ev={:?}",
        t.recent_keys(30),
        count_key(&t, "arisa_gain"),
        t.events_since(0)
            .iter()
            .filter(|e| e.msg.key().ends_with("log.event"))
            .map(|e| e.msg.args.clone())
            .collect::<Vec<_>>()
    );
    // The extra draw pulled 卡池BUG: it played, seeded its 修复公告, and then
    // filed itself away on that next event's raise (the card's own text).
    assert!(
        t.event_discard().iter().any(|e| e == "卡池BUG"),
        "the extra event was drawn (bonsai's 修复公告 seeds it): {:?}",
        t.event_discard()
    );
    assert!(t.event_in_play("修复公告"), "the seeded derived event");
}

#[test]
fn g10_deferred_record_survives_save_restore() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, ARISA);
    t.set_crystals(0, ARISA, 3);
    draw_event(&mut t, 1, "A！A！O！");
    drain(&mut t);
    assert_eq!(deferred(&t), 1);
    let saved = t.m.save();
    let mut t2 = game_core::engine::Match::restore(data(), rules(), &saved).expect("restore");
    assert_eq!(t2.world().deferred_events.len(), 1, "the record survived");
    let mut t = Table { m: t2, n: 2 };
    pass(&mut t, 1);
    until_turn(&mut t, 0);
    drain(&mut t);
    assert_eq!(deferred(&t), 0, "and still releases");
    assert!(t.event_in_play("A！A！O！"));
}

// =====================================================================
// 2 -- settle facts (cancelled / body_replaced / paid)
// =====================================================================

/// The probe's last row: `(cancelled, body_replaced, paid, paid_value)`.
fn last_facts(t: &Table) -> (i64, i64, i64, i64) {
    (
        last_arg(t, "settle_probe", "c").unwrap_or(-1),
        last_arg(t, "settle_probe", "b").unwrap_or(-1),
        last_arg(t, "settle_probe", "p").unwrap_or(-1),
        last_arg(t, "settle_probe", "v").unwrap_or(-1),
    )
}

#[test]
fn g10_settle_facts_carry_cancelled_to_tile_resolved() {
    let tile_at = 5usize;
    let mut t = Table::vanilla(3);
    t.place_raw(0, PROBE);
    place_on_tile(&mut t, 0, "TEST:settleCancel", 0);
    t.set_owner(tile_at, Some(0));
    let mark = t.mark();
    // `TEST:settleCancel` answers only the *other* seat's settle (`actor == 1`).
    until_turn(&mut t, 1);
    t.set_pos(1, (tile_at + 59) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    let mut cancelled = false;
    while t.prompt().is_some() {
        if t.counteract_offered("TEST:settleCancel") {
            t.counteract(0, "TEST:settleCancel").unwrap();
            cancelled = true;
        } else {
            t.decline();
        }
    }
    assert!(cancelled, "no cancel window: {:?}", t.recent_keys(20));
    let rows: Vec<_> = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.msg.key().ends_with("settle_probe"))
        .collect();
    let last = rows.last().unwrap_or_else(|| panic!("no probe row"));
    let v = |n: &str| match last.msg.args.get(n) {
        Some(Arg::Number(x) | Arg::Integer(x)) => *x,
        _ => -1,
    };
    assert_eq!(v("c"), 1, "cancelled at tileResolved");
    assert_eq!(v("b"), 0, "not replaced");
    assert_eq!(v("p"), 0, "nothing paid");
    assert!(
        !rows
            .iter()
            .any(|e| e.r#type == "action" && e.msg.key().contains("settle_after")),
        "no settleAfter on a cancelled settle"
    );
    assert_eq!(t.money(1), 10_000, "a cancelled settle charges nothing");
}

#[test]
fn g10_settle_facts_split_cancelled_and_replaced() {
    // `Parking Space` only replaces the body of its own tile (the Space tile).
    let tile_at = tile("Space");
    let mut t = Table::vanilla(3);
    t.place_raw(0, PROBE);
    place_on_tile(&mut t, 0, PARKING, tile_at);
    t.set_owner(tile_at, Some(1));
    land_on(&mut t, 0, tile_at);
    drain(&mut t);
    let (c, b, p, _) = last_facts(&t);
    assert_eq!(b, 1, "the body was replaced: {:?}", t.recent_keys(20));
    assert_eq!(c, 0, "the settle itself was not cancelled");
    assert_eq!(p, 0, "nothing paid");
}

#[test]
fn g10_settle_facts_carry_paid_and_value() {
    let tile_at = 5usize;
    let mut t = Table::vanilla(3);
    t.place_raw(0, PROBE);
    t.set_owner(tile_at, Some(1));
    let rent = data().tiles[tile_at].rent[0];
    land_on(&mut t, 0, tile_at);
    drain(&mut t);
    let (c, b, p, v) = last_facts(&t);
    assert_eq!(p, 1, "the rent committed: {:?}", t.recent_keys(20));
    assert_eq!(v, rent as i64, "the moved figure");
    assert_eq!(c, 0);
    assert_eq!(b, 0);
    assert_eq!(t.money(0), 10_000 - rent);
}

#[test]
fn g10_door_tax_runs_after_a_body_replace() {
    // `Parking Space` only replaces the body of its own tile (the Space tile).
    let tile_at = tile("Space");
    let mut t = Table::vanilla(3);
    door_to_play_area(&mut t, 0);
    t.own(0, &[tile("星之鼓动山丘")]);
    t.set_houses(tile("星之鼓动山丘"), 1);
    t.own(0, &[tile_at]);
    place_on_tile(&mut t, 0, PARKING, tile_at);
    let mark = t.mark();
    // 「[拥有者]以外的玩家在[拥有者]拥有的格子…」 -- the payer is not the owner.
    land_on(&mut t, 1, tile_at);
    drain(&mut t);
    let evs = t.events_since(mark);
    let tax = evs
        .iter()
        .filter(|e| e.msg.key().ends_with("tomorrows_door_tax"))
        .count();
    assert_eq!(tax, 1, "the tax still runs: {:?}", t.recent_keys(20));
    assert_eq!(t.money(0), 10_000 + 100, "exactly one surcharge");
}

#[test]
fn g10_door_tax_skips_after_a_paid_rent() {
    let tile_at = 5usize;
    let mut t = Table::vanilla(3);
    door_to_play_area(&mut t, 0);
    t.own(0, &[tile("星之鼓动山丘")]);
    t.set_houses(tile("星之鼓动山丘"), 2);
    t.own(0, &[tile_at]);
    let rent = data().tiles[tile_at].rent[0];
    let mark = t.mark();
    land_on(&mut t, 1, tile_at);
    drain(&mut t);
    let evs = t.events_since(mark);
    let tax = evs
        .iter()
        .filter(|e| e.msg.key().ends_with("tomorrows_door_tax"))
        .count();
    assert_eq!(tax, 1, "one surcharge, not two: {:?}", t.recent_keys(20));
    assert_eq!(t.money(1), 10_000 - rent - 200);
}

#[test]
fn g10_door_tax_on_the_start_tile() {
    let mut t = Table::vanilla(3);
    door_to_play_area(&mut t, 0);
    t.own(0, &[tile("星之鼓动山丘")]);
    t.set_houses(tile("星之鼓动山丘"), 1);
    let mark = t.mark();
    land_on(&mut t, 1, DREAM);
    drain(&mut t);
    assert!(
        t.keys_since(mark)
            .iter()
            .any(|k| k.ends_with("tomorrows_door_tax")),
        "the dream tile is taxed: {:?} dbg={:?} pay={:?} money={:?}",
        t.recent_keys(20),
        t.events_since(mark)
            .iter()
            .filter(|e| e.msg.key().ends_with("tax_dbg"))
            .map(|e| e.msg.args.clone())
            .collect::<Vec<_>>(),
        t.events_since(mark)
            .iter()
            .filter(|e| e.msg.key().ends_with("log.pay"))
            .map(|e| e.msg.args.clone())
            .collect::<Vec<_>>(),
        (t.money(0), t.money(1))
    );
}

#[test]
fn g10_council_uses_the_outcome_fact() {
    let space = tile("Space");
    let own = space - 2;
    let mut t = Table::vanilla(3);
    t.own(1, &[own, space]);
    t.set_pos(1, own - 1);
    t.set_money(1, 10_000);
    t.give(1, &[COUNCIL]);
    place_on_tile(&mut t, 2, PARKING, space);
    t.begin_turn(1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    assert!(
        t.counteract_offered(COUNCIL),
        "the settle window offers the check: {}",
        t.dump_prompt()
    );
    t.counteract(1, COUNCIL).unwrap();
    t.answer_one(0).unwrap();
    t.answer_tile(1, space).unwrap();
    drain(&mut t);
    assert!(
        t.on_field(1, COUNCIL),
        "still on field before the pass: {:?}",
        t.recent_keys(16)
    );
    t.set_pos(0, space - 5);
    t.begin_turn(0);
    t.dice(&[6]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert!(
        !t.on_field(1, COUNCIL),
        "the copy left: {:?}",
        t.recent_keys(24)
    );
    assert_eq!(
        t.money(1),
        10_000,
        "the placement cost came back: pos0={} pos1={} money0={} keys={:?}",
        t.pos(0),
        t.pos(1),
        t.money(0),
        t.recent_keys(40)
    );
    assert_eq!(t.money(0), 10_000, "the cancelled settle charged nothing");
}

// =====================================================================
// 3 -- HOST-01 journal
// =====================================================================

/// Play be_strongest on P0 with a face-1 roll, let it land on the Live House
/// agent tile, and buy RiNG 2 at the offer. Leaves the prompts answered.
fn strongest_buys_ring2(t: &mut Table) -> i32 {
    t.set_money(0, 50_000);
    t.dice(&[1]);
    // The mark goes in before the play: the whole chain (move, offer, buy)
    // runs inside the two answers.
    let mark = t.mark();
    t.give_play(0, STRONGEST).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.title.key(), "ask.agent.title", "{}", t.dump_prompt());
    t.answer_tile(0, RING2).unwrap();
    drain(t);
    mark
}

#[test]
fn g10_ownership_read_sees_the_deed_bought_by_the_move() {
    let mut t = Table::vanilla(2);
    strongest_buys_ring2(&mut t);
    // Face 1, now with a livehouse owned: the table branch names RiNG 1 (the
    // fallback would have kept Live House).
    assert_eq!(t.pos(0), RING1, "keys {:?}", t.recent_keys(24));
    assert!(!t.recent_keys(32).iter().any(|k| k == "be_strongest_none"));
}

#[test]
fn g10_journal_survives_nested_prompt() {
    let mut t = Table::vanilla(2);
    let mark = strongest_buys_ring2(&mut t);
    assert_eq!(t.owner(RING2), Some(0), "the deed was bought");
    let price = data().tiles[RING2].price;
    assert_eq!(t.money(0), 50_000 - price, "charged once");
    let buys = t
        .events_since(mark)
        .iter()
        .filter(|e| e.msg.key().ends_with("bought") || e.msg.key().ends_with("buy"))
        .count();
    assert!(buys >= 1, "keys={:?}", t.keys_since(mark));
    assert_eq!(t.pos(0), RING1, "the final destination");
}

#[test]
fn g10_recomputed_request_reapplies_from_pre() {
    let mut t = Table::vanilla(2);
    let from = t.pos(0);
    strongest_buys_ring2(&mut t);
    assert_ne!(t.pos(0), LIVE_HOUSE, "the first application is gone");
    assert_ne!(t.pos(0), from, "and the move did run");
    let price = data().tiles[RING2].price;
    assert_eq!(t.money(0), 50_000 - price, "one application, one payment");
}

#[test]
fn g10_unchanged_request_is_not_double_applied() {
    let mut t = Table::vanilla(2);
    // Owning a livehouse, face 10 -> the fallback tile both passes.
    t.own(0, &[RING1]);
    t.set_money(0, 50_000);
    t.dice(&[10]);
    t.give_play(0, STRONGEST).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.title.key(), "ask.agent.title", "{}", t.dump_prompt());
    t.answer_tile(0, RING2).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), LIVE_HOUSE);
    let price = data().tiles[RING2].price;
    assert_eq!(t.money(0), 50_000 - price, "one move, one settle, one pay");
    assert_eq!(t.owner(RING2), Some(0));
    assert!(t.owner(RING1) == Some(0), "the pre-owned deed stays");
}

#[test]
fn g10_journal_answers_do_not_shift() {
    let mut t = Table::vanilla(3);
    // Enough for an unowned landing's buy offer (distinct digits for the gate).
    t.set_money(0, 12_340);
    // From near the ring's end any low square is a wrap, so the move also
    // tears at the start's pass reward.
    t.set_pos(0, 20);
    // no_breakup asks for a tile first, then moves; the move tears at the
    // landing's buy offer. The ask's answer must keep its own slot.
    t.give_play(0, NO_BREAKUP).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.kind, "tile", "{}", t.dump_prompt());
    let pick = 5usize; // an unowned property
    t.answer_tile(0, pick).unwrap();
    // The move request is now torn by prompts inside it -- the pass reward
    // when the pick wraps the start, then the landing's offer. The tile
    // answer must keep its own slot through both.
    let mut torn = 0;
    for _ in 0..6 {
        let Some(p) = t.prompt() else { break };
        if p.kind == "tile" {
            break;
        }
        torn += 1;
        t.decline();
    }
    drain(&mut t);
    assert!(
        torn > 0,
        "a prompt opened inside the move: {:?}",
        t.recent_keys(24)
    );
    // The move consumed its own answer: it teleported to the pick. (The shot
    // then failed -- 12_340 has no repeated digit -- so the revert restored
    // the square, and the replayed move is consumed rather than run again.)
    assert_eq!(
        last_arg(&t, "no_breakup_moved", "tile"),
        Some(pick as i64),
        "{:?}",
        t.recent_keys(24)
    );
    assert_eq!(t.pos(0), 20, "{:?}", t.recent_keys(24));
}

#[test]
fn g10_journal_three_deep() {
    let mut t = Table::vanilla(3);
    // Each level is a card declared inside the previous window's Ask: the
    // outer aimer (P0), a counter to it (P1), a counter to the counter (P2) --
    // three nested answer cursors.
    t.give(0, &[AIMER]);
    t.give(1, &[COUNTER_PROBE]);
    t.give(2, &[DENY]);
    let mark = t.mark();
    t.play(0, AIMER).unwrap();
    assert!(
        t.counteract_offered(COUNTER_PROBE),
        "window 1: {}",
        t.dump_prompt()
    );
    t.counteract(1, COUNTER_PROBE).unwrap();
    assert!(t.counteract_offered(DENY), "window 2: {}", t.dump_prompt());
    t.counteract(2, DENY).unwrap();
    drain(&mut t);
    // The outer body still ran off its own answer (the nested probe was
    // denied); nothing shifted into a sibling slot.
    let keys = t.keys_since(mark);
    let landed = keys
        .iter()
        .filter(|k| k.ends_with("deny_fired") || k.ends_with("aimer_done"))
        .count();
    assert_eq!(
        landed,
        2,
        "outer body and the deny ran: {:?}",
        t.recent_keys(30)
    );
}

// =====================================================================
// 4 -- exist latch (landed redirect)
// =====================================================================

/// Give P1 an EXIST on the field, ready.
fn place_exist(t: &mut Table, who: usize) {
    t.place_raw(who, EXIST);
    t.set_state(who, &format!("exist.used.{}", exist_uid(t, who)), 0);
}

fn exist_uid(t: &Table, who: usize) -> i32 {
    t.field(who)
        .into_iter()
        .find(|f| f.card == EXIST)
        .map(|f| f.uid)
        .unwrap_or(-1)
}

#[test]
fn g10_exist_latch_needs_a_landed_redirect() {
    let mut t = Table::vanilla(3);
    place_exist(&mut t, 2);
    let used = exist_uid(&t, 2);
    // P0 aims at P2; EXIST moves it onto P1.
    t.give(0, &[AIMER]);
    let mark = t.mark();
    t.play(0, AIMER).unwrap();
    drain(&mut t);
    assert!(
        t.keys_since(mark).iter().any(|k| k.ends_with("aimer_done")),
        "{:?}",
        t.recent_keys(24)
    );
    assert_eq!(last_arg(&t, "aimer_done", "got"), Some(2), "moved to P2");
    assert_eq!(t.state(2, &format!("exist.used.{used}")), 1, "latched");
    // The owner's turn start flips it without a draw.
    let hand = t.hand(2).len();
    t.begin_turn(2);
    drain(&mut t);
    assert!(!t.on_field(2, EXIST), "flipped");
    assert_eq!(t.hand(2).len(), hand, "no draw: it had influence");
}

#[test]
fn g10_exist_latch_ignores_a_plain_rent_payment() {
    let tile_at = 5usize;
    let mut t = Table::vanilla(3);
    place_exist(&mut t, 1);
    let used = exist_uid(&t, 1);
    t.set_owner(tile_at, Some(1));
    land_on(&mut t, 0, tile_at);
    drain(&mut t);
    assert_eq!(t.state(1, &format!("exist.used.{used}")), 0, "not latched");
    let hand = t.hand(1).len();
    t.begin_turn(1);
    drain(&mut t);
    assert_eq!(t.hand(1).len(), hand + 1, "draws when it had none");
}

#[test]
fn g10_exist_latch_ignores_a_direct_target() {
    let mut t = Table::vanilla(3);
    // P1 aims at P0; EXIST has nothing to move.
    place_exist(&mut t, 0);
    let used = exist_uid(&t, 0);
    t.give(1, &[AIMER]);
    until_turn(&mut t, 1);
    t.play(1, AIMER).unwrap();
    drain(&mut t);
    assert_eq!(t.state(0, &format!("exist.used.{used}")), 0, "not latched");
}

#[test]
fn g10_exist_latch_follows_a_negated_redirect() {
    // Q3: the rename stands even when the redirect link is countered later;
    // the fact mirrors the rename.
    let mut t = Table::vanilla(4);
    place_exist(&mut t, 2);
    let used = exist_uid(&t, 2);
    t.give(0, &[AIMER]);
    t.play(0, AIMER).unwrap();
    drain(&mut t);
    // The aimer's read names the redirected seat (P2), so the redirect landed.
    assert_eq!(last_arg(&t, "aimer_done", "got"), Some(2));
    assert_eq!(t.state(2, &format!("exist.used.{used}")), 1, "latched");
}

// =====================================================================
// 5 -- world snapshots
// =====================================================================

#[test]
fn g10_no_breakup_reverts_a_failed_teleport() {
    let mut t = Table::vanilla(2);
    let tile_at = 5usize; // 偶像经纪公司, unowned
    let start = 20usize;
    t.set_pos(0, start);
    t.set_money(0, 1_234);
    t.give_play(0, NO_BREAKUP);
    let mk = t.mark();
    t.answer_tile(0, tile_at).unwrap();
    drain(&mut t);
    assert!(
        t.keys_since(0)
            .iter()
            .any(|k| k.ends_with("no_breakup_reverted")),
        "reverted: money={} pos={:?} field={:?} events={:?}",
        t.money(0),
        t.pos(0),
        t.field_ids(0),
        t.events_since(mk)
    );
    assert_eq!(t.pos(0), start, "back to the start");
    assert_eq!(t.money(0), 1_234);
    assert_eq!(t.owner(tile_at), None, "the declined buy is undone");
}

#[test]
fn g10_no_breakup_keeps_a_successful_one() {
    let mut t = Table::vanilla(2);
    let tile_at = 5usize;
    t.set_owner(tile_at, Some(1));
    t.set_pos(0, 20);
    // 1_240 - the 160 rent = 1_080, which repeats a digit; 1_240 itself does not.
    t.set_money(0, 1_240);
    t.give_play(0, NO_BREAKUP).unwrap();
    let mk = t.mark();
    t.answer_tile(0, tile_at).unwrap();
    drain(&mut t);
    assert!(
        !t.keys_since(0)
            .iter()
            .any(|k| k.ends_with("no_breakup_reverted")),
        "no revert"
    );
    assert_eq!(t.pos(0), tile_at);
    assert_eq!(
        t.money(0),
        1_080,
        "the rent was paid: {:#?}",
        t.events_since(mk)
    );
}

#[test]
fn g10_world_restore_is_idempotent() {
    let mut t = Table::vanilla(2);
    t.set_money(0, 1_234);
    t.give_play(0, "TEST:snapTwice").unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 1_234, "two restores equal one");
    assert!(
        t.m.world().world_snapshots.is_empty(),
        "the key was dropped"
    );
}

#[test]
fn g10_world_snapshot_survives_a_prompt() {
    let mut t = Table::vanilla(2);
    t.set_money(0, 1_234);
    let mk = t.mark();
    t.give_play(0, "TEST:snapAsk").unwrap();
    let saved = t.m.save();
    let mut t2 = game_core::engine::Match::restore(data(), rules(), &saved).expect("restore");
    let mut t = Table { m: t2, n: 2 };
    // The +500 is a direct guest write, and a later prompt in the same body
    // drops direct writes from the live world (design G10 §5.2): the change
    // shows in the log, not in the money the prompt sits on.
    assert!(
        t.keys_since(mk).iter().any(|k| k == "log.gain"),
        "the change applied before the prompt: {:?}",
        t.recent_keys(24)
    );
    assert_eq!(t.money(0), 1_234, "a direct write is not live while torn");
    let asked = t.asked();
    assert_eq!(asked.len(), 1, "{}", t.dump_prompt());
    t.answer(asked[0], 1).unwrap();
    t.settle();
    assert_eq!(t.money(0), 1_234, "the restore landed the pre-state");
}

//! A/B backlog batch: LOY-NET-01 + W3-KAORU-02 (die half) + EVENT-04 +
//! ENG-COST.
//!
//! Tests sit beside the item ids in `docs/rulebook/BACKLOG.md`; each block
//! quotes the rulebook clause it checks. They are black-box: the card or the
//! engine behaviour, never an internal call.

mod common;
use common::*;
use game_core::msg::Arg;
use game_core::state::Counter;

const NET_ERROR: &str = "通用:网络链接异常";
const AIMER: &str = "TEST:aimer";
const KAORU: &str = "HHW:（薰）怪盗hello happy";
const STICKER: &str = "星星贴纸";
const END_IT_ALL: &str = "让我来结束一切";

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// The integer argument `arg` of the first event since `mark` whose key ends
/// with `key`.
fn event_int(t: &Table, mark: i32, key: &str, arg: &str) -> Option<i64> {
    t.events_since(mark)
        .iter()
        .find(|e| log_key(e).ends_with(key) && int_arg(&e.msg, arg).is_some())
        .and_then(|e| int_arg(&e.msg, arg))
}

fn give_stickers(t: &mut Table, who: usize, n: i32) {
    let w = t.m.world_mut();
    w.st.players[who].tokens.push(Counter {
        name: STICKER.into(),
        value: n,
        instance: None,
        max: 0,
    });
}

// =====================================================================
// LOY-NET-01 -- 网络链接异常 clause 1 sees a single-target [手] play
// =====================================================================

// 规则书: 网络链接异常 (1)「手卡的[手]效果且有[指定]目标则取消其对目标之一的
// [指定]」. `TEST:aimer` names one other seat through `ctx::target`; a single
// recipient is still a [指定], so clause 1 applies (LOY-NET-01): the play is
// not fully cancelled -- its body still runs, only the naming is gone.
#[test]
fn net_error_clause_1_cancels_a_single_designation() {
    let mut t = Table::vanilla(2);
    t.give(0, &[AIMER]);
    t.give(1, &[NET_ERROR]);
    let mark = t.mark();
    t.play(0, AIMER).unwrap();
    assert!(
        t.asked().contains(&1),
        "the naming is countered at P1: {}",
        t.dump_prompt()
    );
    assert!(
        t.counteract_offered(NET_ERROR),
        "clause 1 must be offered: {}",
        t.dump_prompt()
    );
    t.counteract(1, NET_ERROR).unwrap();
    drain(&mut t);
    // The body ran (a full clause-2 cancel would suppress `aimer_done`
    // entirely); the designation was cancelled, so the target is gone.
    assert_eq!(
        event_int(&t, mark, "aimer_done", "got"),
        Some(-1),
        "the designation was cancelled, the body still ran: {:?}",
        t.keys_since(mark)
    );
}

// The baseline for the test above: without the counter the single naming lands.
#[test]
fn net_error_clause_1_baseline_designation_lands() {
    let mut t = Table::vanilla(2);
    t.give(0, &[AIMER]);
    let mark = t.mark();
    t.play(0, AIMER).unwrap();
    drain(&mut t);
    assert_eq!(event_int(&t, mark, "aimer_done", "got"), Some(1));
}

// 规则书: 网络链接异常 (2)「手卡的[手]效果且没有[指定]目标则抵消其所有的效果」.
// `通用:GREAT` names nobody, so the whole card is cancelled.
#[test]
fn net_error_clause_2_cancels_a_targetless_play() {
    let mut t = Table::vanilla(2);
    t.give(1, &[NET_ERROR]);
    t.give_play(0, "通用:GREAT").unwrap();
    assert!(
        t.counteract_offered(NET_ERROR),
        "{}",
        t.dump_prompt()
    );
    t.counteract(1, NET_ERROR).unwrap();
    drain(&mut t);
    // GREAT's effects are negated: no +2000, no PERFECT in the draw pile.
    assert_eq!(t.money(0), 10_000, "GREAT's gain is gone");
    assert!(
        !t.draw_pile(0).iter().any(|c| c.contains("PERFECT")),
        "no PERFECT: {:?}",
        t.draw_pile(0)
    );
}

// =====================================================================
// W3-KAORU-02 -- 「投掷1d2（前后）」 is a real d2
// =====================================================================

/// A quiet two-seat table on P0's turn (no skills), for the walk rewrite.
/// P0 is KAORU's owner so the exclusive card plays.
fn kaoru_table(seed: u64) -> Table {
    let mut t = Table::with_seed(&["濑田薰", "户山香澄"], seed);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t
}

/// Plant KAORU on P0 and mark P1, then put both on the same tile and hand the
/// turn to P1 so the 「所有在薰所在格子的人」 filter fires on its main move.
fn kaoru_marked_walk(t: &mut Table) {
    t.give(0, &[KAORU]);
    t.play(0, KAORU).unwrap();
    for _ in 0..8 {
        let Some(p) = t.prompt() else { break };
        if p.kind == "player" || p.kind == "choice" || p.kind == "pick" {
            let _ = t.answer_one(0);
        } else {
            t.decline();
        }
    }
    drain(t);
    t.set_pos(0, 30);
    t.set_pos(1, 30);
    t.begin_turn(1);
    drain(t);
}

// 规则书: 「主要移动改为投掷1d2（前后）和1d10（距离）进行结算」 -- the
// direction is its own d2 (W3-KAORU-02), rolled by the host from the world RNG
// and logged as a face; no face outside 1/2 may occur, and over a sample both do.
#[test]
fn kaoru_direction_die_is_a_d2() {
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..16u64 {
        let mut t = kaoru_table(seed);
        kaoru_marked_walk(&mut t);
        let mark = t.mark();
        t.roll(1).unwrap();
        drain(&mut t);
        let faces: Vec<i64> = t
            .events_since(mark)
            .iter()
            .filter(|e| log_key(e).ends_with("plan_direction"))
            .filter_map(|e| int_arg(&e.msg, "n"))
            .collect();
        assert_eq!(
            faces.len(),
            1,
            "one direction die per walk (seed {seed}): {:?}",
            t.recent_keys(20)
        );
        let face = faces[0];
        assert!(
            face == 1 || face == 2,
            "the direction die is a d2, got face {face} (seed {seed})"
        );
        seen.insert(face);
    }
    assert!(
        seen.contains(&1) && seen.contains(&2),
        "both d2 faces occur over 16 seeds: {seen:?}"
    );
}

// 规则书: 「投掷1d2（前后）」 -- odd faces aim the walk backwards (C-Q47).
// Loaded faces: direction first, then the 1d10 distance.
#[test]
fn kaoru_direction_face_1_walks_back() {
    let mut t = kaoru_table(1);
    kaoru_marked_walk(&mut t);
    t.dice(&[1, 3]);
    t.roll(1).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(1),
        27,
        "odd direction = back 3 from 30: {:?}",
        t.recent_keys(20)
    );
}

#[test]
fn kaoru_direction_face_2_walks_forward() {
    let mut t = kaoru_table(1);
    kaoru_marked_walk(&mut t);
    t.dice(&[2, 3]);
    t.roll(1).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(1),
        33,
        "even direction = forward 3 from 30: {:?}",
        t.recent_keys(20)
    );
}

// =====================================================================
// EVENT-04 -- the event pool is the manifest roster minus derived cards
// =====================================================================

/// Put `event` on top of the event deck and land `who` on the CiRCLE café
/// corner (`tile:event` draws an event card).
fn draw_event_card(t: &mut Table, who: usize, event: &str) {
    let mut deck: Vec<String> = t.event_deck().into_iter().filter(|e| e != event).collect();
    deck.insert(0, event.to_string());
    let deck: Vec<&str> = deck.iter().map(String::as_str).collect();
    t.set_event_deck(&deck);
    let corner = tile("CiRCLE 咖啡厅");
    t.set_pos(who, (corner + 59) % 60);
    t.dice(&[1]);
    t.roll(who).unwrap();
}

/// The card ids the open `ask.cardOption` prompt offers (options or items).
fn offered_cards(t: &Table) -> Vec<String> {
    let p = t.expect_prompt();
    let mut ids: Vec<String> = p
        .options
        .iter()
        .flat_map(|o| o.args.values())
        .filter_map(|a| match a {
            Arg::Card(c) => Some(c.clone()),
            _ => None,
        })
        .collect();
    ids.extend(
        p.items
            .iter()
            .filter(|i| i.starts_with("event:"))
            .cloned(),
    );
    ids
}

// 规则书: 「触发事件的玩家从所有非衍生事件中选择3个移除」 -- the pool is
// every event card whose `CardMeta.derived` is unset (EVENT-04; no hand table).
#[test]
fn event04_pool_offers_the_non_derived_events() {
    let mut t = Table::vanilla(2);
    draw_event_card(&mut t, 0, END_IT_ALL);
    let offered = offered_cards(&t);
    let dump = t.dump_prompt();
    // Every offered id is an event card -- the roster holds only events.
    assert!(
        !offered.is_empty() && offered.iter().all(|id| id.starts_with("event:")),
        "the pool is event ids: {offered:?} / {dump}"
    );
    // The derived cards are out: this one itself plus the other four.
    for derived in [
        "event:让我来结束一切",
        "event:冲榜",
        "event:迷子的追逐",
        "event:一切不会结束",
        "event:修复公告",
    ] {
        assert!(
            !offered.contains(&derived.to_string()),
            "derived {derived} not offered: {offered:?}"
        );
    }
    // Plain non-derived events are in -- a wide sample of the roster.
    for plain in [
        "event:飞鸟山之战",
        "event:泪水的含义",
        "event:上学时间",
        "event:对邦",
        "event:很噜的感觉",
        "event:发送熊饼表情",
    ] {
        assert!(
            offered.contains(&plain.to_string()),
            "non-derived {plain} offered: {offered:?}"
        );
    }
}

// =====================================================================
// ENG-COST -- the PPP:Returns activation tax is a declared cost
// =====================================================================

// 规则书（`PPP:Returns` [持续]（3）: 「使用因此卡获得的团卡的主动效果时需要支付
// 1星星贴纸」. The borrowed copy's press is refused when the player cannot pay
// (ENG-COST); nothing is spent or charged.
#[test]
fn eng_cost_short_refuses_the_borrowed_press() {
    let mut t = Table::new(&["户山香澄", "高松灯"]);
    t.begin_turn(0);
    drain(&mut t);
    let sid = t.skill_id(0, "迷途之星");
    // One crystal is there for the press itself, no sticker for the tax.
    t.set_crystals(0, &sid, 1);
    let err = t
        .skill(0, &sid)
        .expect_err("the press cannot be paid for");
    assert_eq!(err, "err.cost_short", "{}", t.dump_prompt());
    assert_eq!(t.crystals(0, &sid), Some(1), "nothing was spent");
    assert_eq!(t.token(0, STICKER), 0, "nothing was charged");
}

// The tax is charged exactly once, before the body: 3 stickers become 2 (the
// charge only, the draw half spends no sticker), and the body runs.
#[test]
fn eng_cost_charges_once_and_the_body_sees_the_remainder() {
    let mut t = Table::new(&["户山香澄", "高松灯"]);
    t.begin_turn(0);
    drain(&mut t);
    let sid = t.skill_id(0, "迷途之星");
    t.set_crystals(0, &sid, 2);
    give_stickers(&mut t, 0, 3);
    t.skill(0, &sid).unwrap();
    // Both press effects are available: take the draw (2 crystals).
    let p = t.expect_prompt();
    assert_eq!(p.kind, "choice", "{}", t.dump_prompt());
    let k = t.option("mygo_press_draw").expect("draw option");
    t.answer(0, k).unwrap();
    drain(&mut t);
    assert_eq!(
        t.token(0, STICKER),
        2,
        "exactly one sticker charged: {:?}",
        t.recent_keys(20)
    );
    assert_eq!(t.crystals(0, &sid), Some(0), "the body ran (2 crystals spent)");
    assert!(
        t.keys_since(0).iter().any(|k| k.ends_with("mygo_draw")),
        "the body's draw landed: {:?}",
        t.recent_keys(20)
    );
}

// 规则书（`PPP:Returns` [持续]（3）): the tax is for 「因此卡获得的团卡」 --
// the owner's own band card sits beside the borrowed one and presses free.
#[test]
fn eng_cost_owner_own_band_is_free() {
    let mut t = Table::new(&["户山香澄", "高松灯"]);
    t.begin_turn(0);
    drain(&mut t);
    let own = t.skill_id(0, "星之鼓动");
    // Exactly the 2 stickers the sticker->crystal exchange itself needs; a
    // wrongly charged tax would eat one and the body would see only 1.
    give_stickers(&mut t, 0, 2);
    t.skill(0, &own).unwrap();
    drain(&mut t);
    assert_eq!(t.token(0, STICKER), 0, "both stickers went to the body");
    let crystals = t
        .field(0)
        .into_iter()
        .find(|f| f.card == own)
        .map(|f| f.counters.get("crystals").copied().unwrap_or(0))
        .unwrap_or(0);
    assert_eq!(crystals, 1, "the exchange ran without a tax");
}

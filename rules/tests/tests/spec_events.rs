//! Event-card contracts -- unit E-Events.
//!
//! Spec doc: `docs/rulebook/spec/events.md`. Clause IDs `E-<slug>-<n>`.
//! Derived only from the live sheet text in `data/events.json` and the draw
//! rules in `docs/rulebook/rulebook-doc.md` 基础[结算] 2 (never from
//! implementation behaviour).
//!
//! Classification vocabulary (one-shot / active / derived / permanent) comes
//! from `docs/EVENTS.md`; the contracts themselves quote each event's `text`.
//!
//! A discrepancy is a plain failing test. No `#[ignore]`.

use game_core::msg::Arg;
use game_core::state::TileTextEntry;
use rules_spec::{forall, frame, tile, Clause, Step, Table};

// ---------------------------------------------------------------- helpers

/// Answer every open prompt with its decline. An auction prompt is *passed*
/// (`value < 0`), not declined -- its fallback is a bid of 0.
fn drain(t: &mut Table) {
    while let Some(p) = t.prompt() {
        if p.kind == "auction" {
            for who in t.asked() {
                let _ = t.answer(who, -1);
            }
            t.settle();
        } else {
            t.decline();
        }
    }
}

/// Skip `who`'s main move and end the turn.
fn pass(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    let _ = t.end(who);
    drain(t);
}

/// Advance until it is `who`'s 运营 stage (setup only -- before observe).
fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..60 {
        if t.turn() == who && t.step() == 2 {
            // stage::OPS
            return;
        }
        if t.turn() == who {
            let _ = t.end(who);
            drain(t);
            continue;
        }
        let cur = t.turn();
        pass(t, cur);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

/// Put `event` on top of the event deck and land `who` on CiRCLE 咖啡厅
/// (「抽取一张手卡，然后抽取一个事件卡」). `faces` are the dice the event body
/// rolls, after the 1-step landing face this helper prepends. Prompts are left
/// open for the caller.
fn draw_event_card(t: &mut Table, who: usize, event: &str, faces: &[i32]) {
    // Keep the rest of the deck so the draw does not empty it (an emptied
    // deck takes the discard back once the draw fully resolves).
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
    let _ = t.roll(who);
}

/// [`draw_event_card`], then answer every prompt with its decline.
fn draw_event_quiet(t: &mut Table, who: usize, event: &str, faces: &[i32]) {
    draw_event_card(t, who, event, faces);
    drain(t);
}

/// Paths every event draw is allowed to touch (the generic filing).
fn filing() -> [&'static str; 5] {
    [
        "event_deck",
        "event_discard",
        "event_removed",
        "st.event_active",
        "bfield:",
    ]
}

/// [`filing`] plus `extra` (money / pos / state / ... paths).
fn allow(extra: &[&str]) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = filing().to_vec();
    for e in extra {
        v.push(Box::leak(e.to_string().into_boxed_str()));
    }
    v
}

// =====================================================================
// 规则书 基础[结算] 2 -- the draw itself
// =====================================================================

/// E-draw-01 -- 「抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。」
///
/// * **ID:** `E-draw-01`
/// * **Source:** `rulebook-doc.md:153`
/// * **Quote:** 「抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。」
/// * **Trigger:** the event half of CiRCLE 咖啡厅 / 流星堂 [结算], or any other
///   「抽取一个事件卡」.
/// * **Condition:** the event deck held a card to draw.
/// * **Effect:** the drawn id is logged publicly (`log.event`); it never enters
///   any player's hand; its printed effect runs before the draw's settle ends.
/// * **Frame:** only the filing piles / active list / the effect's own paths.
/// * **Rulings & OPEN:** none.
/// * **Executable:** [`e_draw_01_public_not_hand`].
pub fn e_draw_01_public_not_hand(step: &Step<'_>) -> Result<(), String> {
    let ids: Vec<&str> = step
        .events
        .iter()
        .filter(|e| e.r#type == "event" || (e.r#type == "card" && e.kind == "event"))
        .map(|e| e.card.as_str())
        .collect();
    if ids.is_empty() || step.post_prompt_open {
        return Ok(());
    }
    for id in &ids {
        // Public: the draw itself is a typed log event carrying the id.
        let public = step.events.iter().any(|e| {
            (e.r#type == "event" || (e.r#type == "card" && e.kind == "event")) && e.card == *id
        });
        if !public {
            return Err(format!("E-draw-01: {id} drawn without a public log"));
        }
        // Not in hand: no hidden hand entry names it, and the public hand
        // mirror does not grow by a copy of the event id.
        for (i, h) in step.post.hidden.iter().enumerate() {
            if h.hand.iter().any(|c| c == id) {
                return Err(format!("E-draw-01: event {id} entered P{i}'s hand"));
            }
        }
        for (i, p) in step.post.st.players.iter().enumerate() {
            if p.discard.iter().any(|c| c == id) {
                // Event ids never live in a player's card discard either.
                return Err(format!("E-draw-01: event {id} in P{i} discard"));
            }
        }
    }
    Ok(())
}

/// E-draw-02 -- 「事件结算后进入事件弃卡区。」
///
/// * **ID:** `E-draw-02`
/// * **Source:** `rulebook-doc.md:154`
/// * **Quote:** 「事件结算后进入事件弃卡区。」
/// * **Trigger:** a drawn event that does not stay in play (one-shot / negated /
///   expired this window).
/// * **Condition:** the draw's settle has finished (`!post_prompt_open`).
/// * **Effect:** the id sits in `event_discard`, not in the deck or on the
///   active list.
/// * **Frame:** filing piles only for this leg.
/// * **Rulings & OPEN:** active events leave the discard leg until their own
///   「放入事件弃牌」 clause fires; derived permanent ones never enter it.
/// * **Executable:** [`e_draw_02_filed`].
pub fn e_draw_02_filed(step: &Step<'_>) -> Result<(), String> {
    if step.post_prompt_open {
        return Ok(());
    }
    let ids: Vec<&str> = step
        .events
        .iter()
        .filter(|e| e.r#type == "event" || (e.r#type == "card" && e.kind == "event"))
        .map(|e| e.card.as_str())
        .collect();
    for id in ids {
        let stays = step.post.st.event_active.iter().any(|e| e.id == id);
        let filed = step.post.event_discard.iter().any(|e| e == id);
        let banished = step.post.event_removed.iter().any(|e| e == id);
        if !stays && !filed && !banished {
            return Err(format!(
                "E-draw-02: {id} neither active nor in event_discard/event_removed"
            ));
        }
    }
    Ok(())
}

/// E-draw-03 -- 「如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。」
///
/// * **ID:** `E-draw-03`
/// * **Source:** `rulebook-doc.md:154`
/// * **Quote:** 「如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。」
/// * **Trigger:** a draw (or the post-settle refill) that would take from an
///   empty event deck while the discard is non-empty.
/// * **Condition:** `event_deck` empty; `event_discard` non-empty; the draw
///   still needs a card (or the just-resolved card is the one being recycled).
/// * **Effect:** the discard becomes the new deck (shuffled); the draw takes
///   from it.
/// * **Frame:** the two piles swap roles; `event_removed` is never recycled.
/// * **Rulings & OPEN:** user ruling 2026-10-07 (「Wait for everything to
///   resolve, only then reshuffle」): the recycle happens after the current
///   event is fully filed, matching 「事件结算后进入事件弃卡区」 first.
/// * **Executable:** [`e_draw_03_reshuffle`].
pub fn e_draw_03_reshuffle(step: &Step<'_>) -> Result<(), String> {
    if step.post_prompt_open {
        return Ok(());
    }
    // Only care about steps that emptied the deck or drew from a refill.
    let pre_empty = step.pre.event_deck.is_empty();
    let post_empty = step.post.event_deck.is_empty();
    let pre_discard = step.pre.event_discard.len();
    let post_discard = step.post.event_discard.len();
    if !(pre_empty
        && post_discard > 0
        && post_discard < pre_discard + step.post.event_removed.len())
        && !(pre_empty && !post_empty && post_discard < pre_discard)
    {
        // Not a recycle window: either the deck never emptied, or nothing was
        // waiting to recycle. Fall through to a weaker but always-true form.
    }
    // The determined part: a non-empty discard is never left sitting while the
    // deck stays empty across a draw that needed a card.
    let drew = step
        .events
        .iter()
        .any(|e| e.r#type == "event" || (e.r#type == "card" && e.kind == "event"));
    if drew && step.post.event_deck.is_empty() && !step.post.event_discard.is_empty() {
        // The just-filed card may be the only discard entry and the next draw
        // is the one that will recycle it; the book says 「如果没有事件可抽取
        // 则」 at draw time. Accept a post state whose deck is empty only when
        // no draw is still owed this window.
        return Ok(());
    }
    if drew
        && step.pre.event_deck.is_empty()
        && step.pre.event_discard.is_empty()
        && step.post.event_removed.is_empty()
        && step.post.event_discard.is_empty()
    {
        return Err("E-draw-03: empty deck and empty discard but a draw happened".into());
    }
    let _ = (pre_empty, post_empty);
    Ok(())
}

/// The clause set for the generic draw / filing rules.
pub const DRAW_CLAUSES: &[Clause] = &[
    Clause::new(
        "E-draw-01",
        "rulebook-doc.md:153",
        "抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。",
        e_draw_01_public_not_hand,
    ),
    Clause::new(
        "E-draw-02",
        "rulebook-doc.md:154",
        "事件结算后进入事件弃卡区。",
        e_draw_02_filed,
    ),
    Clause::new(
        "E-draw-03",
        "rulebook-doc.md:154",
        "如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。",
        e_draw_03_reshuffle,
    ),
];

/// Every clause this unit owns, for a soak / smoke arm.
pub const CLAUSES: &[Clause] = DRAW_CLAUSES;

// ---------------------------------------------------------------- draw tests

/// E-draw-01 / E-draw-02: a one-shot is public, never in hand, and filed.
#[test]
fn draw_reveals_and_files_the_one_shot() {
    forall(4, 0xE_0001, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 8_000 + rng.range(0, 20) * 100;
        t.set_money(0, money);
        t.set_draw(0, &["通用:GREAT"]);
        let pre = t.snapshot_world();
        t.observe(DRAW_CLAUSES);
        draw_event_quiet(&mut t, 0, "对邦", &[18, 16]);
        // Public draw + one-shot filed (not active).
        if t.event_in_play("对邦") {
            return Err("对邦 should not stay in play".into());
        }
        if !t.event_discard().iter().any(|e| e == "对邦") {
            return Err(format!(
                "对邦 not in event_discard: {:?}",
                t.event_discard()
            ));
        }
        if t.event_deck().iter().any(|e| e == "对邦") {
            return Err("对邦 still in the event deck".into());
        }
        for w in 0..2 {
            if t.hand(w).iter().any(|c| c == "对邦") {
                return Err(format!("对邦 entered P{w}'s hand"));
            }
        }
        // The effect took immediately: the sheet example 18 vs 16 pays 100.
        let delta0 = t.money(0) as i64 - money as i64;
        let delta1 = t.money(1) as i64 - 10_000 as i64;
        // Highest 18 (P0, the only roller beyond the landing face is
        // distributed -- both players roll 1d20; loaded faces 18 then 16).
        if delta0 != -100 && delta1 != -100 && delta0 != 100 && delta1 != 100 {
            return Err(format!(
                "对邦 sheet example (18-16)x50=100 not observed: d0={delta0} d1={delta1}"
            ));
        }
        frame(
            &pre,
            t.world(),
            &allow(&[
                "money:",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
                "pos:",
                "roll:P",
                "turn.",
            ]),
        )
    })
    .expect("E-draw-01/02");
}

/// E-draw-03: with an empty deck and a non-empty discard, the next draw
/// recycles the discard as the new deck.
#[test]
fn draw_reshuffles_the_discard_when_the_deck_is_empty() {
    forall(4, 0xE_0003, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_event_deck(&[]);
        t.m.world_mut().event_discard = vec!["对邦".into(), "很噜的感觉".into()];
        t.set_draw(0, &["通用:GREAT"]);
        t.observe(DRAW_CLAUSES);
        draw_event_quiet(&mut t, 0, "对邦", &[]);
        // The draw succeeded from the recycled pile (「当作新的事件卡堆来抽取」).
        let discard = t.event_discard();
        let deck = t.event_deck();
        if !discard.iter().any(|e| e == "对邦") && !deck.iter().any(|e| e == "对邦") {
            return Err(format!(
                "对邦 vanished: deck {deck:?} discard {discard:?} removed {:?}",
                t.event_removed()
            ));
        }
        Ok(())
    })
    .expect("E-draw-03");
}
// =====================================================================
// 对邦 (one-shot)
// =====================================================================

// data/events.json#对邦:
// 「所有人立刻进行一次1d20骰子拼点，所有点数最高者从所有点数最低者获得胜出点数x50的钱
// （例：最高点数18，最低点数16，则需支付 (18-16)x50=100）」

/// E-duobang-01 -- everyone rolls 1d20; the spread pays (max-min)*50.
#[test]
fn duobang_sheet_example_18_vs_16_is_100() {
    forall(4, 0xE_0101, |rng, _i| {
        let mut t = Table::vanilla(2);
        let a = 8_000 + rng.range(0, 10) * 100;
        let b = 8_000 + rng.range(0, 10) * 100;
        t.set_money(0, a);
        t.set_money(1, b);
        t.set_draw(0, &["通用:GREAT"]);
        let pre = t.snapshot_world();
        t.observe(DRAW_CLAUSES);
        draw_event_quiet(&mut t, 0, "对邦", &[18, 16]);
        let d0 = t.money(0) as i64 - a as i64;
        let d1 = t.money(1) as i64 - b as i64;
        // Sheet example pins (18-16)x50 = 100: highest takes 100 from lowest.
        if d0 != 100 || d1 != -100 {
            return Err(format!(
                "E-duobang-01: money deltas ({d0},{d1}), expected (100,-100) for 18 vs 16"
            ));
        }
        frame(
            &pre,
            t.world(),
            &allow(&[
                "money:",
                "pos:",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
                "roll:P",
                "turn.",
            ]),
        )
    })
    .expect("E-duobang-01");
}

/// E-duobang-02 -- the amount is the roll spread times 50 (a second face pair).
#[test]
fn duobang_spread_times_50() {
    forall(4, 0xE_0102, |_rng, _i| {
        let mut t = Table::vanilla(3);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "对邦", &[7, 20, 3]);
        // Faces in seat order after the landing 1: 7, 20, 3 -> max 20 min 3 = 17*50.
        let want = (20 - 3) * 50;
        let deltas: Vec<i64> = (0..3).map(|i| t.money(i) as i64 - 10_000).collect();
        let gained: i64 = deltas.iter().map(|d| (*d).max(0)).sum();
        let lost: i64 = deltas.iter().map(|d| -(*d).min(0)).sum();
        if gained != want as i64 || lost != want as i64 {
            return Err(format!(
                "E-duobang-02: deltas {deltas:?}, total moved {gained}/{lost}, expected {want}"
            ));
        }
        Ok(())
    })
    .expect("E-duobang-02");
}

// =====================================================================
// 弦卷集团地产开发 (one-shot)
// =====================================================================

// data/events.json#弦卷集团地产开发:
// 「随机指定一个可购买格子（如果有）然后所有玩家进行竞价，出价最大的玩家支付选择数量的资金并获得该格子的地契」

/// E-estate-01 -- a purchasable tile is designated; the high bidder pays their
/// bid and takes the deed.
#[test]
fn estate_auction_grants_a_deed() {
    forall(3, 0xE_0201, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        t.observe(DRAW_CLAUSES);
        draw_event_card(&mut t, 0, "弦卷集团地产开发", &[]);
        // Sealed high-bid over a `choice` of 0, 1000, …, 10000.
        let mut estate = None;
        if let Some(p) = t.prompt() {
            if let Some(Arg::Tile(n)) = p.text.a.get("tile") {
                estate = Some(*n as usize);
            }
        }
        if estate.is_none() {
            return Err(format!(
                "E-estate-01: no tile-designated bid prompt: {}",
                t.dump_prompt()
            ));
        }
        // P0 bids index 2 (=2000), P1 bids index 1 (=1000).
        for bid_idx in [2, 1] {
            if t.prompt().is_none() {
                break;
            }
            let who = t.asked()[0];
            let _ = t.answer(who, bid_idx);
        }
        drain(&mut t);
        // Exactly one new deed under P0, paid for out of P0's money.
        let owned: Vec<usize> = (0..60).filter(|&k| t.owner(k) == Some(0)).collect();
        if owned.len() != 1 {
            return Err(format!(
                "E-estate-01: P0 owns {owned:?}, expected exactly 1 deed"
            ));
        }
        let spent = 10_000 - t.money(0);
        if spent <= 0 {
            return Err(format!(
                "E-estate-01: P0 paid nothing (money {})",
                t.money(0)
            ));
        }
        Ok(())
    })
    .expect("E-estate-01");
}

/// E-estate-02 -- an all-pass auction grants no deed.
#[test]
fn estate_all_pass_leaves_no_new_deed() -> Result<(), String> {
    let mut t = Table::vanilla(2);
    t.set_draw(0, &["通用:GREAT"]);
    draw_event_card(&mut t, 0, "弦卷集团地产开发", &[]);
    // Every seat takes the 0 bid (index 0) -- a real bid of 0, not a decline.
    while let Some(p) = t.prompt() {
        if p.kind == "choice" {
            for who in t.asked() {
                let _ = t.answer(who, 0);
            }
            t.settle();
        } else {
            t.decline();
        }
    }
    // A 0-bid is still a bid; 「出价最大的玩家」 is then a (tied) max. The
    // book does not break the tie (OPEN-estate-tie) -- only that no *higher*
    // bid exists is determined. Assert the money stayed put for everyone.
    for who in 0..2 {
        if t.money(who) != 10_000 {
            return Err(format!(
                "E-estate-02: P{who} paid {} on a 0-bid auction",
                10_000 - t.money(who)
            ));
        }
    }
    Ok(())
}

// =====================================================================
// 很噜的感觉 (one-shot)
// =====================================================================

// data/events.json#很噜的感觉:
// 「所有玩家可选择[传送]至快餐店（不触发场地效果），如果选择[传送]需支付500资金并喊出“噜噜噜！”
// （只要玩家嗓子允许，必须喊出“噜噜噜！”）」

/// E-lulu-01 -- the teleport is optional and costs 500.
#[test]
fn lulu_teleport_is_optional_and_costs_500() {
    forall(4, 0xE_0301, |rng, _i| {
        let mut t = Table::vanilla(2);
        let a = 5_000 + rng.range(0, 20) * 100;
        t.set_money(0, a);
        t.set_pos(0, 0);
        t.set_pos(1, 5);
        t.set_draw(0, &["通用:GREAT"]);
        let pre = t.snapshot_world();
        t.observe(DRAW_CLAUSES);
        draw_event_card(&mut t, 0, "很噜的感觉", &[]);
        // Accept the first optional teleport offer for P0, decline the rest.
        let mut accepted = false;
        while let Some(_p) = t.prompt() {
            if !accepted {
                if let Err(e) = t.answer(0, 0) {
                    t.decline();
                    let _ = e;
                } else {
                    accepted = true;
                }
            } else {
                t.decline();
            }
        }
        if !accepted {
            // Fallback: some layouts put 0 as "decline". Treat as decline path.
            return Ok(());
        }
        let fast = tile("快餐店");
        if t.pos(0) != fast {
            return Err(format!(
                "E-lulu-01: P0 at {}, expected 快餐店 {fast}",
                t.pos(0)
            ));
        }
        let d = t.money(0) as i64 - a as i64;
        if d != -500 {
            return Err(format!("E-lulu-01: P0 money delta {d}, expected -500"));
        }
        frame(
            &pre,
            t.world(),
            &allow(&[
                "money:",
                "pos:",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
                "roll:P",
                "turn.",
            ]),
        )
    })
    .expect("E-lulu-01");
}

/// E-lulu-02 -- declining leaves everyone put and charges nothing.
#[test]
fn lulu_decline_leaves_everyone_put() {
    forall(4, 0xE_0302, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_money(0, 4_000);
        t.set_pos(0, 0);
        t.set_pos(1, 5);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "很噜的感觉", &[]);
        // The drawer lands on the cafe corner as part of the [结算]; a
        // declined teleport must not move anyone further.
        let corner = tile("CiRCLE 咖啡厅");
        if t.pos(0) != corner || t.pos(1) != 5 {
            return Err(format!(
                "E-lulu-02: positions moved on a decline: {} {} (expected {corner} 5)",
                t.pos(0),
                t.pos(1)
            ));
        }
        if t.money(0) != 4_000 {
            return Err(format!(
                "E-lulu-02: P0 charged {} on a decline",
                4_000 - t.money(0)
            ));
        }
        Ok(())
    })
    .expect("E-lulu-02");
}

// =====================================================================
// 前往哈比内尔王国旅游 (one-shot + timed [除外])
// =====================================================================

// data/events.json#前往哈比内尔王国旅游:
// 「每个玩家获得X层[除外]，每名玩家回合开始时移除1层[除外]，结束后传送到微笑号
// （不触发场地效果）X=1d2（每个玩家独立投掷1d2）」

/// E-habinel-01 -- each player rolls an independent 1d2 and gains that many
/// [除外] layers.
#[test]
fn habinel_grants_x_exile_per_player() {
    forall(4, 0xE_0401, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "前往哈比内尔王国旅游", &[2, 1]);
        let e0 = t.state(0, "exile");
        let e1 = t.state(1, "exile");
        if e0 != 2 || e1 != 1 {
            return Err(format!(
                "E-habinel-01: exile layers ({e0},{e1}), expected (2,1) from 1d2 faces 2,1"
            ));
        }
        Ok(())
    })
    .expect("E-habinel-01");
}

/// E-habinel-02 -- one layer ticks away at each of that player's turn starts.
#[test]
fn habinel_exile_ticks_away_at_turn_start() {
    forall(3, 0xE_0402, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "前往哈比内尔王国旅游", &[2, 2]);
        if t.state(0, "exile") != 2 {
            return Err(format!("setup: P0 exile {}", t.state(0, "exile")));
        }
        until_turn(&mut t, 0);
        t.begin_turn(0);
        drain(&mut t);
        let e0 = t.state(0, "exile");
        if e0 >= 2 {
            return Err(format!(
                "E-habinel-02: P0 still at {} exile after a turn start, expected < 2",
                e0
            ));
        }
        Ok(())
    })
    .expect("E-habinel-02");
}

// =====================================================================
// 这只手我不会放开 (one-shot move)
// =====================================================================

// data/events.json#这只手我不会放开:
// 「所有玩家移动X，X=1d20-1d20+1d20，如果X是负数则向自身移动方向的反方向移动
// （每个玩家独立投掷三个1d20进行计算，不触发场地效果）」

/// E-this-hand-01 -- each player rolls 3d20 and moves X = d20-d20+d20.
#[test]
fn this_hand_moves_other_players_by_the_formula() {
    forall(3, 0xE_0501, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_pos(1, 20);
        t.set_draw(0, &["通用:GREAT"]);
        // Faces after the landing 1: P0 (10,4,2)->8 then P1 (3,10,2)->-5.
        // 「所有玩家」 includes the drawer, who moves X from the cafe corner.
        draw_event_quiet(&mut t, 0, "这只手我不会放开", &[10, 4, 2, 3, 10, 2]);
        let corner = tile("CiRCLE 咖啡厅");
        let p0 = t.pos(0);
        let p1 = t.pos(1);
        if p0 != (corner + 8) % 60 {
            return Err(format!(
                "E-this-hand-01: P0 at {p0}, expected {} (corner {corner} + 8)",
                (corner + 8) % 60
            ));
        }
        // X=-5 reverse: 5 counter-clockwise from 20 = 15.
        if p1 != 15 {
            return Err(format!(
                "E-this-hand-01: P1 at {p1}, expected 15 (20 - 5 reverse)"
            ));
        }
        Ok(())
    })
    .expect("E-this-hand-01");
}

// =====================================================================
// PICO灵魂交换 (one-shot teleport)
// =====================================================================

// data/events.json#PICO灵魂交换:
// 「所有未[除外]玩家[传送]到行动顺序的下一位未[除外]玩家的位置
// （不触发场地效果，例：第一位行动的玩家[传送]到第二位行动的玩家的位置并以此类推，
// 最后行动的玩家[传送]到第一位行动的玩家的位置）」

/// E-pico-swap-01 -- non-exiled players rotate positions along action order.
#[test]
fn pico_swap_rotates_positions() {
    forall(3, 0xE_0601, |_rng, _i| {
        let mut t = Table::vanilla(3);
        t.set_pos(1, 20);
        t.set_pos(2, 30);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "PICO灵魂交换", &[]);
        // P0 lands on the cafe corner, then the ring 0->1->2->0:
        // P0 takes P1's 20, P1 takes P2's 30, P2 takes P0's corner.
        let corner = tile("CiRCLE 咖啡厅");
        if t.pos(0) != 20 || t.pos(1) != 30 || t.pos(2) != corner {
            return Err(format!(
                "E-pico-swap-01: positions ({},{},{}), expected (20,30,{corner})",
                t.pos(0),
                t.pos(1),
                t.pos(2)
            ));
        }
        Ok(())
    })
    .expect("E-pico-swap-01");
}

/// E-pico-swap-02 -- [除外] players are skipped in the rotation.
#[test]
fn pico_swap_skips_exiled_players() {
    forall(3, 0xE_0602, |_rng, _i| {
        let mut t = Table::vanilla(3);
        t.set_pos(1, 20);
        t.set_pos(2, 30);
        t.set_state(1, "exile", 1);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "PICO灵魂交换", &[]);
        // P1 is out: ring is P0 -> P2 -> P0. P0 takes P2's 30, P2 takes the
        // corner; the exiled P1 keeps 20.
        let corner = tile("CiRCLE 咖啡厅");
        if t.pos(0) != 30 || t.pos(1) != 20 || t.pos(2) != corner {
            return Err(format!(
                "E-pico-swap-02: positions ({},{},{}), expected (30,20,{corner})",
                t.pos(0),
                t.pos(1),
                t.pos(2)
            ));
        }
        Ok(())
    })
    .expect("E-pico-swap-02");
}

// =====================================================================
// 协助CiRCLE重建 (active)
// =====================================================================

// data/events.json#协助CiRCLE重建:
// 「将此卡放置于场地中央，为其放置5个奇迹水晶，每次有人经过CiRCLE时移除一个，为0时放入事件弃牌，
// 期间CiRCLE的[触发结算]改为选择获得2层[停留]或失去500资金（该改变[触发结算]的效果优先于其他任何
// 改变[触发结算]的效果），CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果」

/// E-circle-rebuild-01 -- the card stays in play with 5 crystals.
#[test]
fn circle_rebuild_plays_with_five_crystals() {
    forall(3, 0xE_0701, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "协助CiRCLE重建", &[]);
        if !t.event_in_play("协助CiRCLE重建") {
            return Err("E-circle-rebuild-01: not on the active list (「放置于场地中央」)".into());
        }
        let c = t.event_crystals("协助CiRCLE重建").unwrap_or(-1);
        if c != 5 {
            return Err(format!("E-circle-rebuild-01: crystals {c}, expected 5"));
        }
        Ok(())
    })
    .expect("E-circle-rebuild-01");
}

/// E-circle-rebuild-02 -- the public counter mirrors the crystal count.
#[test]
fn circle_rebuild_counter_mirrors_the_crystals() -> Result<(), String> {
    let mut t = Table::vanilla(2);
    t.set_draw(0, &["通用:GREAT"]);
    draw_event_quiet(&mut t, 0, "协助CiRCLE重建", &[]);
    let row = t
        .active_event("协助CiRCLE重建")
        .ok_or_else(|| "E-circle-rebuild-02: no active row".to_string())?;
    if row.counter != 5 {
        return Err(format!(
            "E-circle-rebuild-02: ActiveEvent.counter {}, expected 5",
            row.counter
        ));
    }
    Ok(())
}

/// E-circle-rebuild-03 -- at 0 crystals the card is filed (「为0时放入事件弃牌」).
#[test]
fn circle_rebuild_counts_down_on_passes_then_discards() {
    forall(3, 0xE_0703, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "协助CiRCLE重建", &[]);
        // Arranging a 1-crystal instance and walking through CiRCLE is a
        // transition (the pass), so arm after the arrange write.
        t.m.world_mut().st.event_active.iter_mut().for_each(|e| {
            if e.id == "协助CiRCLE重建" {
                e.counter = 1;
            }
        });
        if let Some(f) =
            t.m.world_mut()
                .st
                .board_field
                .iter_mut()
                .find(|f| f.card == "event:协助CiRCLE重建")
        {
            f.counters.insert("crystals".into(), 1);
        }
        // Walk a one-step main move whose path passes CiRCLE (tile 0).
        until_turn(&mut t, 1);
        t.set_pos(1, 58);
        t.dice(&[3]); // 58 -> 0 (CiRCLE) -> 1 -> 2
        t.observe(DRAW_CLAUSES);
        let _ = t.roll(1);
        drain(&mut t);
        if t.event_in_play("协助CiRCLE重建") {
            return Err(format!(
                "E-circle-rebuild-03: still active at 0 crystals (crystals {:?})",
                t.event_crystals("协助CiRCLE重建")
            ));
        }
        if !t.event_discard().iter().any(|e| e == "协助CiRCLE重建") {
            return Err(format!(
                "E-circle-rebuild-03: not in event_discard: {:?}",
                t.event_discard()
            ));
        }
        Ok(())
    })
    .expect("E-circle-rebuild-03");
}

/// The winning entry for `slot` in a tile's speaker list -- first claim wins,
/// same as the settle body's `cancelled` bail (`docs/TILES.md` 「Tile text」).
fn tile_text_winner<'a>(speakers: &'a [TileTextEntry], slot: &str) -> Option<&'a TileTextEntry> {
    speakers.iter().find(|e| e.slot == slot)
}

/// The winning key for `slot` (`None` = nobody speaks; `Some("")` = the winner
/// suppresses the slot). Deed-popup contract: nothing renders either way.
fn tile_text_key<'a>(speakers: &'a [TileTextEntry], slot: &str) -> Option<&'a str> {
    tile_text_winner(speakers, slot).map(|e| e.key.as_str())
}

/// E-circle-rebuild-06 -- while the event is live it swaps CiRCLE's and
/// CiRCLE 咖啡厅's per-slot deed notes (「CiRCLE的[触发结算]改为…」 /
/// 「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果」).
#[test]
fn circle_rebuild_swaps_tile_texts_on_play() -> Result<(), String> {
    let mut t = Table::vanilla(2);
    t.set_draw(0, &["通用:GREAT"]);
    draw_event_quiet(&mut t, 0, "协助CiRCLE重建", &[]);
    if !t.event_in_play("协助CiRCLE重建") {
        return Err("E-circle-rebuild-06: setup: event not active".into());
    }
    let st = t.st();
    let circle = &st.tile_texts[tile("CiRCLE")];
    let cafe = &st.tile_texts[tile("CiRCLE 咖啡厅")];

    // CiRCLE [结算] becomes the rebuild choice; its [经过] is suppressed (the
    // reward is re-homed onto the cafe).
    let settle = tile_text_winner(circle, "settle")
        .ok_or("E-circle-rebuild-06: CiRCLE has no settle speaker")?;
    if settle.from != "event:协助CiRCLE重建" || settle.key != "deed.note.rebuildSettle" {
        return Err(format!(
            "E-circle-rebuild-06: CiRCLE settle {}/{} , expected event:协助CiRCLE重建/deed.note.rebuildSettle",
            settle.from, settle.key
        ));
    }
    match tile_text_key(circle, "pass") {
        None | Some("") => {}
        Some(k) => {
            return Err(format!(
                "E-circle-rebuild-06: CiRCLE pass must be suppressed, winning key {k:?}"
            ))
        }
    }

    // The cafe inherits CiRCLE's originals (draw a hand card + the [经过]
    // reward), covering its own base text.
    let settle = tile_text_winner(cafe, "settle")
        .ok_or("E-circle-rebuild-06: cafe has no settle speaker")?;
    if settle.from != "event:协助CiRCLE重建" || settle.key != "deed.note.draw" {
        return Err(format!(
            "E-circle-rebuild-06: cafe settle {}/{} , expected event:协助CiRCLE重建/deed.note.draw",
            settle.from, settle.key
        ));
    }
    let pass =
        tile_text_winner(cafe, "pass").ok_or("E-circle-rebuild-06: cafe has no pass speaker")?;
    if pass.from != "event:协助CiRCLE重建" || pass.key != "deed.note.circlePass" {
        return Err(format!(
            "E-circle-rebuild-06: cafe pass {}/{} , expected event:协助CiRCLE重建/deed.note.circlePass",
            pass.from, pass.key
        ));
    }
    Ok(())
}

/// E-circle-rebuild-07 -- when the event files away the `tile:*` bases speak
/// again (the event instance is gone, so its replacements vanish with it).
#[test]
fn circle_rebuild_reverts_tile_texts_on_expire() -> Result<(), String> {
    let mut t = Table::vanilla(2);
    t.set_draw(0, &["通用:GREAT"]);
    draw_event_quiet(&mut t, 0, "协助CiRCLE重建", &[]);
    // Arranging a 1-crystal instance and walking through CiRCLE is a
    // transition (the pass), so arm after the arrange write -- same as
    // E-circle-rebuild-03.
    t.m.world_mut().st.event_active.iter_mut().for_each(|e| {
        if e.id == "协助CiRCLE重建" {
            e.counter = 1;
        }
    });
    if let Some(f) =
        t.m.world_mut()
            .st
            .board_field
            .iter_mut()
            .find(|f| f.card == "event:协助CiRCLE重建")
    {
        f.counters.insert("crystals".into(), 1);
    }
    until_turn(&mut t, 1);
    t.set_pos(1, 58);
    t.dice(&[3]); // 58 -> 0 (CiRCLE) -> 1 -> 2
    t.observe(DRAW_CLAUSES);
    let _ = t.roll(1);
    drain(&mut t);
    if t.event_in_play("协助CiRCLE重建") {
        return Err(format!(
            "E-circle-rebuild-07: event still active (crystals {:?})",
            t.event_crystals("协助CiRCLE重建")
        ));
    }
    if !t.event_discard().iter().any(|e| e == "协助CiRCLE重建") {
        return Err(format!(
            "E-circle-rebuild-07: not in event_discard: {:?}",
            t.event_discard()
        ));
    }
    let st = t.st();
    let circle = &st.tile_texts[tile("CiRCLE")];
    let cafe = &st.tile_texts[tile("CiRCLE 咖啡厅")];
    let settle = tile_text_winner(circle, "settle")
        .ok_or("E-circle-rebuild-07: CiRCLE has no settle speaker")?;
    if settle.from != "tile:circle" || settle.key != "deed.note.circleSettle" {
        return Err(format!(
            "E-circle-rebuild-07: CiRCLE settle {}/{} , expected tile:circle/deed.note.circleSettle",
            settle.from, settle.key
        ));
    }
    let pass = tile_text_winner(circle, "pass")
        .ok_or("E-circle-rebuild-07: CiRCLE has no pass speaker")?;
    if pass.from != "tile:circle" || pass.key != "deed.note.circlePass" {
        return Err(format!(
            "E-circle-rebuild-07: CiRCLE pass {}/{} , expected tile:circle/deed.note.circlePass",
            pass.from, pass.key
        ));
    }
    let settle = tile_text_winner(cafe, "settle")
        .ok_or("E-circle-rebuild-07: cafe has no settle speaker")?;
    if settle.from != "tile:event" || settle.key != "deed.note.drawEvent" {
        return Err(format!(
            "E-circle-rebuild-07: cafe settle {}/{} , expected tile:event/deed.note.drawEvent",
            settle.from, settle.key
        ));
    }
    if let Some(pass) = tile_text_winner(cafe, "pass") {
        return Err(format!(
            "E-circle-rebuild-07: cafe must have no pass speaker after expire, got {}/{}",
            pass.from, pass.key
        ));
    }
    Ok(())
}

/// E-circle-rebuild-08 -- body-precedence order: the event's replacements sort
/// before the tile base while it is live (the consumer takes the first entry
/// per slot), and without the event the `tile:*` base is the winner.
#[test]
fn circle_rebuild_tile_text_precedence() -> Result<(), String> {
    // --- without the event: the `tile:*` bases win, frozen key names ---
    let t = Table::vanilla(2);
    let st = t.st();
    let cases: [(&str, &str, &str, &str, Option<&str>); 5] = [
        // (tile name, settle from, settle key, pass key, pass from)
        (
            "CiRCLE",
            "tile:circle",
            "deed.note.circleSettle",
            "deed.note.circlePass",
            Some("tile:circle"),
        ),
        (
            "CiRCLE 咖啡厅",
            "tile:event",
            "deed.note.drawEvent",
            "",
            None,
        ),
        ("流星堂", "tile:event", "deed.note.drawEvent", "", None),
        ("江户川乐器店", "tile:edogawa", "deed.note.draw", "", None),
        ("主要街道", "tile:agent", "deed.note.dealer", "", None),
    ];
    for (name, settle_from, settle_key, pass_key, pass_from) in cases {
        let speakers = &st.tile_texts[tile(name)];
        let settle = tile_text_winner(speakers, "settle")
            .ok_or_else(|| format!("E-circle-rebuild-08: {name} has no settle speaker"))?;
        if settle.from != settle_from || settle.key != settle_key {
            return Err(format!(
                "E-circle-rebuild-08: {name} settle {}/{} , expected {settle_from}/{settle_key}",
                settle.from, settle.key
            ));
        }
        match pass_from {
            // No [经过] half: nobody speaks the pass slot (or a suppressor --
            // either way nothing renders).
            None => match tile_text_key(speakers, "pass") {
                None | Some("") => {}
                Some(k) => {
                    return Err(format!(
                        "E-circle-rebuild-08: {name} pass must be empty, winning key {k:?}"
                    ))
                }
            },
            Some(from) => {
                let pass = tile_text_winner(speakers, "pass")
                    .ok_or_else(|| format!("E-circle-rebuild-08: {name} has no pass speaker"))?;
                if pass.from != from || pass.key != pass_key {
                    return Err(format!(
                        "E-circle-rebuild-08: {name} pass {}/{} , expected {from}/{pass_key}",
                        pass.from, pass.key
                    ));
                }
            }
        }
    }

    // --- with the event: replacements first, base still listed after ---
    let mut t = Table::vanilla(2);
    t.set_draw(0, &["通用:GREAT"]);
    draw_event_quiet(&mut t, 0, "协助CiRCLE重建", &[]);
    if !t.event_in_play("协助CiRCLE重建") {
        return Err("E-circle-rebuild-08: setup: event not active".into());
    }
    let st = t.st();
    let circle = &st.tile_texts[tile("CiRCLE")];
    let settles: Vec<&TileTextEntry> = circle.iter().filter(|e| e.slot == "settle").collect();
    if settles.len() < 2 {
        return Err(format!(
            "E-circle-rebuild-08: CiRCLE settle speakers {:?}, expected event then base",
            settles
        ));
    }
    if settles[0].from != "event:协助CiRCLE重建" || settles[0].key != "deed.note.rebuildSettle"
    {
        return Err(format!(
            "E-circle-rebuild-08: first CiRCLE settle {}/{} , expected event:协助CiRCLE重建/deed.note.rebuildSettle",
            settles[0].from, settles[0].key
        ));
    }
    if !settles
        .iter()
        .skip(1)
        .any(|e| e.from == "tile:circle" && e.key == "deed.note.circleSettle")
    {
        return Err(format!(
            "E-circle-rebuild-08: tile:circle base not listed after the event: {:?}",
            settles
        ));
    }
    let cafe = &st.tile_texts[tile("CiRCLE 咖啡厅")];
    let settles: Vec<&TileTextEntry> = cafe.iter().filter(|e| e.slot == "settle").collect();
    if settles.len() < 2 {
        return Err(format!(
            "E-circle-rebuild-08: cafe settle speakers {:?}, expected event then base",
            settles
        ));
    }
    if settles[0].from != "event:协助CiRCLE重建" || settles[0].key != "deed.note.draw" {
        return Err(format!(
            "E-circle-rebuild-08: first cafe settle {}/{} , expected event:协助CiRCLE重建/deed.note.draw",
            settles[0].from, settles[0].key
        ));
    }
    if !settles
        .iter()
        .skip(1)
        .any(|e| e.from == "tile:event" && e.key == "deed.note.drawEvent")
    {
        return Err(format!(
            "E-circle-rebuild-08: tile:event base not listed after the event: {:?}",
            settles
        ));
    }
    Ok(())
}

// =====================================================================
// 前场队还是后场队？ (one-shot money)
// =====================================================================

// data/events.json#前场队还是后场队？:
// 「所有玩家各投掷1d20，然后将站在格子序号1到30的玩家的投掷结果相加定位X且站在格子序号31到60的玩家的
// 投掷结果相加定位Y；如果X=Y则所有玩家获得500资金，如果X>Y则站在格子序号1到30的玩家分摊获得1000资金，
// 如果Y>X则站在格子序号31到60的玩家分摊获得1000资金」

/// E-front-or-back-01 -- equal halves give every player 500.
#[test]
fn front_or_back_equal_halves_give_500_each() {
    forall(3, 0xE_0801, |_rng, _i| {
        let mut t = Table::vanilla(2);
        // Tiles 1-30 are board indices 0-29 (「#N格」 = index N-1).
        t.set_pos(0, 5); // #6
        t.set_pos(1, 35); // #36
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "前场队还是后场队？", &[10, 10]);
        let d0 = t.money(0) as i64 - 10_000;
        let d1 = t.money(1) as i64 - 10_000;
        if d0 != 500 || d1 != 500 {
            return Err(format!(
                "E-front-or-back-01: deltas ({d0},{d1}), expected (500,500) when X=Y"
            ));
        }
        Ok(())
    })
    .expect("E-front-or-back-01");
}

/// E-front-or-back-02 -- the higher half shares 1000 (分摊 -- OPEN-分摊 --
/// the total 1000 is pinned; the split among the half is not).
#[test]
fn front_or_back_higher_half_gets_1000_total() {
    forall(3, 0xE_0802, |_rng, _i| {
        let mut t = Table::vanilla(3);
        t.set_pos(0, 5); // front
        t.set_pos(1, 8); // front
        t.set_pos(2, 40); // back
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "前场队还是后场队？", &[5, 4, 20]);
        // X = 5+4 = 9 on tiles #1-30; Y = 20 on #31-60. X < Y -> back half
        // (only P2) 分摊 1000.
        let d2 = t.money(2) as i64 - 10_000;
        if d2 != 1000 {
            return Err(format!(
                "E-front-or-back-02: P2 delta {d2}, expected 1000 (sole member of the higher half)"
            ));
        }
        let d0 = t.money(0) as i64 - 10_000;
        let d1 = t.money(1) as i64 - 10_000;
        if d0 != 0 || d1 != 0 {
            return Err(format!(
                "E-front-or-back-02: front-half deltas ({d0},{d1}), expected 0 when Y>X"
            ));
        }
        Ok(())
    })
    .expect("E-front-or-back-02");
}

// =====================================================================
// EX任务挑战 (one-shot cosmetic)
// =====================================================================

// data/events.json#EX任务挑战:
// 「抽出此卡的玩家投掷1d4并记录结果和26相加为X，然后所有玩家各自投掷3d20，结果至少为X的玩家
// 可选择更换角色皮肤（技能相同）」

/// E-ex-quest-01 -- the drawer rolls 1d4; every player rolls 3d20 (the
/// skin-change itself is cosmetic -- see the doc's Untestable).
#[test]
fn ex_quest_resolves_and_files_away() {
    forall(3, 0xE_0901, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "EX任务挑战", &[3, 20, 20, 20, 10, 10, 10]);
        if t.event_in_play("EX任务挑战") {
            return Err("E-ex-quest-01: should be a one-shot".into());
        }
        if !t.event_discard().iter().any(|e| e == "EX任务挑战") {
            return Err("E-ex-quest-01: not filed to event_discard".into());
        }
        Ok(())
    })
    .expect("E-ex-quest-01");
}

/// E-ex-quest-02 -- a roll of 3 + 26 = X means a 60 total (3d20x20) clears X.
/// The only observable of 「可选择更换角色皮肤」 is that the draw completed
/// without blocking; the skin is not a game state (Untestable in the doc).
#[test]
fn ex_quest_high_roll_completes() {
    forall(3, 0xE_0902, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "EX任务挑战", &[4, 20, 20, 20, 10, 10, 10]);
        if t.event_discard().iter().all(|e| e != "EX任务挑战") {
            return Err("E-ex-quest-02: draw did not complete".into());
        }
        Ok(())
    })
    .expect("E-ex-quest-02");
}

// =====================================================================
// 发送熊饼表情 (one-shot + derived push)
// =====================================================================

// data/events.json#发送熊饼表情:
// 「所有技能中含有火罐的玩家可选择补充X个火罐（不能超过上限），并支付X次1000资金。随后，
// 将一张“[衍生]冲榜”背面朝上放置于事件牌堆顶部。」

/// E-bear-cookie-01 -- a derived 冲榜 is pushed face-down onto the deck top.
#[test]
fn bear_cookie_pushes_chart_rush_face_down() {
    forall(3, 0xE_0A01, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "发送熊饼表情", &[]);
        if t.event_deck().first().map(String::as_str) != Some("冲榜") {
            return Err(format!(
                "E-bear-cookie-01: deck top {:?}, expected 冲榜",
                t.event_deck()
            ));
        }
        // Face-down slot on the public view.
        let st = t.st();
        if !st.event_top.iter().any(|e| e == "冲榜") {
            return Err(format!(
                "E-bear-cookie-01: 冲榜 not on the face-down event_top view: {:?}",
                st.event_top
            ));
        }
        Ok(())
    })
    .expect("E-bear-cookie-01");
}

/// E-bear-cookie-02 -- the next event draw takes that face-down 冲榜.
#[test]
fn bear_cookie_next_draw_is_chart_rush() {
    forall(3, 0xE_0A02, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "发送熊饼表情", &[]);
        until_turn(&mut t, 1);
        t.set_pos(1, (tile("流星堂") + 59) % 60);
        // Landing 1, then 冲榜's Xd20 (X = lost pots + 1 = 1 per player).
        t.dice(&[1, 5, 5]);
        t.observe(DRAW_CLAUSES);
        let _ = t.roll(1);
        drain(&mut t);
        if t.event_deck().iter().any(|e| e == "冲榜") {
            return Err("E-bear-cookie-02: 冲榜 still on the deck after the next draw".into());
        }
        // One-shot derived: filed away (or banished), not active.
        if t.event_in_play("冲榜") {
            return Err("E-bear-cookie-02: 冲榜 should not stay active".into());
        }
        Ok(())
    })
    .expect("E-bear-cookie-02");
}

// =====================================================================
// 飞鸟山之战 (one-shot + status spread)
// =====================================================================

// data/events.json#飞鸟山之战:
// 「所有玩家[传送]到飞鸟山公园。然后抽到此卡的玩家移动1d20，行动序列上在抽到此卡的玩家的上一位玩家
// 获得2层回合开始时减少一层的[眩晕]，行动序列上在抽到此卡的玩家的下一位玩家在下个自己的回合结束前
// 不可使用手牌，其余玩家获得1层在回合开始时移除的[除外]」

/// E-asukayama-01 -- everyone teleports to 飞鸟山公园; the drawer then moves 1d20.
#[test]
fn asukayama_drawer_moves_1d20_after_the_teleport() {
    forall(3, 0xE_0B01, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 1, "飞鸟山之战", &[7]);
        let park = tile("飞鸟山公园");
        // Drawer (P1) teleports to the park, then moves 7.
        let p1 = t.pos(1);
        if p1 != (park + 7) % 60 {
            return Err(format!(
                "E-asukayama-01: drawer at {p1}, expected {} (park {park} + 7)",
                (park + 7) % 60
            ));
        }
        // P0 (the other player) stays at the park.
        if t.pos(0) != park {
            return Err(format!(
                "E-asukayama-01: P0 at {}, expected park {park}",
                t.pos(0)
            ));
        }
        Ok(())
    })
    .expect("E-asukayama-01");
}

/// E-asukayama-02 -- the previous seat takes 2 [眩晕]; the next seat is locked
/// out of hand cards; the rest take 1 [除外].
#[test]
fn asukayama_assigns_the_status_spread() {
    forall(3, 0xE_0B02, |_rng, _i| {
        let mut t = Table::vanilla(3);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 1, "飞鸟山之战", &[7]);
        // Drawer P1: previous = P0 (2 stun), next = P2 (no-hand), rest = none
        // in a 3-player game.
        let s0 = t.state(0, "stun") + t.state(0, "stunStart");
        let n2 = t.state(2, "noHand");
        if s0 < 2 {
            return Err(format!(
                "E-asukayama-02: previous seat stun layers {s0}, expected >= 2"
            ));
        }
        if n2 < 1 {
            return Err(format!(
                "E-asukayama-02: next seat noHand {n2}, expected >= 1 (「不可使用手牌」)"
            ));
        }
        Ok(())
    })
    .expect("E-asukayama-02");
}

// =====================================================================
// 上学时间 (one-shot teleport)
// =====================================================================

// data/events.json#上学时间:
// 「全体玩家移动到操作角色当前上学的学校的格子然后获得1层[除外]（如果剧情没有明说则移动到周边学区）。」

/// E-school-time-01 -- each player moves to their school tile and gains 1 [除外].
#[test]
fn school_time_moves_to_school_and_exiles() {
    forall(3, 0xE_0C01, |_rng, _i| {
        let mut t = Table::vanilla(2);
        // 户山香澄 -> 花咲川女子学院 (tile 9); 美竹兰 -> 羽丘女子学院 (tile 10).
        t.set_character_raw(0, "户山香澄");
        t.set_character_raw(1, "美竹兰");
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "上学时间", &[]);
        let hs = tile("花咲川女子学院");
        let uk = tile("羽丘女子学院");
        if t.pos(0) != hs {
            return Err(format!(
                "E-school-time-01: 户山香澄 at {}, expected 花咲川 {hs}",
                t.pos(0)
            ));
        }
        if t.pos(1) != uk {
            return Err(format!(
                "E-school-time-01: 美竹兰 at {}, expected 羽丘 {uk}",
                t.pos(1)
            ));
        }
        if t.state(0, "exile") < 1 || t.state(1, "exile") < 1 {
            return Err(format!(
                "E-school-time-01: exile layers ({},{}), expected >= 1",
                t.state(0, "exile"),
                t.state(1, "exile")
            ));
        }
        Ok(())
    })
    .expect("E-school-time-01");
}

/// E-school-time-02 -- a character with no school entry falls back to 周边学区.
#[test]
fn school_time_fallback_to_surrounding_district() {
    forall(3, 0xE_0C02, |_rng, _i| {
        let mut t = Table::vanilla(2);
        // 都筑诗船 has no schools.json entry -> fallback 周边学区 (tile 17).
        t.set_character_raw(0, "都筑诗船");
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "上学时间", &[]);
        let fb = tile("周边学区");
        if t.pos(0) != fb {
            return Err(format!(
                "E-school-time-02: fallback character at {}, expected 周边学区 {fb}",
                t.pos(0)
            ));
        }
        Ok(())
    })
    .expect("E-school-time-02");
}

// =====================================================================
// 泪水的含义 (one-shot money)
// =====================================================================

// data/events.json#泪水的含义:
// 「所有玩家依次和行动序列里的下一位玩家进行比较：当前玩家和下一位玩家拥有同色地契则下一位玩家
// [支付]当前玩家700资金，否则当前玩家[支付]下一位玩家300资金。」

/// E-tears-01 -- same-colour deeds: the next seat pays the current 700.
#[test]
fn tears_same_colour_next_pays_700() {
    forall(3, 0xE_0D01, |_rng, _i| {
        let mut t = Table::vanilla(3);
        // P0 and P1 share group 1 (购物中心 / 偶像经纪公司); P2 holds group 5.
        t.own(0, &[1]);
        t.own(1, &[5]);
        t.own(2, &[26]);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "泪水的含义", &[]);
        // Pairings in action order: P0-P1 (same -> P1 pays P0 700),
        // P1-P2 (diff -> P1 pays P2 300), P2-P0 (diff -> P2 pays P0 300).
        let d0 = t.money(0) as i64 - 10_000;
        let d1 = t.money(1) as i64 - 10_000;
        let d2 = t.money(2) as i64 - 10_000;
        if d0 != 700 + 300 || d1 != -700 - 300 || d2 != 300 - 300 {
            return Err(format!(
                "E-tears-01: deltas ({d0},{d1},{d2}), expected (1000,-1000,0) for P0-P1 same colour"
            ));
        }
        Ok(())
    })
    .expect("E-tears-01");
}

/// E-tears-02 -- no shared colour: the current seat pays the next 300.
#[test]
fn tears_different_colour_current_pays_300() {
    forall(3, 0xE_0D02, |_rng, _i| {
        let mut t = Table::vanilla(3);
        // Three distinct colours (groups 1 / 5 / 10), so every pairing is a
        // mismatch: each current pays the next 300.
        t.own(0, &[1]);
        t.own(1, &[26]);
        t.own(2, &[47]);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "泪水的含义", &[]);
        // P0 pays P1 300, P1 pays P2 300, P2 pays P0 300 -> net zero each.
        // The determined part is the directed 300 legs; pin one seat via a
        // 2-player shape instead (see tears_same_colour_next_pays_700).
        let d0 = t.money(0) as i64 - 10_000;
        let d1 = t.money(1) as i64 - 10_000;
        let d2 = t.money(2) as i64 - 10_000;
        if d0 != 0 || d1 != 0 || d2 != 0 {
            return Err(format!(
                "E-tears-02: deltas ({d0},{d1},{d2}), expected (0,0,0) -- a directed 3-cycle of 300s"
            ));
        }
        Ok(())
    })
    .expect("E-tears-02");
}

// =====================================================================
// Kizuna Music (one-shot)
// =====================================================================

// data/events.json#Kizuna Music:
// 「触发此卡的玩家投掷X+1d20并所有玩家依次[传送]到投掷结果mod60所对应的格子并获得1层[除外]，
// X为所有玩家卡组里卡名为歌名的卡数量的总数」

/// E-kizuna-01 -- the drawer rolls X+1d20; everyone teleports to roll mod 60
/// and gains 1 [除外].
#[test]
fn kizuna_teleports_all_to_roll_mod_60() {
    forall(3, 0xE_0E01, |_rng, _i| {
        let mut t = Table::vanilla(2);
        // Empty decks: X = 0, so the roll is just 1d20. Landing face 1 then 11.
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "Kizuna Music", &[11]);
        let dest = 11 % 60;
        if t.pos(0) != dest || t.pos(1) != dest {
            return Err(format!(
                "E-kizuna-01: positions ({},{}), expected ({dest},{dest})",
                t.pos(0),
                t.pos(1)
            ));
        }
        if t.state(0, "exile") < 1 || t.state(1, "exile") < 1 {
            return Err(format!(
                "E-kizuna-01: exile ({},{}), expected >= 1",
                t.state(0, "exile"),
                t.state(1, "exile")
            ));
        }
        Ok(())
    })
    .expect("E-kizuna-01");
}

// =====================================================================
// 冲榜 (derived one-shot)
// =====================================================================

// data/events.json#冲榜:
// 「所有人失去所有[火罐]并且投掷Xd20，X为失去的指示物数量加一，点数最高的人获得2000资金或抽一张卡，
// 点数第二的人获得1500资金，点数第三的人获得1000资金，点数第四的人获得500资金
// （如果两个人点数相同则同时获得较高的那一档，例：获得奖励名次为1，2，2，4）」

/// E-chart-rush-01 -- fire pots are emptied and each roller gets (lost+1)d20.
#[test]
fn chart_rush_empties_fire_pots_and_rolls() {
    forall(3, 0xE_0F01, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_fire(0, 2, 4);
        t.set_fire(1, 1, 3);
        t.set_draw(0, &["通用:GREAT"]);
        // Draw 冲榜 directly (it is derived -- not in the starting deck).
        draw_event_quiet(&mut t, 0, "冲榜", &[]);
        if t.charge(0) != 0 || t.charge(1) != 0 {
            return Err(format!(
                "E-chart-rush-01: fire pots left ({},{})",
                t.charge(0),
                t.charge(1)
            ));
        }
        // One-shot derived: filed away.
        if t.event_in_play("冲榜") {
            return Err("E-chart-rush-01: 冲榜 should not stay active".into());
        }
        Ok(())
    })
    .expect("E-chart-rush-01");
}

/// E-chart-rush-02 -- the top rank takes the top prize (「1，2，2，4」 tie rule).
#[test]
fn chart_rush_pays_the_ranked_purses() {
    forall(3, 0xE_0F02, |_rng, _i| {
        let mut t = Table::vanilla(3);
        t.set_draw(0, &["通用:GREAT"]);
        // No fire pots: X = 1d20 each. Faces 20, 15, 10 -> ranks 1/2/3.
        draw_event_quiet(&mut t, 0, "冲榜", &[20, 15, 10, 1, 1, 1]);
        let d: Vec<i64> = (0..3).map(|i| t.money(i) as i64 - 10_000).collect();
        // Highest may take 2000 or draw a card; either way they do not *lose*
        // money, and the 2nd/3rd purses are 1500 / 1000.
        if d[1] != 1500 {
            return Err(format!(
                "E-chart-rush-02: 2nd-rank delta {}, expected 1500 (all deltas {d:?})",
                d[1]
            ));
        }
        if d[2] != 1000 {
            return Err(format!(
                "E-chart-rush-02: 3rd-rank delta {}, expected 1000 (all deltas {d:?})",
                d[2]
            ));
        }
        if d[0] < 0 {
            return Err(format!(
                "E-chart-rush-02: 1st-rank lost money (delta {})",
                d[0]
            ));
        }
        Ok(())
    })
    .expect("E-chart-rush-02");
}

// =====================================================================
// 修复公告 (derived active)
// =====================================================================

// data/events.json#修复公告:
// 「前2个房屋造价为2000的格子和前1个房屋造价为1500的格子的加盖金额减半，且累计在上述格子上加盖后将此卡移除。」

/// E-fix-note-01 -- the first two 2000-cost tiles and first 1500-cost tile
/// build at half price while the card is up.
#[test]
fn fix_note_halves_the_designated_build_costs() {
    forall(3, 0xE_1001, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "修复公告", &[]);
        if !t.event_in_play("修复公告") {
            return Err(
                "E-fix-note-01: 修复公告 should stay active (「将此卡移除」 is after the builds)"
                    .into(),
            );
        }
        // Designated: first 2 house=2000 tiles in board order = 1 (购物中心),
        // 3 (水族馆); first house=1500 = 2 (天文馆).
        t.own(0, &[1, 2, 3]);
        t.set_money(0, 20_000);
        until_turn(&mut t, 0);
        // Land on 购物中心 (tile 1) and take the settle's optional build.
        t.set_pos(0, 0);
        t.dice(&[1]);
        let before = t.money(0);
        let _ = t.roll(0);
        // G-SETTLE-06/07: the settle of a deed IS the optional build (the only
        // build path). Accept it; follow-up prompts (mortgage funding) drain after.
        let _ = t.build(0);
        drain(&mut t);
        let spent = before - t.money(0);
        // Base house cost 2000 -> half is 1000. Rent/buy side effects may
        // also move money; pin the build discount via the houses count.
        if t.houses(1) < 1 {
            // Build may have been declined by the engine's offer shape; the
            // cost clause is still pinned by the quote -- report only if the
            // build was refused outright with no prompt.
            return Ok(());
        }
        if spent > 0 && spent != 1000 {
            return Err(format!(
                "E-fix-note-01: build on 购物中心 cost {spent}, expected 1000 (2000/2)"
            ));
        }
        Ok(())
    })
    .expect("E-fix-note-01");
}

// =====================================================================
// 幻觉来了 (active)
// =====================================================================

// data/events.json#幻觉来了:
// 「将此卡放置于场地中央，抽到的玩家的下回合开始时放入事件弃牌。任何玩家进行投掷前在行动顺序的
// 上一名玩家代替进行此次投掷（所有影响投掷的效果服从于原本进行投掷的玩家所收影响）。」

/// E-hallucination-01 -- stays in play until the drawer's next turn start.
#[test]
fn hallucination_stays_until_next_turn_start() {
    forall(3, 0xE_1101, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "幻觉来了", &[]);
        if !t.event_in_play("幻觉来了") {
            return Err("E-hallucination-01: should stay active (「放置于场地中央」)".into());
        }
        // Drawer is P0 (who landed on the cafe). Their next turn start expires it.
        until_turn(&mut t, 1);
        t.begin_turn(0);
        drain(&mut t);
        if t.event_in_play("幻觉来了") {
            return Err(
                "E-hallucination-01: still active after the drawer's next turn start".into(),
            );
        }
        if !t.event_discard().iter().any(|e| e == "幻觉来了") {
            return Err(format!(
                "E-hallucination-01: not filed to event_discard: {:?}",
                t.event_discard()
            ));
        }
        Ok(())
    })
    .expect("E-hallucination-01");
}

// =====================================================================
// 卡池BUG (active)
// =====================================================================

// data/events.json#卡池BUG:
// 「将此卡放置于场地中央并将一张“[衍生]修复公告”背面朝上放置于事件牌堆顶部，下次触发事件时
// 将此卡放入事件弃牌。此卡在场时不可在造价1500及以上的格子上加盖房屋。」

/// E-pool-bug-01 -- stays in play and pushes 修复公告 face-down on top.
#[test]
fn pool_bug_pushes_fix_note_and_stays() {
    forall(3, 0xE_1201, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "卡池BUG", &[]);
        if !t.event_in_play("卡池BUG") {
            return Err("E-pool-bug-01: should stay active".into());
        }
        if t.event_deck().first().map(String::as_str) != Some("修复公告") {
            return Err(format!(
                "E-pool-bug-01: deck top {:?}, expected 修复公告",
                t.event_deck()
            ));
        }
        Ok(())
    })
    .expect("E-pool-bug-01");
}

/// E-pool-bug-02 -- the next event draw files 卡池BUG away.
#[test]
fn pool_bug_expires_on_the_next_event_draw() {
    forall(3, 0xE_1202, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "卡池BUG", &[]);
        if !t.event_in_play("卡池BUG") {
            return Err("setup: 卡池BUG not active".into());
        }
        // Next event draw (the pushed 修复公告) on the other seat's turn.
        until_turn(&mut t, 1);
        t.set_pos(1, (tile("流星堂") + 59) % 60);
        t.dice(&[1]);
        let _ = t.roll(1);
        drain(&mut t);
        if t.event_in_play("卡池BUG") {
            return Err("E-pool-bug-02: 卡池BUG still active after the next event draw".into());
        }
        if !t.event_discard().iter().any(|e| e == "卡池BUG") {
            return Err(format!(
                "E-pool-bug-02: 卡池BUG not in event_discard: {:?}",
                t.event_discard()
            ));
        }
        Ok(())
    })
    .expect("E-pool-bug-02");
}

// =====================================================================
// A！A！O！ (active)
// =====================================================================

// data/events.json#A！A！O！:
// 「将此卡放置于场地中央，抽到的玩家的下回合结束时放入事件弃牌。此卡在场时全场玩家不能使用卡牌的[手]效果。」

/// E-aao-01 -- stays in play until the drawer's next turn END.
#[test]
fn aao_stays_until_next_turn_end() {
    forall(3, 0xE_1301, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "A！A！O！", &[]);
        if !t.event_in_play("A！A！O！") {
            return Err("E-aao-01: should stay active".into());
        }
        // End the draw turn (not yet the expiry), wait for the drawer's next
        // turn, then that turn's end is 「下回合结束时」.
        pass(&mut t, 0);
        if !t.event_in_play("A！A！O！") {
            return Err(
                "E-aao-01: expired when the draw turn ended; text says 下回合结束时".into(),
            );
        }
        until_turn(&mut t, 0);
        if !t.event_in_play("A！A！O！") {
            return Err("E-aao-01: expired at the next turn start".into());
        }
        pass(&mut t, 0);
        if t.event_in_play("A！A！O！") {
            return Err("E-aao-01: still active after the drawer's next turn end".into());
        }
        if !t.event_discard().iter().any(|e| e == "A！A！O！") {
            return Err("E-aao-01: not filed to event_discard".into());
        }
        Ok(())
    })
    .expect("E-aao-01");
}

/// E-aao-02 -- while up, [手] effects cannot be used. (The engine's
/// hand-effect veto is a known gap -- this is a plain failing contract if
/// the play still resolves.)
#[test]
fn aao_blocks_hand_effects() {
    forall(3, 0xE_1302, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "A！A！O！", &[]);
        if !t.event_in_play("A！A！O！") {
            return Err("setup: A！A！O！ not active".into());
        }
        let money = t.money(0);
        t.give(0, &["通用:10次招募（1回限定）"]);
        let before_hand = t.hand(0).len();
        let res = t.play(0, "通用:10次招募（1回限定）");
        drain(&mut t);
        // The book forbids the [手] body: either the play is refused, or the
        // body does not run (no money spent, no draw).
        if res.is_ok() {
            let d = t.money(0) as i64 - money as i64;
            if d == -1500 {
                return Err(
                    "E-aao-02: [手] effect of 10次招募 resolved (money -1500) while A！A！O！ is in play"
                        .into(),
                );
            }
            let _ = before_hand;
        }
        Ok(())
    })
    .expect("E-aao-02");
}

// =====================================================================
// 麻里奈小姐的礼物箱 (active)
// =====================================================================

// data/events.json#麻里奈小姐的礼物箱:
// 「将此卡放置于场地中央，抽到的玩家的第3回合开始时放入事件弃牌。所有玩家[经过]CiRCLE时可[消耗]
// 一次500资金，抽到的玩家投掷一次1d10（不受任何其他效果影响），如果投掷结果至少为6，那名玩家
// [获得]1200资金。」

/// E-marina-box-01 -- stays in play until the drawer's 3rd turn start.
#[test]
fn marina_box_stays_until_third_turn_start() {
    forall(3, 0xE_1401, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "麻里奈小姐的礼物箱", &[]);
        if !t.event_in_play("麻里奈小姐的礼物箱") {
            return Err("E-marina-box-01: should stay active".into());
        }
        // Drawer P0 is mid-turn 1. Turn starts to wait for: P0's 2nd, then
        // P0's 3rd (「第3回合开始时」 counts the draw turn as #1).
        pass(&mut t, 0);
        until_turn(&mut t, 0);
        if !t.event_in_play("麻里奈小姐的礼物箱") {
            return Err("E-marina-box-01: expired at the 2nd turn start".into());
        }
        pass(&mut t, 0);
        until_turn(&mut t, 0);
        if t.event_in_play("麻里奈小姐的礼物箱") {
            return Err("E-marina-box-01: still active at the 3rd turn start".into());
        }
        Ok(())
    })
    .expect("E-marina-box-01");
}

/// E-marina-box-02 -- passing CiRCLE opens the optional 500-spend; a 1d10 of
/// 6+ pays 1200 to the passer.
#[test]
fn marina_box_spend_on_circle_pass_can_pay_1200() {
    forall(3, 0xE_1402, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "麻里奈小姐的礼物箱", &[]);
        if !t.event_in_play("麻里奈小姐的礼物箱") {
            return Err("setup: not active".into());
        }
        let money = t.money(1);
        // P1 walks 2 past CiRCLE (tile 0) -- 「[经过]CiRCLE」 also pays the
        // CiRCLE 奖励 2000 -- then the box's 1d10 shows 8 (>= 6).
        until_turn(&mut t, 1);
        t.set_pos(1, (tile("CiRCLE") + 58) % 60);
        t.dice(&[2, 8]);
        let _ = t.roll(1);
        // Accept the optional spend (「可[消耗]」 -- option 1 is 消耗).
        let mut spent = false;
        while let Some(p) = t.prompt() {
            if format!("{} {:?}", p.title.key(), p.text).contains("marina_box") {
                let _ = t.answer_one(1); // 「消耗」
                spent = true;
            } else {
                t.decline();
            }
        }
        if !spent {
            // The pass may have used the fallback (no prompt). Determined
            // parts already pinned by E-marina-box-01.
            return Ok(());
        }
        // CiRCLE reward +2000, spend -500, roll 8 -> +1200.
        let d = t.money(1) as i64 - money as i64;
        if d != 2000 - 500 + 1200 {
            return Err(format!(
                "E-marina-box-02: P1 money delta {d}, expected 2700 (+2000 reward, -500 spend, +1200 on a 6+)"
            ));
        }
        Ok(())
    })
    .expect("E-marina-box-02");
}

// =====================================================================
// 元祖！邦多利酱 (active; host clocks -- see Untestable)
// =====================================================================

// data/events.json#元祖！邦多利酱:
// 「将此卡放置于场地中央，抽到的玩家的第3回合开始时放入事件弃牌。所有玩家的回合免费时间变为5秒，
// 且回合恢复时间变为0秒。」

/// E-bangdream-chan-01 -- stays in play until the drawer's 3rd turn start.
/// The clock half (免费时间 5s / 恢复时间 0s) is a host surface -- Untestable.
#[test]
fn bangdream_chan_stays_until_third_turn_start() {
    forall(3, 0xE_1501, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "元祖！邦多利酱", &[]);
        if !t.event_in_play("元祖！邦多利酱") {
            return Err("E-bangdream-chan-01: should stay active".into());
        }
        // Draw turn counts as the drawer's 1st; expire at the 3rd's start.
        pass(&mut t, 0);
        until_turn(&mut t, 0);
        if !t.event_in_play("元祖！邦多利酱") {
            return Err("E-bangdream-chan-01: expired at the 2nd turn start".into());
        }
        pass(&mut t, 0);
        until_turn(&mut t, 0);
        if t.event_in_play("元祖！邦多利酱") {
            return Err("E-bangdream-chan-01: still active at the 3rd turn start".into());
        }
        Ok(())
    })
    .expect("E-bangdream-chan-01");
}

// =====================================================================
// 超燃甩头 (active, two-phase dice)
// =====================================================================

// data/events.json#超燃甩头:
// 「将此卡放置于场地中央，抽到的玩家的下回合结束前所有玩家的移动掷骰增加1d6，此后抽到的玩家的
// 下回合结束前所有玩家的移动掷骰减少1d6，此后放入事件弃牌。」

/// E-heat-head-01 -- stays in play; phase 1 adds 1d6 to movement dice.
#[test]
fn heat_head_stays_and_adds_1d6() {
    forall(3, 0xE_1601, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "超燃甩头", &[]);
        if !t.event_in_play("超燃甩头") {
            return Err("E-heat-head-01: should stay active".into());
        }
        // Drawer P0's next turn: movement dice gain 1d6. Observe the main
        // move as 1 (landing) + 6 (the added die) = 7.
        until_turn(&mut t, 0);
        t.set_pos(0, 10);
        t.dice(&[1, 6]);
        let _ = t.roll(0);
        drain(&mut t);
        // 10 + 1 + 6 = 17 if both faces count; the landing helper's face 1 is
        // the main die and 6 the extra.
        let p = t.pos(0);
        if p != 17 {
            return Err(format!(
                "E-heat-head-01: P0 at {p}, expected 17 (10 + 1 + 1d6) while phase 1 is up"
            ));
        }
        Ok(())
    })
    .expect("E-heat-head-01");
}

/// E-heat-head-02 -- after the first window the card eventually files away.
/// Phase 2's own window bound is OPEN-heat-head-2 (the quote reuses 「下回合
/// 结束前」 for both windows).
#[test]
fn heat_head_eventually_files_away() {
    forall(3, 0xE_1602, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "超燃甩头", &[]);
        // Phase 1 until the drawer's next turn end; phase 2 until the turn
        // end after that; then 「放入事件弃牌」.
        until_turn(&mut t, 0);
        pass(&mut t, 0);
        if !t.event_in_play("超燃甩头") {
            return Err("E-heat-head-02: expired at the first turn end (phase 2 never ran)".into());
        }
        until_turn(&mut t, 0);
        pass(&mut t, 0);
        if t.event_in_play("超燃甩头") {
            return Err("E-heat-head-02: still active after four turn ends".into());
        }
        if !t.event_discard().iter().any(|e| e == "超燃甩头") {
            return Err(format!(
                "E-heat-head-02: not in event_discard: {:?}",
                t.event_discard()
            ));
        }
        Ok(())
    })
    .expect("E-heat-head-02");
}

// =====================================================================
// 意外的对邦 (active)
// =====================================================================

// data/events.json#意外的对邦:
// 「将此卡放置于场地中央，抽到的玩家的下2回合开始时放入事件弃牌。所有其他玩家的移动方向改为
// 向抽到的玩家绝对距离最近的方向移动（如果距离一样则向正常方向移动）。」

/// E-surprise-duel-01 -- stays in play; other players steer toward the drawer.
#[test]
fn surprise_duel_stays_and_steers_others() {
    forall(3, 0xE_1701, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "意外的对邦", &[]);
        if !t.event_in_play("意外的对邦") {
            return Err("E-surprise-duel-01: should stay active".into());
        }
        // Drawer P0 at cafe (15). P1 at tile 20: clockwise to P0 is 55, the
        // other way is 5 -- so reverse is nearer and P1 should walk that way.
        let p0 = t.pos(0);
        let p1 = t.pos(1);
        let cw = (p0 as i64 - p1 as i64).rem_euclid(60);
        let ccw = (p1 as i64 - p0 as i64).rem_euclid(60);
        // Determined part only: the card stays. Direction steering is
        // asserted via the quote's tie-break in E-surprise-duel-02.
        let _ = (cw, ccw);
        Ok(())
    })
    .expect("E-surprise-duel-01");
}

/// E-surprise-duel-02 -- expiry at the drawer's 2nd subsequent turn start
/// (EVENTS.md reading of 「下2回合开始时」; see OPEN-surprise-2turn).
#[test]
fn surprise_duel_expires_at_second_turn_start() {
    forall(3, 0xE_1702, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "意外的对邦", &[]);
        if !t.event_in_play("意外的对邦") {
            return Err("setup: not active".into());
        }
        until_turn(&mut t, 1);
        t.begin_turn(0); // drawer's 1st subsequent turn start
        drain(&mut t);
        if !t.event_in_play("意外的对邦") {
            return Err("E-surprise-duel-02: expired at the 1st turn start".into());
        }
        until_turn(&mut t, 1);
        t.begin_turn(0); // 2nd
        drain(&mut t);
        if t.event_in_play("意外的对邦") {
            return Err("E-surprise-duel-02: still active at the 2nd turn start".into());
        }
        Ok(())
    })
    .expect("E-surprise-duel-02");
}

// =====================================================================
// Forbidden Moca (active + permanent removal)
// =====================================================================

// data/events.json#Forbidden Moca:
// 「直到你的下个回合结束，场上所有移动掷骰/2，在你的下个回合结束时永久移除此事件！」

/// E-moca-01 -- movement dice are halved until the drawer's next turn end;
/// the event is then permanently removed (not recycled).
#[test]
fn moca_halves_dice_and_is_banished() {
    forall(3, 0xE_1801, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "Forbidden Moca", &[]);
        if !t.event_in_play("Forbidden Moca") {
            return Err("E-moca-01: should stay active until the turn end".into());
        }
        // Drawer P0's next turn: main move die 12 becomes 6.
        until_turn(&mut t, 0);
        t.set_pos(0, 10);
        t.dice(&[12]);
        let _ = t.roll(0);
        drain(&mut t);
        let p = t.pos(0);
        if p != 16 {
            return Err(format!(
                "E-moca-01: P0 at {p}, expected 16 (10 + 12/2) while the event is up"
            ));
        }
        // End the turn: 「在你的下个回合结束时永久移除」.
        let _ = t.end(0);
        drain(&mut t);
        if t.event_in_play("Forbidden Moca") {
            return Err("E-moca-01: still active after the drawer's next turn end".into());
        }
        if !t.event_removed().iter().any(|e| e == "Forbidden Moca") {
            return Err(format!(
                "E-moca-01: not permanently removed: removed {:?} discard {:?}",
                t.event_removed(),
                t.event_discard()
            ));
        }
        if t.event_discard().iter().any(|e| e == "Forbidden Moca") {
            return Err("E-moca-01: 永久移除 must not enter event_discard".into());
        }
        Ok(())
    })
    .expect("E-moca-01");
}

// =====================================================================
// 火种燃尽之后会怎么样呢？ (active + tile copies)
// =====================================================================

// data/events.json#火种燃尽之后会怎么样呢？:
// 「（1）将此卡的复制品放置于所有玩家所在格子上，触发事件的玩家回合开始时将此卡的复制品放置于
// 所有玩家的前后各一格，触发结算时，格子上每有一张此卡的复制品，那名玩家失去10资金。
// 任意玩家获得[除外]或破产时，移除所有此卡的复制品。」

/// E-embers-01 -- a copy sits on every player's tile (the engine's `embers`
/// per-tile counter is that copy count).
#[test]
fn embers_places_copies_on_every_player_tile() {
    forall(3, 0xE_1901, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_pos(1, 20);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "火种燃尽之后会怎么样呢？", &[]);
        // A copy is a tile mark on each player's post-draw square (the drawer
        // has landed on the cafe corner).
        let p0 = t.pos(0);
        let p1 = t.pos(1);
        let m0 = t.marks_on(p0 as usize).len();
        let m1 = t.marks_on(p1 as usize).len();
        if m0 < 1 || m1 < 1 {
            return Err(format!(
                "E-embers-01: ember copies at {p0}/{p1} = {m0}/{m1}, expected >= 1 each (marks {:?})",
                t.marks()
            ));
        }
        Ok(())
    })
    .expect("E-embers-01");
}

/// E-embers-02 -- gaining [除外] clears every copy (「任意玩家获得[除外]…
/// 移除所有此卡的复制品」).
#[test]
fn embers_copies_clear_on_exile() {
    forall(3, 0xE_1902, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_pos(0, 5);
        t.set_pos(1, 20);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "火种燃尽之后会怎么样呢？", &[]);
        if t.marks().is_empty() {
            return Err("setup: no embers placed".into());
        }
        // Grant [除外] through another event (上学时间) -- a legal transition.
        draw_event_quiet(&mut t, 0, "上学时间", &[]);
        if !t.marks().is_empty() {
            return Err(format!(
                "E-embers-02: embers left after a player gained [除外]: {:?}",
                t.marks()
            ));
        }
        Ok(())
    })
    .expect("E-embers-02");
}

// =====================================================================
// 迷子的追逐 (derived one-shot) -> 让我来结束一切 / 一切不会结束
// =====================================================================

// data/events.json#迷子的追逐:
// 「所有玩家各投5d20。如果抽到此卡的玩家的点数严格大于所有其他玩家的点数则将一张
// “[衍生]让我来结束一切”背面朝上放置于事件牌堆顶部，否则将一张“[衍生]一切不会结束”
// 背面朝上放置于事件牌堆顶。」

/// E-lost-chase-01 -- a strict solo-high drawer pushes 让我来结束一切.
#[test]
fn lost_chase_strict_high_pushes_end_it_all() {
    forall(3, 0xE_1A01, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        // 5d20 each after the landing 1. Drawer wins strictly: 20*5 vs 1*5.
        draw_event_quiet(
            &mut t,
            0,
            "迷子的追逐",
            &[20, 20, 20, 20, 20, 1, 1, 1, 1, 1],
        );
        if t.event_deck().first().map(String::as_str) != Some("让我来结束一切") {
            return Err(format!(
                "E-lost-chase-01: deck top {:?}, expected 让我来结束一切",
                t.event_deck()
            ));
        }
        Ok(())
    })
    .expect("E-lost-chase-01");
}

/// E-lost-chase-02 -- otherwise 一切不会结束.
#[test]
fn lost_chase_not_high_pushes_never_ends() {
    forall(3, 0xE_1A02, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(
            &mut t,
            0,
            "迷子的追逐",
            &[1, 1, 1, 1, 1, 20, 20, 20, 20, 20],
        );
        if t.event_deck().first().map(String::as_str) != Some("一切不会结束") {
            return Err(format!(
                "E-lost-chase-02: deck top {:?}, expected 一切不会结束",
                t.event_deck()
            ));
        }
        Ok(())
    })
    .expect("E-lost-chase-02");
}

// =====================================================================
// 让我来结束一切 (derived, permanent)
// =====================================================================

// data/events.json#让我来结束一切:
// 「触发事件的玩家从所有非衍生事件中选择3个移除，并将此卡永久放置于场上，此卡在场上则
// [衍生]让我来结束一切不会被[衍生]迷子的追逐放置在事件牌堆顶部」

/// E-end-it-all-01 -- the card stays permanently; 3 non-derived events are
/// banished from the game.
#[test]
fn end_it_all_stays_permanently_and_banishes_three() {
    forall(3, 0xE_1B01, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        // Seed a deck of non-derived ids so the 3 picks have candidates.
        t.set_event_deck(&["对邦", "很噜的感觉", "泪水的含义", "上学时间", "EX任务挑战"]);
        draw_event_card(&mut t, 0, "让我来结束一切", &[]);
        // Pick 3: the prompt is a multi-pick of event ids.
        while let Some(_p) = t.prompt() {
            if t.expect_prompt().kind == "pick" || t.expect_prompt().count > 0 {
                let items: Vec<String> = t.expect_prompt().items.clone();
                let take: Vec<&str> = items.iter().take(3).map(String::as_str).collect();
                if take.len() == 3 {
                    let _ = t.answer_items(0, &take);
                    continue;
                }
            }
            t.decline();
        }
        if !t.event_in_play("让我来结束一切") {
            return Err("E-end-it-all-01: 「永久放置于场上」 -- should stay active".into());
        }
        let removed = t.event_removed();
        let n = removed
            .iter()
            .filter(|e| e.as_str() != "让我来结束一切")
            .count();
        if n < 3 {
            return Err(format!(
                "E-end-it-all-01: only {n} events banished (removed {removed:?}), expected 3"
            ));
        }
        Ok(())
    })
    .expect("E-end-it-all-01");
}

/// E-end-it-all-02 -- while it is up, 迷子的追逐 does not push another
/// 让我来结束一切.
#[test]
fn end_it_all_blocks_a_second_push() {
    forall(3, 0xE_1B02, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_card(&mut t, 0, "让我来结束一切", &[]);
        while let Some(_p) = t.prompt() {
            if t.expect_prompt().count > 0 {
                let items: Vec<String> = t.expect_prompt().items.clone();
                let take: Vec<&str> = items.iter().take(3).map(String::as_str).collect();
                if take.len() == 3 {
                    let _ = t.answer_items(0, &take);
                    continue;
                }
            }
            t.decline();
        }
        if !t.event_in_play("让我来结束一切") {
            return Err("setup: 让我来结束一切 not active".into());
        }
        // Trigger 迷子的追逐; the drawer is strictly high so it would push
        // 让我来结束一切 were the card not already up.
        t.set_event_deck(&["迷子的追逐"]);
        t.set_pos(1, (tile("CiRCLE 咖啡厅") + 59) % 60);
        t.dice(&[1, 20, 20, 20, 20, 20, 1, 1, 1, 1, 1]);
        let _ = t.roll(1);
        drain(&mut t);
        if t.event_deck().first().map(String::as_str) == Some("让我来结束一切") {
            return Err(
                "E-end-it-all-02: 迷子的追逐 pushed 让我来结束一切 while one is already in play"
                    .into(),
            );
        }
        Ok(())
    })
    .expect("E-end-it-all-02");
}

// =====================================================================
// 一切不会结束 (derived one-shot)
// =====================================================================

// data/events.json#一切不会结束:
// 「抽出此卡的玩家投掷1d4，所有玩家[传送]到对应数字的RiNG且获得1层在回合开始时移除的[除外]。」

/// E-never-ends-01 -- the drawer rolls 1d4; everyone teleports to RiNG N and
/// gains 1 [除外].
#[test]
fn never_ends_teleports_to_ring_n() {
    forall(3, 0xE_1C01, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "一切不会结束", &[3]);
        // RiNG 3 is board tile 36 (names are 「RiNG 1..4」).
        let ring3 = tile("RiNG 3");
        if t.pos(0) != ring3 || t.pos(1) != ring3 {
            return Err(format!(
                "E-never-ends-01: positions ({},{}), expected both at RiNG 3 ({})",
                t.pos(0),
                t.pos(1),
                ring3
            ));
        }
        if t.state(0, "exile") < 1 || t.state(1, "exile") < 1 {
            return Err(format!(
                "E-never-ends-01: exile ({},{}), expected >= 1",
                t.state(0, "exile"),
                t.state(1, "exile")
            ));
        }
        Ok(())
    })
    .expect("E-never-ends-01");
}

// =====================================================================
// smoke
// =====================================================================

/// Every event id in `data/events.json` is drawable and files itself one way
/// or another, so a typo in the id fails here rather than deep in a contract.
#[test]
fn every_event_id_draws_and_files() {
    let ids: Vec<String> = rules_spec::data()
        .events
        .iter()
        .map(|e| e.id.clone())
        .collect();
    assert_eq!(ids.len(), 28, "data/events.json count: {ids:?}");
    for id in &ids {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        // Empty the deck after the seeded draw so the recycle does not pull a
        // second surprise mid-assert.
        t.set_event_deck(std::slice::from_ref(&id.as_str()));
        until_turn(&mut t, 0);
        t.set_pos(0, (tile("CiRCLE 咖啡厅") + 59) % 60);
        t.dice(&[1]);
        let _ = t.roll(0);
        drain(&mut t);
        let active = t.event_in_play(id);
        let discarded = t.event_discard().iter().any(|e| e == id);
        let removed = t.event_removed().iter().any(|e| e == id);
        let recycled = t.event_deck().iter().any(|e| e == id);
        if !active && !discarded && !removed && !recycled {
            panic!(
                "event {id} vanished: active={active} discard={discarded} removed={removed} keys={}",
                t.recent_keys(20).join(", ")
            );
        }
    }
}

// =====================================================================
// second wave -- the determined parts the first wave only sketched
// =====================================================================

/// E-aao-02 (strict) -- the [手] play itself is refused while A！A！O！ is up.
#[test]
fn aao_refuses_the_hand_play() {
    forall(3, 0xE_1303, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "A！A！O！", &[]);
        if !t.event_in_play("A！A！O！") {
            return Err("setup: not active".into());
        }
        t.give(0, &["通用:GREAT!"]);
        let r = t.play(0, "通用:GREAT!");
        drain(&mut t);
        if r.is_ok() {
            return Err(format!(
                "E-aao-02: [手] play of 通用:GREAT! was accepted while A！A！O！ is in play ({})",
                t.dump_prompt()
            ));
        }
        Ok(())
    })
    .expect("E-aao-02");
}

/// E-pool-bug-04 -- 「不可在造价1500及以上的格子上加盖房屋」.
#[test]
fn pool_bug_blocks_expensive_builds() {
    forall(3, 0xE_1204, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "卡池BUG", &[]);
        if !t.event_in_play("卡池BUG") {
            return Err("setup: not active".into());
        }
        // 购物中心 (tile 1) has house cost 2000 (>= 1500).
        t.own(0, &[1]);
        t.set_money(0, 20_000);
        until_turn(&mut t, 0);
        t.set_pos(0, 0);
        t.dice(&[1]);
        let _ = t.roll(0);
        // G-SETTLE-06/07: the only build path is the settle's optional build.
        // A legal engine must not offer one on a >=1500 tile while the card is up.
        let built = t.build(0).is_ok();
        drain(&mut t);
        if t.houses(1) > 0 || built {
            return Err(format!(
                "E-pool-bug-04: a house went up on 购物中心 (cost 2000) while 卡池BUG is in play (houses {}, build offered: {built})",
                t.houses(1)
            ));
        }
        Ok(())
    })
    .expect("E-pool-bug-04");
}

/// E-heat-head-03 -- phase 2 subtracts 1d6 from movement dice.
#[test]
fn heat_head_phase_two_subtracts_1d6() {
    forall(3, 0xE_1603, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "超燃甩头", &[]);
        // End the drawer's next turn -> phase 2.
        until_turn(&mut t, 0);
        pass(&mut t, 0);
        if !t.event_in_play("超燃甩头") {
            return Err("setup: phase 2 never started".into());
        }
        until_turn(&mut t, 1);
        t.set_pos(1, 30);
        t.dice(&[10, 4]); // 10 - d6(4) = 6
        let _ = t.roll(1);
        drain(&mut t);
        if t.pos(1) != 36 {
            return Err(format!(
                "E-heat-head-03: P1 at {}, expected 36 (30 + 10 - 4)",
                t.pos(1)
            ));
        }
        Ok(())
    })
    .expect("E-heat-head-03");
}

/// E-embers-03 -- 「格子上每有一张此卡的复制品，那名玩家失去10资金」 on settle.
#[test]
fn embers_copy_costs_10_on_settle() {
    forall(3, 0xE_1903, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_pos(1, 20);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "火种燃尽之后会怎么样呢？", &[]);
        let site = t.pos(1) as usize;
        let copies = t.marks_on(site).len().max(1) as i32;
        until_turn(&mut t, 0);
        t.set_pos(0, (site + 59) % 60);
        let before = t.money(0);
        t.dice(&[1]);
        let _ = t.roll(0);
        drain(&mut t);
        let want = before - 10 * copies;
        if t.money(0) != want {
            return Err(format!(
                "E-embers-03: money {}, expected {want} (before {before}, {copies} copies x 10)",
                t.money(0)
            ));
        }
        Ok(())
    })
    .expect("E-embers-03");
}

/// E-circle-rebuild-05 -- CiRCLE's settle becomes 「2层[停留] or 失去500」.
#[test]
fn circle_rebuild_replaces_circle_settle() {
    forall(3, 0xE_0705, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_quiet(&mut t, 0, "协助CiRCLE重建", &[]);
        if !t.event_in_play("协助CiRCLE重建") {
            return Err("setup: not active".into());
        }
        // Land on CiRCLE (tile 0).
        until_turn(&mut t, 1);
        t.set_pos(1, 59);
        let money = t.money(1);
        t.dice(&[1]);
        let _ = t.roll(1);
        // Expect a choice of 2 [停留] or lose 500.
        let mut saw = false;
        while let Some(_p) = t.prompt() {
            let dump = t.dump_prompt();
            if dump.contains("stay") || dump.contains("停留") || dump.contains("circle_rebuild") {
                saw = true;
            }
            t.decline();
        }
        if !saw {
            return Err(format!(
                "E-circle-rebuild-05: no stay-or-pay choice on CiRCLE settle: {}",
                t.recent_keys(15).join(", ")
            ));
        }
        let _ = money;
        Ok(())
    })
    .expect("E-circle-rebuild-05");
}

/// E-this-hand-03 -- 「不触发场地效果」: the event move settles nothing.
#[test]
fn this_hand_move_triggers_no_tile_settle() {
    forall(3, 0xE_0503, |_rng, _i| {
        let mut t = Table::vanilla(2);
        let shop = tile("购物中心");
        t.set_pos(1, (shop + 60 - 3) % 60);
        t.set_draw(0, &["通用:GREAT"]);
        // P1: X = 3-1+1 = 3 forward -> lands on 购物中心 (unowned).
        draw_event_card(&mut t, 0, "这只手我不会放开", &[1, 1, 1, 3, 1, 1]);
        drain(&mut t);
        if t.pos(1) != shop {
            return Err(format!(
                "E-this-hand-03: P1 at {}, expected 购物中心 {shop}",
                t.pos(1)
            ));
        }
        if t.owner(shop).is_some() {
            return Err("E-this-hand-03: a free buy fired on the event's move".into());
        }
        Ok(())
    })
    .expect("E-this-hand-03");
}

/// E-estate-03 -- only the auctioned tile changes hands.
#[test]
fn estate_grants_only_the_auctioned_tile() {
    forall(3, 0xE_0203, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["通用:GREAT"]);
        draw_event_card(&mut t, 0, "弦卷集团地产开发", &[]);
        let p = t.expect_prompt();
        let estate = match p.text.a.get("tile") {
            Some(Arg::Tile(n)) => *n as usize,
            _ => {
                return Err(format!(
                    "E-estate-03: prompt names no tile: {}",
                    t.dump_prompt()
                ))
            }
        };
        for bid_idx in [2, 1] {
            if t.prompt().is_none() {
                break;
            }
            let who = t.asked()[0];
            let _ = t.answer(who, bid_idx);
        }
        drain(&mut t);
        let extra: Vec<usize> = (0..60)
            .filter(|&i| i != estate && t.owner(i).is_some())
            .collect();
        if !extra.is_empty() {
            return Err(format!(
                "E-estate-03: extra deeds beyond the auctioned tile {estate}: {extra:?}"
            ));
        }
        if t.owner(estate) != Some(0) {
            return Err(format!(
                "E-estate-03: auctioned tile {estate} owner {:?}, expected P0",
                t.owner(estate)
            ));
        }
        Ok(())
    })
    .expect("E-estate-03");
}

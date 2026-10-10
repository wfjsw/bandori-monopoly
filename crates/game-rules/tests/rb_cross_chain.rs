//! Cross-card chains: §1 `C*` of `docs/rulebook/CROSS-TESTS.md`.
//!
//! Black-box: effects from different cards / skills meeting on one timing.
//! Expectations come from the sheet extracts (`target/scratch/rb/*.md`) and
//! `data/rules.txt` (32, 89 and the counter rulings).
//!
//! Chain order is **ruling 2026-10-07**: the ask ring starts with the initial
//! user (the player whose action or effect raised the link; the active turn
//! player when there is no player), each visit exhausts every eligible
//! counteraction or ends on an explicit pass, laps continue until a quiet lap,
//! and a counter's own round starts with its declarer. LIFO resolution is
//! unchanged.

mod common;

use common::*;
use game_core::msg::Arg;

/// Inert filler for draw piles / hands (never auto-plays).
const FILL: &str = "R:[衍生] 觉悟";

fn give_n(t: &mut Table, who: usize, card: &str, n: usize) {
    let cards: Vec<&str> = vec![card; n];
    t.give(who, &cards);
}

fn set_draw_n(t: &mut Table, who: usize, n: usize) {
    let cards: Vec<&str> = vec![FILL; n];
    t.set_draw(who, &cards);
}

/// The card the open [反击] window's detail line names -- the timing it answers.
fn answered_card(t: &Table) -> Option<String> {
    let p = t.prompt()?;
    match p.text.a.get("detail") {
        Some(Arg::Msg(m)) => match m.a.get("card") {
            Some(Arg::Card(id)) => Some(id.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Answer a `player` prompt with this player id.
fn answer_player(t: &mut Table, who: usize, target: i32) {
    let p = t.expect_prompt();
    let k = p
        .options
        .iter()
        .position(|o| match o.a.get("who") {
            Some(Arg::PlayerId(v)) => *v == target,
            Some(Arg::I(v)) | Some(Arg::N(v)) => *v == target as i64,
            _ => false,
        })
        .unwrap_or_else(|| panic!("player {target} not offered: {}", t.dump_prompt()));
    t.answer(who, k as i32).unwrap();
}

/// Decline every open prompt.
fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// End `who`'s turn without a main move (the harness's `end` refuses at 运营
/// unless the move is done or skipped).
fn pass(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

/// Advance until it is `who`'s turn (passing every other seat).
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

/// ceil-to-10 of `n` (the rulebook's 「向上取整10」).
fn ceil10(n: i32) -> i32 {
    (n + 9) / 10 * 10
}

// =====================================================================
// C1. A single counter answers a multi-target card
// =====================================================================

// 规则书: 「[反击]…结算优先于X」 (32). 登上武道馆: X = 2000÷其他[存活]玩家数 向上取整10.
// 宣战布告: 被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡.
#[test]
fn c01_a_single_counter_answers_a_multi_target_card() {
    let mut t = Table::vanilla(3);
    set_draw_n(&mut t, 1, 3);
    t.give(1, &["AG:宣战布告"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    // ruling 2026-10-07: the ring starts at the initial user (P0, who played
    // 武道馆). P0 holds no counter, so it is skipped without a prompt and the
    // first offer is P1 -- the old rule (seat after the trigger player) gave
    // the same first offer only by accident here.
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    assert!(t.counteract_offered("AG:宣战布告"), "{}", t.dump_prompt());
    t.counteract(1, "AG:宣战布告").unwrap();
    // 宣战布告 resolves first (32), then 武道馆.
    // X = ceil10(2000/2) = 1000.
    let x = ceil10(2000 / 2);
    assert_eq!(x, 1000);
    assert_eq!(t.money(0), 10_000 - 500 + x * 2, "P0 paid 500, received {x} twice");
    assert_eq!(t.money(1), 10_000 + 500 - x, "P1 gained 500, paid {x}");
    assert_eq!(t.money(2), 10_000 - x, "P2 paid {x}");
    assert_eq!(t.hand(1).len(), 1, "宣战布告 drew 1: {:?}", t.hand(1));
}

// =====================================================================
// C2. Two counters on the same timing, from different players
// =====================================================================

// 规则书: 「多个效果可[反击]同一个时点」 (89) -- both answer X, not each other.
#[test]
fn c02_two_counters_on_the_same_timing() {
    let mut t = Table::vanilla(3);
    set_draw_n(&mut t, 1, 3);
    set_draw_n(&mut t, 2, 3);
    t.give(1, &["AG:宣战布告"]);
    t.give(2, &["AG:宣战布告"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    // ruling 2026-10-07: the ring starts at the initial user P0, who holds no
    // counter and is skipped; P1 is the first offer. Both windows answer
    // 登上武道馆 (X), not each other.
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    assert_eq!(answered_card(&t).as_deref(), Some("通用:登上武道馆"));
    t.counteract(1, "AG:宣战布告").unwrap();
    // ruling 2026-10-07: P1's visit is exhausted (its only counter is spent),
    // so priority advances to the next seat -- not the declaration itself.
    assert_eq!(t.asked(), vec![2], "the visit exhausted, priority advances: {}", t.dump_prompt());
    assert_eq!(
        answered_card(&t).as_deref(),
        Some("通用:登上武道馆"),
        "both answer X: {}",
        t.dump_prompt()
    );
    t.counteract(2, "AG:宣战布告").unwrap();
    // Both resolve before X: P0 pays 500 twice; P1 and P2 each draw 1.
    let x = ceil10(2000 / 2);
    assert_eq!(t.money(0), 10_000 - 500 * 2 + x * 2);
    assert_eq!(t.money(1), 10_000 + 500 - x);
    assert_eq!(t.money(2), 10_000 + 500 - x);
    assert_eq!(t.hand(1).len(), 1, "{:?}", t.hand(1));
    assert_eq!(t.hand(2).len(), 1, "{:?}", t.hand(2));
}

// =====================================================================
// C3. 网络链接异常 negates a counter's designation
// =====================================================================

// 规则书: 32 / 89. 网络链接异常 clause 1: 「手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]」.
// 宣战布告 is a [手] with a [指定], so its designation of P0 is cancelled.
// RULING: whether P1 still draws (one sentence covers both).
#[test]
fn c03_counter_to_counter_cancels_designation() {
    let mut t = Table::vanilla(3);
    set_draw_n(&mut t, 1, 3);
    t.give(1, &["AG:宣战布告"]);
    t.give(0, &["通用:网络链接异常"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    // Round on X. ruling 2026-10-07: the ring starts at the initial user P0,
    // whose 网络链接异常 answers a *counter's* play and is therefore not
    // eligible on X -- P0 is skipped and P1 is the first offer. P1 declares
    // 宣战布告.
    assert_eq!(t.asked(), vec![1], "{}", t.dump_prompt());
    assert_eq!(answered_card(&t).as_deref(), Some("通用:登上武道馆"));
    t.counteract(1, "AG:宣战布告").unwrap();
    // The round on X closes (nobody holds a card eligible on X);
    // 网络链接异常 is not offered against the *counter* yet (counters to
    // counters wait until the round on X has closed, 89).
    while t.prompt().is_some() {
        let asked = t.asked();
        assert_eq!(asked.len(), 1, "{}", t.dump_prompt());
        let answering = answered_card(&t);
        if answering.as_deref() == Some("AG:宣战布告") {
            break;
        }
        assert_eq!(
            answering.as_deref(),
            Some("通用:登上武道馆"),
            "still inside the round on X: {}",
            t.dump_prompt()
        );
        t.decline();
    }
    // The round on 宣战布告 is open. ruling 2026-10-07 point 4: it starts with
    // its declarer P1, who holds no card that answers a counter's play and is
    // skipped; P2 holds nothing either, so the only offer is P0's
    // 网络链接异常.
    assert!(t.prompt().is_some(), "round on 宣战布告");
    assert_eq!(
        answered_card(&t).as_deref(),
        Some("AG:宣战布告"),
        "the new timing: {}",
        t.dump_prompt()
    );
    // P0 declares 网络链接异常 when asked.
    while t.prompt().is_some() {
        let asked = t.asked();
        assert_eq!(asked.len(), 1, "{}", t.dump_prompt());
        let who = asked[0];
        if who == 0 && t.counteract_offered("通用:网络链接异常") {
            break;
        }
        t.decline();
        if who == 0 {
            break;
        }
    }
    assert!(t.counteract_offered("通用:网络链接异常"), "{}", t.dump_prompt());
    t.counteract(0, "通用:网络链接异常").unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    // P0 does not pay P1 500. 武道馆 still resolves.
    let x = ceil10(2000 / 2);
    assert_eq!(t.money(0), 10_000 + x * 2, "no 500 payment");
    assert_eq!(t.money(1), 10_000 - x);
    assert_eq!(t.money(2), 10_000 - x);
    // RULING: whether P1 still draws. Record, do not assert.
    eprintln!("c03 record: P1 hand after = {:?}", t.hand(1));
}

// =====================================================================
// C4. 网络链接异常 negates a [手] effect with no target
// =====================================================================

// 规则书: 网络链接异常 clause 2: 「手卡的[手]效果且没有[指定]目标则抵消其所有的效果」.
#[test]
fn c04_negates_an_untargeted_hand_effect() {
    let mut t = Table::vanilla(2);
    t.give(1, &["通用:网络链接异常"]);
    t.give_play(0, "通用:GREAT").unwrap();
    assert!(t.counteract_offered("通用:网络链接异常"), "{}", t.dump_prompt());
    t.counteract(1, "通用:网络链接异常").unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    // GREAT's effects are negated: no +2000, no PERFECT in the draw pile.
    assert_eq!(t.money(0), 10_000, "GREAT's gain is gone");
    assert!(
        !t.draw_pile(0).iter().any(|c| c.contains("PERFECT")),
        "no PERFECT: {:?}",
        t.draw_pile(0)
    );
}

// =====================================================================
// C5. 网络链接异常 cancels one designation of four
// =====================================================================

// 规则书: 网络链接异常 clause 1. X = ceil10(2000÷3) = 670 with 4 players.
#[test]
fn c05_cancels_one_designation_of_four() {
    let mut t = Table::vanilla(4);
    t.give(2, &["通用:网络链接异常"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    // Walk the ring; P2 declares 网络链接异常 when asked.
    loop {
        let Some(p) = t.prompt() else { break };
        let asked = t.asked();
        assert_eq!(asked.len(), 1, "{}", t.dump_prompt());
        let who = asked[0];
        if who == 2 && t.counteract_offered("通用:网络链接异常") {
            t.counteract(2, "通用:网络链接异常").unwrap();
        } else {
            t.decline();
        }
        let _ = p;
    }
    // Expected (sheet): only P2's designation is cancelled; P1 and P3 pay.
    let x = ceil10(2000 / 3);
    assert_eq!(x, 670);
    // DISCREPANCY is recorded below if the engine negates the whole card.
    assert_eq!(t.money(1), 10_000 - x, "P1 pays {x}");
    assert_eq!(t.money(3), 10_000 - x, "P3 pays {x}");
    assert_eq!(t.money(2), 10_000, "P2 pays nothing");
    assert_eq!(t.money(0), 10_000 + x * 2);
}

// =====================================================================
// C6. 安可 cancels a [停留]
// =====================================================================

// 规则书: 安可: 「[反击][使用者]即将…受到[异常移动效果]影响时：无效此次[异常移动效果]和其导致的所有效果」.
// 雨啊: X = 2d2, designate X players including the user, each gains 1 [停留].
#[test]
fn c06_encore_cancels_a_stay() {
    let mut t = Table::vanilla(3);
    t.give(1, &["通用:安可"]);
    t.give(0, &["通用:雨啊，快点来吧"]);
    // 2d2 = 1 + 1 -> X = 2.
    t.dice(&[1, 1]);
    t.play(0, "通用:雨啊，快点来吧").unwrap();
    // The prompt offers the other players; the user is one of the X = 2
    // designations (「其中必须包括[使用者]」). Pick P1 as the second.
    loop {
        let Some(p) = t.prompt() else { break };
        if p.kind == "player" {
            answer_player(&mut t, 0, 1);
        } else if t.counteract_offered("通用:安可") {
            t.counteract(1, "通用:安可").unwrap();
        } else {
            t.decline();
        }
    }
    assert_eq!(t.state(1, "stay"), 0, "P1's stay is cancelled");
    assert_eq!(t.state(0, "stay"), 1, "P0 still gets the stay");
}

// =====================================================================
// C7. Counters three deep: 安可 negated by 网络链接异常
// =====================================================================

// 规则书: 32 / 89. 安可 is a [手] with no [指定], so 网络链接异常 clause 2
// negates it entirely -- P1 gets the [停留] after all.
#[test]
fn c07_three_deep_encore_negated() {
    let mut t = Table::vanilla(3);
    t.give(1, &["通用:安可"]);
    t.give(2, &["通用:网络链接异常"]);
    t.give(0, &["通用:雨啊，快点来吧"]);
    t.dice(&[1, 1]);
    t.play(0, "通用:雨啊，快点来吧").unwrap();
    // Designate P1 (user P0 is auto-included as one of X = 2).
    loop {
        let Some(p) = t.prompt() else { break };
        if p.kind == "player" {
            answer_player(&mut t, 0, 1);
        } else {
            break;
        }
    }
    // Round on the stay: P1 declares 安可.
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("通用:安可") {
            t.counteract(1, "通用:安可").unwrap();
            break;
        }
        t.decline();
    }
    // Round on 安可: ruling 2026-10-07 point 4 starts it with its declarer P1,
    // who is out of cards and is skipped; P2's 网络链接异常 is next.
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("通用:网络链接异常") {
            t.counteract(2, "通用:网络链接异常").unwrap();
            break;
        }
        t.decline();
    }
    while t.prompt().is_some() {
        t.decline();
    }
    // 安可 was negated, so P1 gets the stay. Both cards are spent.
    assert_eq!(t.state(1, "stay"), 1, "P1 gets the stay after all");
    assert_eq!(t.state(0, "stay"), 1);
    assert!(
        t.draw_pile(1).contains(&"通用:安可".to_string()),
        "安可 spent and reshuffled: {:?}",
        t.draw_pile(1)
    );
    assert!(
        t.draw_pile(2).contains(&"通用:网络链接异常".to_string()),
        "网络链接异常 spent and reshuffled: {:?}",
        t.draw_pile(2)
    );
}

// =====================================================================
// C8. The user counters their own card's abnormal move
// =====================================================================

// 规则书: 89 / ruling 2026-10-07 -- the ring starts with the initial user
// (P0, who played 无路矢), not with the seat after them. 安可: 「因任何原因」.
// 无路矢: designate another player's tile, gain 2 [除外], teleport when it hits 0.
// (Old expectation, rewritten per ruling 2026-10-07: the ring was P1 then P0
// with P1 asked first; the ring order is now asserted per window.)
#[test]
fn c08_user_counters_own_card() {
    let mut t = Table::vanilla(2);
    t.set_pos(1, 10);
    // P1 holds a decoy so the ring is observable (both seats eligible).
    t.give(1, &["通用:网络链接异常"]);
    t.give(0, &["通用:安可"]);
    t.give_play(0, "MyGO:无路矢").unwrap();
    // Designate P1 (whose tile is 10). The prompt names a player.
    loop {
        let Some(p) = t.prompt() else { break };
        if p.kind == "player" {
            answer_player(&mut t, 0, 1);
        } else {
            break;
        }
    }
    // The counter windows: ruling 2026-10-07 -- the ring starts with the
    // initial user P0. Record every ask as (who, window), where the window is
    // the link its detail line names, so the ring order can be checked per
    // window. (The old rule asked P1 first in every window.)
    let mut order: Vec<(usize, String)> = vec![];
    loop {
        let Some(p) = t.prompt() else { break };
        let asked = t.asked();
        assert_eq!(asked.len(), 1, "{}", t.dump_prompt());
        order.push((asked[0], format!("{:?}", p.text)));
        if asked[0] == 0 && t.counteract_offered("通用:安可") {
            t.counteract(0, "通用:安可").unwrap();
        } else {
            t.decline();
        }
    }
    // ruling 2026-10-07: within every window that asks both seats, the initial
    // user P0 leads P1. A window whose link only P1's decoy answers skips P0
    // (no eligible card) and may ask P1 alone.
    let mut by_win: Vec<(String, Vec<usize>)> = vec![];
    for (who, win) in &order {
        match by_win.iter_mut().find(|(w, _)| w == win) {
            Some((_, asks)) => asks.push(*who),
            None => by_win.push((win.clone(), vec![*who])),
        }
    }
    for (win, asks) in &by_win {
        let p0 = asks.iter().position(|&w| w == 0);
        let p1 = asks.iter().position(|&w| w == 1);
        if let (Some(i0), Some(i1)) = (p0, p1) {
            assert!(
                i0 < i1,
                "the initial user P0 leads P1 in one window (ruling 2026-10-07): {win} -> {asks:?}"
            );
        }
    }
    assert!(
        order.iter().any(|(w, _)| *w == 0),
        "P0 is in the ring: {order:?}"
    );
    assert!(
        order.iter().any(|(w, _)| *w == 1),
        "P1 is in the ring: {order:?}"
    );
    eprintln!("c08 record: order = {order:?}, exile = {}", t.state(0, "exile"));
    // 安可 answers the [除外] application (an abnormal move), not the play.
    assert_eq!(t.state(0, "exile"), 0, "P0 gets no exile");
    assert_eq!(t.pos(0), 0, "P0 is not teleported");
}

// =====================================================================
// C9. 离心力 on the second targeting between turns
// =====================================================================

// 规则书: 离心力: 「当你在两个你的回合之间…第二次成为…目标时…直到下个你的回合开始时，无效化你受到的所有效果」.
#[test]
fn c09_centrifugal_on_the_second_targeting() {
    let mut t = Table::vanilla(3);
    t.give(1, &["Mor:离心力，不为所动"]);
    t.give(0, &["通用:登上武道馆", "通用:登上武道馆"]);
    let x = ceil10(2000 / 2);
    // First 武道馆: 离心力 must not be offered.
    t.play(0, "通用:登上武道馆").unwrap();
    assert!(
        !t.counteract_offered("Mor:离心力，不为所动"),
        "first targeting: {}",
        t.dump_prompt()
    );
    while t.prompt().is_some() {
        t.decline();
    }
    assert_eq!(t.money(1), 10_000 - x);
    assert_eq!(t.money(2), 10_000 - x);
    // Second 武道馆: 离心力 is offered.
    t.play(0, "通用:登上武道馆").unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("Mor:离心力，不为所动") {
            t.counteract(1, "Mor:离心力，不为所动").unwrap();
            break;
        }
        t.decline();
    }
    while t.prompt().is_some() {
        t.decline();
    }
    // P1 pays 1000 once in total; P2 pays twice.
    assert_eq!(t.money(1), 10_000 - x, "P1 pays once");
    assert_eq!(t.money(2), 10_000 - x * 2, "P2 pays twice");
    assert_eq!(t.money(0), 10_000 + x * 3 - 500 * 0, "P0 receives from P2 twice + P1 once");
}

// =====================================================================
// C10. 夏日合宿 vs multi-target and single-target cards
// =====================================================================

// 规则书: 夏日合宿: 「直到下个自己的回合开始前，你只会被自己发动的效果指定」.
// RULING: whether X still counts P1 as an alive other.
#[test]
fn c10_summer_camp_vs_targeting() {
    // P1 plays 夏日合宿 on its turn; on P0's turn P0 tries to target P1.
    let mut t = Table::vanilla(3);
    t.begin_turn(1);
    t.give_play(1, "Mor:夏日合宿").unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    pass(&mut t, 1);
    until_turn(&mut t, 0);
    let x = ceil10(2000 / 2);
    t.give_play(0, "通用:登上武道馆").unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    // Only P2 pays X (P1 is not designated).
    assert_eq!(t.money(2), 10_000 - x, "P2 pays {x}");
    assert_eq!(t.money(1), 10_000, "P1 is not designated");
    eprintln!(
        "c10 record: X={x}, P0={}, P1={} (RULING: does X count P1?)",
        t.money(0),
        t.money(1)
    );
    // 找回珍妮弗 at P1: refused / no effect.
    t.give(0, &["PP:找回珍妮弗"]);
    if t.play(0, "PP:找回珍妮弗").is_ok() {
        loop {
            let Some(_) = t.prompt() else { break };
            let p = t.expect_prompt();
            if p.kind == "player" {
                answer_player(&mut t, 0, 1);
            } else {
                t.decline();
            }
        }
    }
    assert!(
        !t.field_ids(1).iter().any(|c| c.contains("珍妮弗")),
        "P1 must not host the card: {:?}",
        t.field_ids(1)
    );
}

// =====================================================================
// C11. EXIST redirects a single-target card
// =====================================================================

// 规则书: EXIST: 「场上及打出的所有对单一玩家生效的手卡…的目标将改为你…若在此期间此卡没有造成影响，抽1张卡」.
#[test]
fn c11_exist_redirects_single_target() {
    let mut t = Table::vanilla(3);
    set_draw_n(&mut t, 2, 3);
    t.place_raw(2, "RAS:EXIST");
    t.give_play(0, "PP:找回珍妮弗").unwrap();
    // Target P1; the card must land on P2's field.
    loop {
        let Some(_) = t.prompt() else { break };
        let p = t.expect_prompt();
        if p.kind == "player" {
            answer_player(&mut t, 0, 1);
        } else {
            t.decline();
        }
    }
    assert!(
        t.field_ids(2).iter().any(|c| c.contains("珍妮弗")),
        "lands on P2: {:?}",
        t.field_ids(2)
    );
    assert!(
        !t.field_ids(1).iter().any(|c| c.contains("珍妮弗")),
        "not P1: {:?}",
        t.field_ids(1)
    );
    // EXIST had an effect, so at P2's next turn start it goes to discard with
    // no draw. Drive P2's turn.
    until_turn(&mut t, 2);
    assert!(
        t.discard(2).contains(&"RAS:EXIST".to_string()),
        "EXIST to discard: {:?}",
        t.discard(2)
    );
    assert_eq!(t.hand(2).len(), 0, "EXIST draws nothing when it had an effect");
}

// =====================================================================
// C12. EXIST vs 夏日合宿
// =====================================================================

// 规则书: EXIST redirects to its owner; 夏日合宿 blocks designation of P1.
// RULING: whether 夏日合宿 counts as having "negated" something (its draw).
#[ignore = "DISCREPANCY: with 夏日合宿 on P1 and EXIST on P2, 找回珍妮弗 does not land on P2"]
#[test]
fn c12_exist_vs_summer_camp() {
    let mut t = Table::vanilla(3);
    t.begin_turn(1);
    t.give_play(1, "Mor:夏日合宿").unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    pass(&mut t, 1);
    until_turn(&mut t, 0);
    t.place_raw(2, "RAS:EXIST");
    t.give_play(0, "PP:找回珍妮弗").unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        let p = t.expect_prompt();
        if p.kind == "player" {
            answer_player(&mut t, 0, 1);
        } else {
            t.decline();
        }
    }
    assert!(
        t.field_ids(2).iter().any(|c| c.contains("珍妮弗")),
        "lands on P2: {:?}",
        t.field_ids(2)
    );
    eprintln!(
        "c12 record: P1 field = {:?} (RULING: does 夏日合宿 count as negated?)",
        t.field_ids(1)
    );
}

// =====================================================================
// C13. R.I.O.T. and 宣战布告 on the same timing
// =====================================================================

// 规则书: counters to one timing resolve newest first (ruling on 89).
// R.I.O.T.: 所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡.
#[ignore = "DISCREPANCY: with R.I.O.T. + 宣战布告 on one timing, P0 ends at 12000 (the 500 payment is missing) instead of 11500"]
#[test]
fn c13_riot_and_war_declaration() {
    let mut t = Table::vanilla(3);
    set_draw_n(&mut t, 0, 5);
    set_draw_n(&mut t, 1, 5);
    set_draw_n(&mut t, 2, 5);
    // Hands: P0 = 武道馆 + 2 fillers; P1 = R.I.O.T. + 1; P2 = 宣战布告 + 3.
    t.give(0, &["通用:登上武道馆", FILL, FILL]);
    t.give(1, &["RAS:R. I. O. T.", FILL]);
    t.give(2, &["AG:宣战布告", FILL, FILL, FILL]);
    t.play(0, "通用:登上武道馆").unwrap();
    // Round on X: P1 declares R.I.O.T., P2 declares 宣战布告.
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("RAS:R. I. O. T.") {
            t.counteract(1, "RAS:R. I. O. T.").unwrap();
        } else if t.counteract_offered("AG:宣战布告") {
            t.counteract(2, "AG:宣战布告").unwrap();
        } else {
            t.decline();
        }
    }
    // Newest first: 宣战布告 (P2) resolves first, then R.I.O.T. (P1), then X.
    // After R.I.O.T.: P0 hand 2, P1 hand 1+1, P2 hand 3+1 (redrawn same count).
    assert_eq!(t.hand(0).len(), 2, "P0: {:?}", t.hand(0));
    assert_eq!(t.hand(1).len(), 2, "P1 = 1 + 1: {:?}", t.hand(1));
    assert_eq!(t.hand(2).len(), 4, "P2 = 3 + 1: {:?}", t.hand(2));
    // Money: P0 paid P2 500; X = 1000 from P1 and P2.
    let x = ceil10(2000 / 2);
    assert_eq!(t.money(0), 10_000 - 500 + x * 2);
    assert_eq!(t.money(1), 10_000 - x);
    assert_eq!(t.money(2), 10_000 + 500 - x);
}

// =====================================================================
// C14. 骰子已经掷下 shuts the counter windows for the turn
// =====================================================================

// 规则书: 骰子已经掷下: 「本回合内所有其他玩家无法从手牌中使用[反击]，回合结束后放入弃牌堆」.
#[ignore = "DISCREPANCY: 骰子已经掷下 is placed twice and does not shut the counter windows"]
#[test]
fn c14_dice_cast_shuts_windows() {
    let mut t = Table::vanilla(2);
    t.give(1, &["AG:宣战布告"]);
    t.give(0, &["Mujica:骰子已经掷下", "通用:登上武道馆"]);
    t.play(0, "Mujica:骰子已经掷下").unwrap();
    // P1 may counter it; let it resolve.
    while t.prompt().is_some() {
        t.decline();
    }
    t.play(0, "通用:登上武道馆").unwrap();
    assert!(
        t.prompt().is_none(),
        "no counter window for 武道馆: {}",
        t.dump_prompt()
    );
    pass(&mut t, 0);
    assert!(
        t.discard(0).contains(&"Mujica:骰子已经掷下".to_string()),
        "card to discard at turn end: {:?}",
        t.discard(0)
    );
}

// =====================================================================
// C15. 骰子已经掷下 can itself be countered
// =====================================================================

// 规则书: 骰子已经掷下: 「此卡可以被反击」. 网络链接异常 clause 2 negates it.
#[test]
fn c15_dice_cast_itself_countered() {
    let mut t = Table::vanilla(2);
    t.give(1, &["AG:宣战布告", "通用:网络链接异常"]);
    t.give(0, &["Mujica:骰子已经掷下", "通用:登上武道馆"]);
    t.play(0, "Mujica:骰子已经掷下").unwrap();
    // P1 counters with 网络链接异常 (no target -> negate all).
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("通用:网络链接异常") {
            t.counteract(1, "通用:网络链接异常").unwrap();
            break;
        }
        t.decline();
    }
    while t.prompt().is_some() {
        t.decline();
    }
    // The dice card never took effect: 武道馆 offers P1 the 宣战布告 window.
    t.play(0, "通用:登上武道馆").unwrap();
    assert!(
        t.counteract_offered("AG:宣战布告"),
        "the window comes back: {}",
        t.dump_prompt()
    );
    while t.prompt().is_some() {
        t.decline();
    }
}

// =====================================================================
// C16. 无法将视线移开 answers a counter
// =====================================================================

// 规则书: 无法将视线移开: 「当有玩家对你使用[反击]后立即使用…强制移动1~4…并[触发结算]」.
#[ignore = "DISCREPANCY: 无法将视线移开 does not force the counter-user to move"]
#[test]
fn c16_cannot_look_away_answers_a_counter() {
    let mut t = Table::vanilla(2);
    t.give(1, &["AG:宣战布告"]);
    t.give(0, &["通用:登上武道馆", "Mujica:无法将视线移开"]);
    t.play(0, "通用:登上武道馆").unwrap();
    // P1 counters with 宣战布告.
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("AG:宣战布告") {
            t.counteract(1, "AG:宣战布告").unwrap();
            break;
        }
        t.decline();
    }
    // Round on 宣战布告: P0 plays 无法将视线移开, forward 2.
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("Mujica:无法将视线移开") {
            t.counteract(0, "Mujica:无法将视线移开").unwrap();
            break;
        }
        t.decline();
    }
    // Choose forward 2 (the move prompt).
    loop {
        let Some(_) = t.prompt() else { break };
        let p = t.expect_prompt();
        // options / items describing 1..4
        let k = t.option("2").or_else(|| {
            p.items
                .iter()
                .position(|s| s == "2")
                .map(|x| x as i32)
        });
        if let Some(k) = k {
            t.answer(0, k).unwrap();
        } else {
            eprintln!("c16 prompt: {}", t.dump_prompt());
            t.decline();
        }
    }
    // P1 moved 2 and settled; then 宣战布告; then 武道馆.
    assert_eq!(t.pos(1), 2, "P1 moved forward 2");
    let x = ceil10(2000 / 2);
    assert_eq!(t.money(0), 10_000 - 500 + x);
    assert_eq!(t.money(1), 10_000 + 500 - x);
}

// =====================================================================
// C17. 再次牵起手来 cancels a card-forced payment
// =====================================================================

// 规则书: 再次牵起手来 [持续]: 「[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区」.
#[test]
fn c17_holding_hands_again_cancels_payment() {
    let mut t = Table::vanilla(3);
    t.place_raw(1, "Mor:再次牵起手来");
    t.give(1, &["AG:宣战布告"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("AG:宣战布告") {
            t.counteract(1, "AG:宣战布告").unwrap();
            break;
        }
        t.decline();
    }
    while t.prompt().is_some() {
        t.decline();
    }
    let x = ceil10(2000 / 2);
    // P0 pays P1 500. P1's 1000 to P0 is cancelled; the card goes to discard.
    assert_eq!(t.money(0), 10_000 - 500 + x, "P1's payment cancelled");
    assert_eq!(t.money(1), 10_000 + 500, "P1 does not pay {x}");
    assert_eq!(t.money(2), 10_000 - x);
    assert!(
        t.discard(1).contains(&"Mor:再次牵起手来".to_string()),
        "the continuous card is spent: {:?}",
        t.discard(1)
    );
}

// =====================================================================
// C18. 三全音 vs a card-forced payment
// =====================================================================

// 规则书: 三全音: 「任意时刻当你将要失去或支付资金时…立刻获得此次失去的资金金额…三回合后…弃置并支付由此卡获得的资金」.
// The sheet says 「立刻获得此次失去的资金金额」 with no 「改为」: the payment still
// happens, P1 nets 0 now, and the payee receives (CROSS-TESTS.md C18).
// RULING: who receives the payback (bank or original payee).
#[test]
fn c18_tritone_vs_card_payment() {
    let mut t = Table::vanilla(2);
    t.give(1, &["Mor:迷茫之蝶们的三全音"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    let x = ceil10(2000 / 1);
    assert_eq!(x, 2000);
    // P1 counters the payment with 三全音.
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("Mor:迷茫之蝶们的三全音") {
            t.counteract(1, "Mor:迷茫之蝶们的三全音").unwrap();
            break;
        }
        t.decline();
    }
    while t.prompt().is_some() {
        t.decline();
    }
    // P1 gains x then pays x: net 0. The payment still happens, so the payee
    // (P0) receives x. The card sits on the field with 3 crystals.
    assert_eq!(t.money(1), 10_000, "net 0 now: {}", t.money(1));
    assert_eq!(t.money(0), 10_000 + x, "the payee receives X");
    assert!(
        t.field_ids(1).iter().any(|c| c.contains("三全音")),
        "on the field: {:?}",
        t.field_ids(1)
    );
    assert_eq!(t.crystals(1, "Mor:迷茫之蝶们的三全音"), Some(3));
    // After 3 of P1's turn ends it is discarded and P1 pays x.
    for _ in 0..3 {
        until_turn(&mut t, 1);
        pass(&mut t, 1);
    }
    assert!(
        t.draw_pile(1).contains(&"Mor:迷茫之蝶们的三全音".to_string()),
        "discarded and reshuffled after 3 turn ends: {:?}",
        t.draw_pile(1)
    );
    eprintln!(
        "c18 record: P1={}, P0={} (RULING: who receives the payback?)",
        t.money(1),
        t.money(0)
    );
}

// =====================================================================
// C19. （小白） vs a card-forced payment
// =====================================================================

// 规则书: （小白）: 「此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金」.
#[test]
fn c19_shiroko_vs_card_payment() {
    let mut t = Table::new(&["青叶摩卡", "仓田真白"]);
    t.clean();
    t.begin_turn(0);
    t.give(1, &["Mor:（小白）"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    let x = ceil10(2000 / 1);
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("Mor:（小白）") {
            t.counteract(1, "Mor:（小白）").unwrap();
            break;
        }
        t.decline();
    }
    while t.prompt().is_some() {
        t.decline();
    }
    // P1 loses x instead of paying it; P0 loses x/2. P0 receives nothing.
    assert_eq!(t.money(1), 10_000 - x, "P1 loses {x}");
    assert_eq!(t.money(0), 10_000 - x / 2, "P0 loses {}", x / 2);
}

// =====================================================================
// C20. 主唱太拼命了 + （摩卡）0.5倍速
// =====================================================================

// 规则书: 主唱太拼命了: 「一次性向其他玩家支付5000以上资金时，免除此次支付」.
// 0.5倍速: 「你的所有资金支付与消耗减半」.
// RULING: whether 「5000以上」 checks pre- or post-modifier amount.
#[test]
fn c20_vocal_too_hard_plus_half_speed() {
    let mut t = Table::new(&["青叶摩卡", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.place_raw(0, "AG:（摩卡）0.5倍速");
    t.give(0, &["CRYCHIC:主唱太拼命了"]);
    // P1 owns tile t whose rent R is in 5000..9999. Tile 29 弦卷豪宅 rent[3] = 6920.
    let t29 = tile("弦卷豪宅");
    let rent = data().tiles[t29].rent[3];
    assert!((5000..10_000).contains(&rent), "R = {rent}");
    t.set_owner(t29, Some(1));
    t.set_houses(t29, 3);
    t.set_pos(0, t29 - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    // The window is offered only when R/2 >= 5000 -- record what happens.
    let offered = t.counteract_offered("CRYCHIC:主唱太拼命了") || t.prompt().is_some();
    eprintln!(
        "c20 record: R={rent}, R/2={}, window offered = {offered}, prompt = {}",
        rent / 2,
        t.dump_prompt()
    );
    while t.prompt().is_some() {
        if t.counteract_offered("CRYCHIC:主唱太拼命了") {
            t.counteract(0, "CRYCHIC:主唱太拼命了").unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!("c20 record after: P0={}, P1={}", t.money(0), t.money(1));
}

// =====================================================================
// C21. Here the world catches a second card in one turn
// =====================================================================

// 规则书: Here the world: 「当有人同一回合内打出两张卡时，将此卡放置于对方场上…
// 下次抽卡时，将那张卡背面朝上放置于此卡上并为其放置3个奇迹水晶…每个回合开始时移除一个，
// 当奇迹水晶数为0时…加入手牌，并使此卡使用者抽一张卡」.
#[ignore = "DISCREPANCY: Here the world does not land on the second card played in one turn"]
#[test]
fn c21_here_the_world() {
    let mut t = Table::vanilla(2);
    set_draw_n(&mut t, 0, 4);
    t.give(0, &["R:[衍生] 压", "通用:10次招募（1回限定）"]);
    t.give(1, &["Sumimi:Here the world"]);
    // P0 plays two cards in one turn.
    t.play(0, "R:[衍生] 压").unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    t.play(0, "通用:10次招募（1回限定）").unwrap();
    // P1 counters the second play with Here the world.
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("Sumimi:Here the world") {
            t.counteract(1, "Sumimi:Here the world").unwrap();
            break;
        }
        t.decline();
    }
    while t.prompt().is_some() {
        t.decline();
    }
    assert!(
        t.field_ids(0).iter().any(|c| c.contains("Here the world")),
        "sits on P0's field: {:?}",
        t.field_ids(0)
    );
    // P0's next draw (10次招募 draws 1) goes face-down onto it with 3 crystals.
    let hw = "Sumimi:Here the world";
    assert_eq!(t.crystals(0, hw), Some(3), "3 crystals");
    eprintln!(
        "c21 record: P0 hand = {:?}, draw = {:?}, crystals = {:?}",
        t.hand(0),
        t.draw_pile(0),
        t.crystals(0, hw)
    );
    // At each of P0's turn starts a crystal drops; at 0 the card joins P0's
    // hand and P1 draws 1.
    for _ in 0..3 {
        until_turn(&mut t, 0);
        let c = t.crystals(0, hw);
        eprintln!("c21 record: crystals at P0 turn start = {c:?}");
        pass(&mut t, 0);
    }
    until_turn(&mut t, 0);
    eprintln!(
        "c21 record after: hand = {:?}, P1 hand = {:?}",
        t.hand(0),
        t.hand(1)
    );
}

// =====================================================================
// C22/C23. （真奈）歌唱大赛5连冠
// =====================================================================

// 规则书: （真奈）歌唱大赛5连冠: 「场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡…
// 若没有人…打出[反击]卡，你抽一张卡」.
#[test]
fn c22_manas_counter_nobody_joins() {
    let mut t = Table::new(&["花园多惠", "纯田真奈", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    set_draw_n(&mut t, 1, 3);
    t.give(1, &["Sumimi:（真奈）歌唱大赛5连冠"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("Sumimi:（真奈）歌唱大赛5连冠") {
            t.counteract(1, "Sumimi:（真奈）歌唱大赛5连冠").unwrap();
            break;
        }
        t.decline();
    }
    while t.prompt().is_some() {
        t.decline();
    }
    assert_eq!(t.hand(1).len(), 1, "P1 draws 1: {:?}", t.hand(1));
    let x = ceil10(2000 / 2);
    assert_eq!(t.money(1), 10_000 - x);
    assert_eq!(t.money(2), 10_000 - x);
}

// 规则书: 同上. RULING: redirection of self-effects (「针对打出玩家自身的效果改为你」).
#[test]
fn c23_manas_counter_with_a_joiner() {
    let mut t = Table::new(&["花园多惠", "纯田真奈", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    set_draw_n(&mut t, 1, 3);
    set_draw_n(&mut t, 2, 3);
    t.give(1, &["Sumimi:（真奈）歌唱大赛5连冠"]);
    t.give(2, &["AG:宣战布告"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    // P1 declares 真奈; P2 is offered 宣战布告 in 真奈's window.
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("Sumimi:（真奈）歌唱大赛5连冠") {
            t.counteract(1, "Sumimi:（真奈）歌唱大赛5连冠").unwrap();
            break;
        }
        t.decline();
    }
    // Now the joiner window.
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("AG:宣战布告") {
            t.counteract(2, "AG:宣战布告").unwrap();
            break;
        }
        t.decline();
    }
    while t.prompt().is_some() {
        t.decline();
    }
    eprintln!(
        "c23 record (RULING: self-effect redirection): P0={} P1={} P2={} hands {:?}/{:?}/{:?}",
        t.money(0),
        t.money(1),
        t.money(2),
        t.hand(0),
        t.hand(1),
        t.hand(2)
    );
}

// =====================================================================
// C24. 花园多惠 (2) cancels a [手] effect with 4 fire
// =====================================================================

// 规则书: 花园多惠 (2): 「在其他玩家使用…手牌的[手]效果时你可以使用4个[火罐]将其抵消，
// 然后你获得500资金且被抵消的玩家获得2000资金」.
#[ignore = "DISCREPANCY: 花园多惠 (2) never opens a cancel window for a hand effect; the card resolves normally"]
#[test]
fn c24_hanae_cancels_a_hand_effect() {
    let mut t = Table::new(&["户山香澄", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.set_fire(1, 4, 4);
    t.give_play(0, "通用:登上武道馆").unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        // 花园多惠's (2) is a skill counteraction.
        let k = t.option("花园多惠").or_else(|| t.option("cancel")).or_else(|| t.option("抵消"));
        if let Some(k) = k {
            t.answer(1, k).unwrap();
        } else if t.counteract_offered("通用:登上武道馆") {
            t.decline();
        } else {
            eprintln!("c24 prompt: {}", t.dump_prompt());
            t.decline();
        }
    }
    // Nobody pays X. P1 +500, P0 +2000, P1's fire = 0.
    assert_eq!(t.money(1), 10_000 + 500, "P1 +500");
    assert_eq!(t.money(0), 10_000 + 2000, "P0 +2000");
    assert_eq!(t.fire(1), 0, "fire spent");
    eprintln!("c24 record: prompt trail done, fire P1 = {}", t.fire(1));
}

// =====================================================================
// C25. 花园多惠 (2) vs a skill's [主] effect
// =====================================================================

// 规则书: 花园多惠 (2) vs 户山香澄 (2) teleport.
// RULING: whether P0's fire is refunded.
#[ignore = "DISCREPANCY: 花园多惠 (2) never opens a cancel window for a skill use; the teleport resolves"]
#[test]
fn c25_hanae_vs_skill_main() {
    let mut t = Table::new(&["户山香澄", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.set_fire(0, 1, 1);
    t.set_fire(1, 4, 4);
    t.own(0, &[10]);
    t.set_pos(0, 5);
    // P0 uses (2): teleport to its own tile.
    let sid = t.skill_id(0, "非凡之星");
    t.skill(0, &sid).unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        let k = t.option("花园多惠").or_else(|| t.option("cancel")).or_else(|| t.option("抵消"));
        if let Some(k) = k {
            t.answer(1, k).unwrap();
        } else {
            eprintln!("c25 prompt: {}", t.dump_prompt());
            // try to pick tile 10 for the teleport if asked
            let p = t.expect_prompt();
            if let Some(k) = p.items.iter().position(|s| s == "10") {
                t.answer(0, k as i32).unwrap();
            } else {
                t.decline();
            }
        }
    }
    eprintln!(
        "c25 record (RULING: P0 fire refund): pos0={} (want 5, not moved), P0={} P1={} fire0={} fire1={}",
        t.pos(0),
        t.money(0),
        t.money(1),
        t.fire(0),
        t.fire(1)
    );
    assert_eq!(t.pos(0), 5, "P0 does not move");
    assert_eq!(t.money(0), 10_000 + 2000, "P0 +2000");
    assert_eq!(t.money(1), 10_000 + 500, "P1 +500");
    assert_eq!(t.fire(1), 0);
}

// =====================================================================
// C26. Card immunity vs a crystal mover
// =====================================================================

// 规则书: 不要背负期待 [特]: 「此卡不受任何其他效果影响」.
// 会被骗着买水晶的人: move one crystal between two cards.
#[ignore = "DISCREPANCY: 会被骗着买水晶的人 offers 不要背负期待 (「此卡不受任何其他效果影响」) as a crystal endpoint"]
#[test]
fn c26_immunity_vs_crystal_mover() {
    let mut t = Table::vanilla(2);
    t.place_raw(1, "PP:不要背负期待");
    t.place_raw(1, "AG:绯红之魂");
    t.set_crystals(1, "AG:绯红之魂", 3);
    t.give_play(0, "Mujica:会被骗着买水晶的人").unwrap();
    // The source / destination pick must not offer 不要背负期待.
    loop {
        let Some(_) = t.prompt() else { break };
        let p = t.expect_prompt();
        let dump = t.dump_prompt();
        assert!(
            !dump.contains("不要背负期待"),
            "immune card must not be offered: {dump}"
        );
        // pick something legal
        if !p.items.is_empty() {
            t.answer(0, 0).unwrap();
        } else {
            t.decline();
        }
    }
}

// =====================================================================
// C27. 欢迎来到ave mujica的世界 (2) on a state switch
// =====================================================================

// 规则书: 欢迎来到ave mujica的世界 (2): 「当有其他玩家切换状态时，你与所有本回合切换了状态的玩家同时切换一次状态」.
// RULING: whether a player with no state 2 is unaffected.
#[ignore = "DISCREPANCY: 欢迎来到ave mujica的世界 (1) cannot be played (err.play_phase)"]
#[test]
fn c27_welcome_state_switch() {
    let mut t = Table::new(&["三角初华", "若叶睦", "丰川祥子"]);
    t.clean();
    t.begin_turn(0);
    t.give(2, &["Mujica:欢迎来到ave mujica的世界"]);
    // P0 plays (1) to switch P1's state. Record what happens.
    t.give_play(0, "Mujica:欢迎来到ave mujica的世界").unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        eprintln!("c27 prompt: {}", t.dump_prompt());
        let k = t.option("2");
        if let Some(k) = k {
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!(
        "c27 record (RULING: no-state-2 players): P0 state = {:?}, P1 = {:?}, P2 = {:?}",
        t.p(0).state,
        t.p(1).state,
        t.p(2).state
    );
}

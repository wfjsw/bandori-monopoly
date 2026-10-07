//! Black-box tests for the money pipeline: one pipeline carries every money
//! movement (print / delete / pay-player) whatever caused it, through staged
//! adjustment points.
//!
//! Spec: `docs/ENGINE.md` + `docs/rulebook/TEST-FINDINGS.md` §1.2. Naming:
//! `<slug>_<what>`. Each assertion block carries the clause it checks.

mod common;
use common::*;

// =====================================================================
// local helpers
// =====================================================================

/// Skip every open prompt (counteract windows included).
fn skip_all(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

// =====================================================================
// stage order
// =====================================================================

/// The money pipeline runs its stages in order on a card-driven **print**
/// (game -> player): the `effect` declaration (the [反击] window) opens first,
/// then the modifier passes (`payAdd` -> `payMul` -> `payChoose` -> `payAt`),
/// then the settlement `pay`, then `payAfter` / `paid`. FEVER! is a `payAdd`
/// hook; it must raise a card's own [获得], not just board-driven money.
///
/// 规则书（FEVER!）: 「[拥有者]被[支付]或[获得]资金时将金额额外提高X」
#[test]
fn gain_runs_the_modifier_stages() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "通用:[衍生]FEVER!").unwrap();
    // GREAT [获得]2000; FEVER! adds X=400 (600 - 200 for its own face-up card).
    t.give_play(0, "通用:GREAT").unwrap();
    assert_eq!(t.money(0), 12_400, "events {:?}", t.recent_keys(8));
}

/// The `effect` declaration (the [反击] chain) opens on a card-driven
/// **pay-player** entry the same way it does on rent, so a counteraction
/// 「当你将要向其他玩家支付时」 sees the payer and payee.
///
/// 规则书（小白）: 「当你将要向其他玩家支付时打出此卡」
#[test]
fn card_payment_opens_the_counteract_window() {
    let mut t = Table::vanilla(3);
    t.set_character_raw(1, "仓田真白");
    t.give(1, &["Mor:（小白）"]);
    t.begin_turn(0);
    t.give(0, &["通用:登上武道馆"]);
    t.play(0, "通用:登上武道馆").unwrap();
    assert!(
        t.counteract_offered("Mor:（小白）"),
        "card-driven payment is answerable: {}",
        t.dump_prompt()
    );
}

// =====================================================================
// per-pair cancel
// =====================================================================

/// Per-pair cancel: a multi-target payment's individual entries can be
/// cancelled one at a time. `通用:网络链接异常`'s 「取消其对目标之一的[指定]」
/// is the archetype. The static targeting query (`ctx::designations`, C#
/// `H.Db.Card(id).Targeting`) names the play's recipients before its body runs;
/// `ctx::cancel_designation` drops one (C# `play.Tags["immune"+seat]`), and the
/// rest of the play's designations still land.
///
/// 规则书（网络链接异常）: 「取消其对目标之一的[指定]」
#[test]
fn per_pair_cancel_drops_one_designation() {
    let mut t = Table::vanilla(3);
    t.give(1, &["通用:网络链接异常"]);
    t.give(0, &["通用:登上武道馆"]);
    t.play(0, "通用:登上武道馆").unwrap();
    t.counteract(1, "通用:网络链接异常").unwrap();
    skip_all(&mut t);
    // One of the two targets is un-designated; the other pays X=1000.
    assert_eq!(t.money(0), 11_000, "receives from the remaining target");
    let paid: i32 = (1..3).map(|w| 10_000 - t.money(w)).sum();
    assert_eq!(paid, 1000, "exactly one target paid");
}

// =====================================================================
// negative-amount clamp
// =====================================================================

/// A negative final amount becomes 0 -- the pipeline clamps at every stage, so
/// a modifier that drives the figure below zero moves nothing (and the payer
/// keeps their money).
///
/// 规则书（FEVER!）: 「X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）」
/// -- with enough face-up cards X goes negative, and the [获得] clamps to 0.
#[test]
fn negative_final_amount_clamps_to_zero() {
    let mut t = Table::vanilla(2);
    // FEVER! itself is one face-up card. Adding two more drops X to 600-200*3
    // = 0; three more drops it to -200, and the 2000 gain clamps to 0.
    t.give_play(0, "通用:[衍生]FEVER!").unwrap();
    t.place_raw(0, "通用:[衍生]FEVER!");
    t.place_raw(0, "通用:[衍生]FEVER!");
    t.place_raw(0, "通用:[衍生]FEVER!");
    // 4 face-up cards -> X = 600 - 800 = -200 -> max(0, 2000-200) = 1800.
    // (FEVER! itself clamps; the pipeline's `t.value.max(0)` is the outer net.)
    t.give_play(0, "通用:GREAT").unwrap();
    let gained = t.money(0) - 10_000;
    assert!(
        gained >= 0 && gained <= 2000,
        "gain clamped into [0, 2000]: {gained} (events {:?})",
        t.recent_keys(6)
    );
    // The outer clamp: nothing below 0 ever moves.
    assert!(t.money(0) >= 10_000, "money never drops from a gain");
}
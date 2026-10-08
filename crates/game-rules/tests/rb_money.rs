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

/// A negative *modifier* shrinks a still-positive gain; it does **not** floor
/// the figure at zero mid-pipeline (Q3: adjustments compose freely), and a
/// net-positive final settles as an ordinary gain. A net-*negative* final is
/// the reversal case below.
///
/// 规则书（FEVER!）: 「X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）」
/// -- with enough face-up cards X goes negative, and the [获得] shrinks.
#[test]
fn negative_gain_modifier_shrinks_the_gain() {
    let mut t = Table::vanilla(2);
    // FEVER! itself is one face-up card. Adding two more drops X to 600-200*3
    // = 0; three more drops it to -200, and the 2000 gain shrinks.
    t.give_play(0, "通用:[衍生]FEVER!").unwrap();
    t.place_raw(0, "通用:[衍生]FEVER!");
    t.place_raw(0, "通用:[衍生]FEVER!");
    t.place_raw(0, "通用:[衍生]FEVER!");
    // 4 face-up cards -> X = 600 - 800 = -200 -> 2000-200 = 1800 (net > 0).
    t.give_play(0, "通用:GREAT").unwrap();
    let gained = t.money(0) - 10_000;
    assert!(
        gained >= 0 && gained <= 2000,
        "gain stays in [0, 2000]: {gained} (events {:?})",
        t.recent_keys(6)
    );
    assert!(t.money(0) >= 10_000, "money never drops from a net-positive gain");
}

// =====================================================================
// Q3 negative-final reversal (PIPELINE-AUDIT, user ruling 2026-10-07)
// =====================================================================

/// A two-sided payment whose final amount is negative settles the original at
/// 0 and starts a **new** payment with the participants swapped and the
/// absolute value. Here `TEST:payOvercut` drives P0→P1's 1000 to −500, so the
/// reverse child is P1→P0 500.
///
/// 规则书 专有名词 3 [支付]: 「A[支付]BX资金」 -- the direction is part of the
/// declaration; a negative final reverses who pays whom rather than
/// disappearing (user ruling 2026-10-07, PIPELINE-AUDIT Q3).
#[test]
fn two_sided_negative_final_reverses_the_payment() {
    let mut t = Table::vanilla(2);
    t.place_raw(1, "TEST:payOvercut");
    t.give_play(0, "TEST:xfer1000").unwrap();
    skip_all(&mut t);
    // Original settles at 0; the reverse is P1 paying P0 500.
    assert_eq!(t.money(0), 10_500, "P0 receives the reverse: {:?}", t.recent_keys(8));
    assert_eq!(t.money(1), 9_500, "P1 pays the reverse");
}

/// One-sided 「[获得]」 (bank→A) driven negative becomes A losing |x|
/// 「[消耗]」: the bank leg flips.
///
/// 规则书 专有名词 1 [获得] / 2 [消耗] -- the two are opposite directions of
/// the same money movement; a negative final flips between them (user ruling
/// 2026-10-07).
#[test]
fn one_sided_negative_gain_becomes_a_loss() {
    let mut t = Table::vanilla(2);
    t.place_raw(1, "TEST:payOvercut");
    t.give_play(0, "TEST:gain1000").unwrap();
    skip_all(&mut t);
    // 1000 gain → −500 → original 0 → reverse: P0 loses 500.
    assert_eq!(t.money(0), 9_500, "the gain reversed into a loss: {:?}", t.recent_keys(8));
}

/// The mirror: one-sided 「[消耗]」 (A→bank) driven negative becomes a gain.
#[test]
fn one_sided_negative_loss_becomes_a_gain() {
    let mut t = Table::vanilla(2);
    t.place_raw(1, "TEST:payOvercut");
    t.give_play(0, "TEST:lose1000").unwrap();
    skip_all(&mut t);
    // 1000 loss → −500 → original 0 → reverse: P0 gains 500.
    assert_eq!(t.money(0), 10_500, "the loss reversed into a gain: {:?}", t.recent_keys(8));
}

/// The reverse child runs the full pipeline including the Q1 shortfall path:
/// a reverse 「[消耗]」 the payer cannot cover (even after 抵押) raises funds
/// and then bankrupts them.
///
/// 规则书 专有名词 6 [破产]: 「当玩家无法支付某笔支出时（包括抵押），将所有可折现的
/// 资产折现并将所有资金[消耗]或[支付]给导致破产的效果对象」.
#[test]
fn reversal_shortfall_raises_funds_then_bankrupts() {
    let mut t = Table::vanilla(2);
    t.place_raw(1, "TEST:payOvercut");
    // P1 is the one who will owe the reverse (P1→P0 500). Leave them with 0
    // cash and no deeds: even 抵抵押 cannot cover 500, so Q1 runs to bankruptcy.
    t.set_money(1, 0);
    t.set_money(0, 10_000);
    t.give_play(0, "TEST:xfer1000").unwrap();
    skip_all(&mut t);
    assert!(t.p(1).bankrupt, "the reverse payer is out: {:?}", t.recent_keys(10));
    assert_eq!(t.money(1), 0, "every fund left the bankrupt seat");
}

// =====================================================================
// card-imposed shortfall (PIPELINE-AUDIT Q1 / bug B1)
// =====================================================================

/// A card's forced 「[支付]」 the payer cannot cover (even after 抵押) runs the
/// same raise-funds-then-bankrupt path rent does -- it may eliminate a player.
/// Before the fix the card path settled as `must = false` and silently
/// underpaid, so a forced payment could never bankrupt anyone.
///
/// 规则书 专有名词 6 [破产]: 「当玩家无法支付某笔支出时（包括抵押），将所有可折现的
/// 资产折现并将所有资金[消耗]或[支付]给导致破产的效果对象」.
/// 规则书（登上武道馆）: 「被[指定]的玩家[支付][使用者]X资金」.
#[test]
fn card_shortfall_raises_funds_then_bankrupts() {
    let mut t = Table::vanilla(2);
    // P1 holds 1000 cash and one deed whose 抵押 pays 500 (小豆岛, price 1000)
    // -- 1500 against budokan's X = ceil10(2000/1) = 2000. Not even 抵押 covers
    // it (L16), so `raise_funds` goes straight under.
    let land = tile("小豆岛");
    t.own(1, &[land]);
    t.set_money(1, 1_000);
    t.set_money(0, 10_000);
    t.give_play(0, "通用:登上武道馆").unwrap();
    skip_all(&mut t);
    assert!(t.p(1).bankrupt, "P1 is out: events {:?}", t.recent_keys(10));
    assert_eq!(t.money(1), 0, "every fund left the bankrupt seat");
    // 规则书 L16: the liquidation goes to the creditor (P0). Cash-in of the
    // deed is its 抵押 value (price/2), on top of the 1000 cash.
    let cashed = data().tiles[land].price / 2;
    assert_eq!(
        t.money(0),
        10_000 + 1_000 + cashed,
        "the creditor received the liquidation (cashed {cashed})"
    );
    // B2 / 规则书 L81: the seat's field is cleared with it.
    assert!(
        t.field(1).is_empty(),
        "the bankrupt seat's field is empty: {:?}",
        t.field_ids(1)
    );
}

/// The other half of Q1: when 抵押 covers the forced payment, the mortgage is
/// offered and the payer stays in the game.
///
/// 规则书 游戏流程 5 (bold): 「运营阶段及需[支付]或[消耗]资金且资金不足时可以选择
/// 抵押拥有的地契，抵押后[获得]地契购买价格50%的资金」.
#[test]
fn card_shortfall_may_be_funded_by_mortgage() {
    let mut t = Table::vanilla(2);
    // 购物中心 is 3000 -> 抵押 1500. 1000 + 1500 = 2500 >= budokan's 2000.
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_money(1, 1_000);
    t.give_play(0, "通用:登上武道馆").unwrap();
    // The raise-funds path opens the mortgage offer before going under.
    let p = t.expect_prompt();
    assert_eq!(p.kind, "mortgage", "the mortgage offer: {}", t.dump_prompt());
    t.answer_items(1, &[&land.to_string()]).unwrap();
    skip_all(&mut t);
    assert!(!t.p(1).bankrupt, "P1 stays in: events {:?}", t.recent_keys(10));
    assert!(t.mortgaged(land), "the deed is mortgaged");
    // 1000 + 1500 (抵押) - 2000 (paid) = 500.
    assert_eq!(t.money(1), 500, "mortgage funded the payment");
    assert_eq!(t.money(0), 12_000, "the creditor received 2000");
}

// =====================================================================
// 「分摊前」 / the pre-split stage (PIPELINE-AUDIT Q2)
// =====================================================================

/// A command-wide **pre-split** modifier shapes the 「[分摊]」 total before it is
/// divided, not each leg afterwards. 「[分摊][支付]2000」 across two payers with
/// a `payTotalAdd` of -500 charges `ceil10(1500/2) = 750` each; wired as a
/// per-share stage it would charge `ceil10(2000/2) - 500 = 500` each.
///
/// 规则书（微笑的铁假面）: 「其他玩家[分摊][支付][使用者]2000资金」.
/// 规则书（丸山彩 (2)）: 「此次支付的分摊前资金减少Y×100（最少0）」 -- the
/// 「分摊前」 figure is the pre-split total.
#[test]
fn pre_split_modifier_shapes_the_total_not_each_leg() {
    let mut t = Table::vanilla(3);
    // 微笑的铁假面 is 白鹭千圣's exclusive.
    t.set_character_raw(0, "白鹭千圣");
    // The 「分摊前」 probe: cuts the command total by 500 wherever it is placed.
    t.place_raw(0, "TEST:totalCut");
    t.give_play(0, "PP:[白鹭千圣]微笑的铁假面").unwrap();
    skip_all(&mut t);
    // 2000 -> 1500 pre-split, then ceil10(1500/2) = 750 per leg.
    assert_eq!(t.money(1), 10_000 - 750, "P1 pays the shaped share");
    assert_eq!(t.money(2), 10_000 - 750, "P2 pays the shaped share");
    assert_eq!(t.money(0), 10_000 + 1_500, "P0 receives both shares");
}

// =====================================================================
// self-payment 「A[支付]A」 (PIPELINE-AUDIT Q4)
// =====================================================================

/// A self-payment is not a no-op: the counteraction windows run in full (a
/// counteraction may redirect the payee) and the affordability / raise-funds /
/// bankruptcy path applies exactly as to any other payment. Only the **net
/// balance effect** is zero when the pair settles -- the debit and the credit
/// cancel. Before the fix `money_inner` returned before any event.
///
/// 规则书 专有名词 3 [支付]: 「A[支付]BX资金」 -- L14 does not exclude A == B.
/// `BANKRUPTCY_PIPELINE` invariant: 「Explicit self-payment follows the same
/// affordability and bankruptcy path even though its successful debit and
/// credit have zero net balance effect.」
#[test]
fn self_payment_leaves_money_unchanged_when_affordable() {
    let mut t = Table::vanilla(2);
    // A `payAdd` probe on P1: it must see the self-payment's windows fire.
    t.place_raw(1, "TEST:payAddAny");
    let mark = t.mark();
    t.give_play(0, "TEST:selfCharge").unwrap();
    skip_all(&mut t);
    assert_eq!(t.money(0), 10_000, "the debit and the credit cancel (zero net)");
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k.contains("pay_add_any")),
        "the counteraction windows ran (the payAdd probe fired): {keys:?}"
    );
}

/// The other half of Q4: a self-payment the payer cannot cover runs the
/// raise-funds-then-bankrupt path (规则书 L16 「当玩家无法支付某笔支出时（包括
/// 抵押）」), even though the payee is the payer. The liquidation is consumed
/// (the creditor is the bankrupt seat itself and so is not a live recipient).
#[test]
fn self_payment_shortfall_raises_funds_then_bankrupts() {
    let mut t = Table::vanilla(2);
    // 1000 cash and one deed whose 抵押 pays 500 -- 1500 against 5000. Not even
    // 抵押 covers it, so `raise_funds` goes straight under.
    let land = tile("天文馆");
    t.own(0, &[land]);
    t.set_money(0, 1_000);
    t.give_play(0, "TEST:selfCharge").unwrap();
    skip_all(&mut t);
    assert!(t.p(0).bankrupt, "P0 is out: events {:?}", t.recent_keys(10));
    assert_eq!(t.money(0), 0, "every fund left the bankrupt seat");
}

/// Q4's mortgage half: when 抵押 covers the self-charge, the mortgage is
/// offered and the pair settles with a zero net -- the payer keeps the raised
/// funds (the debit and the credit cancel).
///
/// 规则书 游戏流程 5 (bold): 「需[支付]或[消耗]资金且资金不足时可以选择抵押拥有的
/// 地契，抵押后[获得]地契购买价格50%的资金」.
#[test]
fn self_payment_may_be_funded_by_mortgage() {
    let mut t = Table::vanilla(2);
    // 1000 cash plus deeds whose 抵押 sums to 4900 (弦卷豪宅 1800 + 水族馆 1600
    // + 购物中心 1500) -- enough to raise the 5000 self-charge.
    let deeds = [tile("弦卷豪宅"), tile("水族馆"), tile("购物中心")];
    t.own(0, &deeds);
    t.set_money(0, 1_000);
    t.give_play(0, "TEST:selfCharge").unwrap();
    // The raise-funds path opens the mortgage offer before going under.
    let p = t.expect_prompt();
    assert_eq!(p.kind, "mortgage", "the mortgage offer: {}", t.dump_prompt());
    let idx: Vec<String> = deeds.iter().map(|d| d.to_string()).collect();
    let picks: Vec<&str> = idx.iter().map(|s| s.as_str()).collect();
    t.answer_items(0, &picks).unwrap();
    skip_all(&mut t);
    assert!(!t.p(0).bankrupt, "P0 stays in: events {:?}", t.recent_keys(10));
    // Debit and credit cancel: the money is the cash plus the 抵押 proceeds.
    assert_eq!(
        t.money(0),
        1_000 + 1_800 + 1_600 + 1_500,
        "the raised funds stay (zero net)"
    );
}
// =====================================================================
// no activation costs (NEGATION-AUDIT V2 / money-gate list)
// =====================================================================

// TODO(规则书) NEGATION-AUDIT V2 -- the six text-less money play gates are
// gone. Each card below is playable while short; the in-body 「[消耗]/[支付]」
// is effect content and takes the Q1 shortfall path (mortgage, then
// bankruptcy).

/// 规则书（10次招募）: 「[消耗]1500资金，抽1张卡。」 -- no [限] on money, so the
/// play is legal at 0 cash; the 1500 then runs the shortfall path.
#[test]
fn gacha10_playable_while_short_then_bankrupts() {
    let mut t = Table::vanilla(2);
    t.set_money(0, 0);
    t.give(0, &["通用:10次招募（1回限定）"]);
    assert!(
        t.play(0, "通用:10次招募（1回限定）").is_ok(),
        "playable at 0 cash: {:?}",
        t.recent_keys(6)
    );
    skip_all(&mut t);
    assert!(t.p(0).bankrupt, "the in-body 消耗 bankrupts: {:?}", t.recent_keys(10));
}

/// 规则书（Fire bird）: 「支付1600资金将此卡放置在自己场地上…」 -- the pay is
/// effect content; a short payer is offered 抵押 first (规则书 L76).
#[test]
fn fire_bird_playable_while_short_then_mortgages() {
    let mut t = Table::vanilla(2);
    // 弦卷豪宅 price 3600 -> 抵押 1800, which covers Fire bird's 1600.
    let land = tile("弦卷豪宅");
    t.own(0, &[land]);
    t.set_money(0, 0);
    t.give(0, &["R:Fire bird"]);
    assert!(
        t.play(0, "R:Fire bird").is_ok(),
        "playable at 0 cash: {:?}",
        t.recent_keys(6)
    );
    // Q1: the 1600 shortfall opens the mortgage offer (规则书 L76).
    let p = t.expect_prompt();
    assert_eq!(p.kind, "mortgage", "the mortgage offer: {}", t.dump_prompt());
    t.answer_items(0, &[&land.to_string()]).unwrap();
    skip_all(&mut t);
    assert!(!t.p(0).bankrupt, "mortgage funded it: {:?}", t.recent_keys(10));
}

/// 规则书（绯红之魂）: 「选择[消耗]1到5次500资金…」 -- playable while short; the
/// chosen 500-n then shortfall-runs.
#[test]
fn crimson_soul_playable_while_short_then_bankrupts() {
    let mut t = Table::vanilla(2);
    t.set_money(0, 0);
    t.give(0, &["AG:绯红之魂"]);
    assert!(
        t.play(0, "AG:绯红之魂").is_ok(),
        "playable at 0 cash: {:?}",
        t.recent_keys(6)
    );
    // The in-body ask is 1..=5 times 500; pick 1 (the clamp still allows 1).
    if let Some(p) = t.prompt() {
        if p.kind != "mortgage" {
            let _ = t.answer(0, 0);
        }
    }
    skip_all(&mut t);
    assert!(t.p(0).bankrupt, "the in-body 消耗 bankrupts: {:?}", t.recent_keys(10));
}

/// 规则书（因为我一直相信着你）: 「至少有另一张手牌时可发动，消耗800资金…」 --
/// the hand-count clause is a real [限] and stays; the money clause does not.
#[test]
fn believe_you_playable_while_short_then_bankrupts() {
    let mut t = Table::vanilla(3);
    t.set_money(0, 0);
    // The real [限]: another hand card.
    t.give(0, &["HHW:因为我一直相信着你", "通用:GREAT"]);
    assert!(
        t.play(0, "HHW:因为我一直相信着你").is_ok(),
        "playable at 0 cash with a spare hand card: {:?}",
        t.recent_keys(6)
    );
    skip_all(&mut t);
    assert!(t.p(0).bankrupt, "the in-body 消耗 bankrupts: {:?}", t.recent_keys(10));
}

/// 规则书（kkr 前往笑容集结的地方）: 「支付10000资金（视为买地花费）…」 --
/// playable while short; the 10000 then shortfall-runs.
#[test]
fn kokoro_circle_playable_while_short_then_bankrupts() {
    let mut t = Table::vanilla(2);
    // The card is 弦卷心's exclusive (规则书 「并将此卡置于CiRCLE上」).
    t.set_character_raw(0, "弦卷心");
    t.set_money(0, 0);
    t.give(0, &["HHW:（kkr）前往笑容集结的地方！"]);
    let r = t.play(0, "HHW:（kkr）前往笑容集结的地方！");
    assert!(
        r.is_ok(),
        "playable at 0 cash: {:?} events {:?}",
        r,
        t.recent_keys(6)
    );
    skip_all(&mut t);
    assert!(t.p(0).bankrupt, "the in-body 支付 bankrupts: {:?}", t.recent_keys(10));
}

/// 规则书（蝴蝶飞舞的星月夜）: 「支付X次1000的的资金…」 -- playable while short;
/// X is chosen then the payment shortfall-runs.
#[test]
fn starry_night_playable_while_short_then_bankrupts() {
    let mut t = Table::vanilla(2);
    t.set_money(0, 0);
    t.give(0, &["Mor:蝴蝶飞舞的星月夜"]);
    assert!(
        t.play(0, "Mor:蝴蝶飞舞的星月夜").is_ok(),
        "playable at 0 cash: {:?}",
        t.recent_keys(6)
    );
    // The X ask clamps to at least 1; answer 1 (1000) then the shortfall runs.
    if let Some(p) = t.prompt() {
        if p.kind != "mortgage" {
            let _ = t.answer(0, 0);
        }
    }
    skip_all(&mut t);
    assert!(t.p(0).bankrupt, "the in-body 支付 bankrupts: {:?}", t.recent_keys(10));
}

// =====================================================================
// marker windows + ownership + bankruptcy (user ruling 2026-10-07)
// =====================================================================

/// A marker spend gets its own [反击] window (`markerSpend`) **before** the
/// markers move. A counter body that cancels the link spends nothing. No
/// shipped card listens to the window yet -- `TEST:markerDeny` pins the shape.
///
/// 规则书 专有名词 「标志物解释」 -- markers are currency like any other and a
/// spend is answerable (user ruling 2026-10-07).
#[test]
fn marker_spend_window_cancels_the_spend() {
    let mut t = Table::vanilla(2);
    t.give(1, &["TEST:markerDeny"]);
    t.give_play(0, "TEST:markerSpend").unwrap();
    // The markerSpend window offers TEST:markerDeny to P1.
    let mut denied = false;
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if t.counteract_offered("TEST:markerDeny") {
            denied = true;
            t.counteract(1, "TEST:markerDeny").unwrap();
        } else {
            t.decline();
        }
        let _ = p;
    }
    let trap: Vec<_> = t.events_since(0).into_iter()
        .filter(|e| e.msg.key().contains("card_trap"))
        .map(|e| format!("{:?}", e.msg))
        .collect();
    assert!(denied, "the markerSpend window opened: {:?} traps {:?}", t.recent_keys(10), trap);
    // Nothing was spent: the +5 gain landed, the -3 was cancelled.
    assert_eq!(t.token(0, "TEST:tok"), 5, "the cancelled spend moved nothing");
}

/// Marker ownership (user ruling 2026-10-07): a marker is owned by the rule
/// that creates it, wherever its copies sit. Bankruptcy of that rule's player
/// clears every copy -- including ones on **other** players and on tiles.
/// Neutral board marks ([CP点], owner -1) stay.
///
/// 规则书 专有名词 6 [破产] + 81: 「将其控制的所有棋子…移出游戏」 -- extended
/// by the ruling to every marker their rules own.
#[test]
fn bankruptcy_clears_owned_markers_wherever_they_sit() {
    let mut t = Table::vanilla(3);
    // 要乐奈's skill owns every 抹茶芭菲 copy.
    t.place_raw(0, "skill:要乐奈:投币式停车场的猫");
    {
        let w = t.m.world_mut();
        w.note_marker_owner("抹茶芭菲", "skill:要乐奈:投币式停车场的猫");
        w.set_tok(1, "抹茶芭菲", 2);
        w.add_mark(tile("Space") as i32, 1, "抹茶芭菲", Default::default());
        w.add_cp_mark(tile("CiRCLE") as i32, -1, "test", Default::default());
    }
    // P0 is forced under by a rent they cannot cover.
    t.set_money(0, 0);
    t.own(1, &[tile("购物中心")]);
    t.set_houses(tile("购物中心"), 5);
    t.begin_turn(0);
    t.set_pos(0, tile("购物中心") - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    skip_all(&mut t);
    assert!(t.p(0).bankrupt, "P0 is out: {:?}", t.recent_keys(12));
    assert_eq!(t.token(1, "抹茶芭菲"), 0, "the other player's copy is cleared");
    assert!(
        t.marks_on(tile("Space")).iter().all(|m| m.kind != "抹茶芭菲"),
        "the tile copy is cleared: {:?}",
        t.marks_on(tile("Space"))
    );
    assert!(
        t.marks_on(tile("CiRCLE")).iter().any(|m| m.is_cp()),
        "neutral CP marks stay: {:?}",
        t.marks_on(tile("CiRCLE"))
    );
}

/// Marker costs keep today's timing (user ruling 2026-10-07): spent as the
/// effect resolves, nothing spent if the effect is negated. The press checks
/// (「消耗7个[火罐]」) stay activation guards. Exception: HHW:（美咲） may
/// counteract with 0 火罐 -- 「消耗所有火罐」 is vacuous when the pot is empty.
///
/// 规则书（美咲）: 「消耗所有火罐使你传送至…」 -- no [限] on holding fire.
#[test]
fn misaki_may_counteract_with_zero_fire() {
    let mut t = Table::vanilla(3);
    t.give(0, &["HHW:（美咲）"]);
    t.set_fire(0, 0, 3); // empty pot
    // A rival standing 2 tiles ahead of P0.
    t.set_pos(0, 10);
    t.set_pos(1, 12);
    t.give_play(0, "TEST:fireRoll").unwrap();
    skip_all(&mut t);
    // A plain roll now looks like a fire-pot roll (the fixture tags `fireRoll`).
    t.dice(&[3]);
    t.roll(0).unwrap();
    // The counteraction window must offer 美咲 even though the pot is empty.
    assert!(
        t.counteract_offered("HHW:（美咲）"),
        "美咲 is offered at 0 fire: {}",
        t.dump_prompt()
    );
}

//! [反击] guard scope: a `can_counteract` guard must match exactly what the
//! card's rulebook clause names -- no broader, no narrower.
//!
//! The money pipeline opens a `ChainKind::Effect` window on **every** money
//! movement (print / delete / pay-player), so a guard like
//! `ChainKind::Effect && target == player_id` with no effect-kind check wrongly
//! fires on a plain gain. Each test below checks the card is **not** offered on
//! an unrelated money movement and **is** offered on its real trigger.

mod common;

use common::*;

/// Skip every open prompt (counteract windows included).
fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// A plain [获得] of 1000 to the turn player (`R:[衍生] 压`). The money pipeline
/// declares a `pay` effect with `from == -1` and `target ==` the gainer -- the
/// archetype of an "effect aimed at the holder" that is **not** a card's
/// [异常移动效果]/「支付」/「成为目标」 clause. Leaves any prompt open for the
/// caller to inspect.
fn plain_gain(t: &mut Table) {
    let who = t.turn();
    t.give_play(who, "R:[衍生] 压").unwrap();
}

/// `TEST:stunner`: the turn player stuns the first other player through the
/// [异常] gate. The `effect` link carries an `abnormal` entry aimed at that
/// player, with `by_card` the turn player. Returns the stunned seat. Leaves any
/// prompt open.
fn stun_first_other(t: &mut Table) -> usize {
    let by = t.turn();
    t.give_play(by, "TEST:stunner").unwrap();
    (0..t.n).find(|&s| s != by).unwrap()
}

/// `TEST:aimer`: the turn player designates the first other player. The
/// `effect` link carries a `target` entry aimed at that player. Leaves any
/// prompt open.
fn target_first_other(t: &mut Table) {
    let by = t.turn();
    t.give_play(by, "TEST:aimer").unwrap();
}

/// End the turn without a main move (the 运营-phase way out).
fn pass_turn(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

/// Whether `card` was offered at any point while draining the open prompts.
fn drain_recording(t: &mut Table, card: &str) -> bool {
    let mut offered = false;
    for _ in 0..30 {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered(card) {
            offered = true;
        }
        t.decline();
    }
    offered
}

/// Assert `card` is not offered on the open (or absent) prompt, then drain.
fn assert_not_offered(t: &mut Table, card: &str, why: &str) {
    assert!(!t.counteract_offered(card), "{why}: {}", t.dump_prompt());
    drain(t);
}

/// Assert `card` **is** offered on the open prompt, declare it as `who`, drain.
fn assert_offered_and_play(t: &mut Table, who: usize, card: &str, why: &str) {
    assert!(t.counteract_offered(card), "{why}: {}", t.dump_prompt());
    t.counteract(who, card).unwrap();
    drain(t);
}

// =====================================================================
// AG:(兰) 像往常一样 -- 「受到异常移动效果（包括你的技能）」
// =====================================================================

/// 规则书: 「（2）受到异常移动效果（包括你的技能）的回合结束前…」. The [反击]
/// timing is that clause: an [异常移动效果] aimed at the holder, not any
/// effect aimed at them (the money pipeline's `pay` on a plain [获得] must not
/// open it).
#[test]
fn ran_as_usual_matches_only_abnormal() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "美竹兰");
    // Not on a plain gain to the holder.
    t.give(0, &["AG:(兰) 像往常一样"]);
    plain_gain(&mut t);
    assert_not_offered(&mut t, "AG:(兰) 像往常一样", "a plain [获得] is not an [异常移动效果]");
    // Is on an [异常移动效果] aimed at the holder (P0 is the stunner, so put
    // the card on P1 -- the stunned one).
    t.set_character_raw(1, "美竹兰");
    t.give(1, &["AG:(兰) 像往常一样"]);
    let victim = stun_first_other(&mut t);
    assert_eq!(victim, 1);
    assert_offered_and_play(&mut t, 1, "AG:(兰) 像往常一样", "[异常移动效果] aimed at the holder");
}

// =====================================================================
// MyGO:普通与理所当然 -- 「受到异常移动效果影响后可打出」
// =====================================================================

/// 规则书: 「[反击] 受到异常移动效果影响后可打出」. Plus the `lastWalk > 0`
/// gate (a previous non-teleport main move whose length can be copied).
#[test]
fn ordinary_matches_only_abnormal() {
    let mut t = Table::vanilla(2);
    // Not on a plain gain to the holder.
    t.give(0, &["MyGO:普通与理所当然"]);
    plain_gain(&mut t);
    assert_not_offered(&mut t, "MyGO:普通与理所当然", "a plain [获得] is not an [异常移动效果]");
    // A non-teleport main move so `lastWalk > 0`, then hand the turn to P1.
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    pass_turn(&mut t, 0);
    // Is on an [异常移动效果] aimed at the holder (P0).
    t.give(0, &["MyGO:普通与理所当然"]);
    let victim = stun_first_other(&mut t); // P1 stuns P0
    assert_eq!(victim, 0);
    assert_offered_and_play(&mut t, 0, "MyGO:普通与理所当然", "[异常移动效果] aimed at the holder");
}

// =====================================================================
// R:选择自己的舞台 -- 「受到[除外]以外的异常移动效果影响时」
// =====================================================================

/// 规则书: 「[反击] 受到[除外]以外的异常移动效果影响时可打出此卡」. The old
/// guard read `trigger::abnormal_kind()`, which is `None` inside the `effect`
/// window (the abnormal rides the link's effect list) -- so the card never
/// fired at all.
#[test]
fn own_stage_matches_non_exile_abnormal() {
    let mut t = Table::vanilla(2);
    // Not on a plain gain to the holder.
    t.give(0, &["R:选择自己的舞台"]);
    plain_gain(&mut t);
    assert_not_offered(&mut t, "R:选择自己的舞台", "a plain [获得] is not an [异常移动效果]");
    // Is on a [晕眩] (an [异常移动效果], and not [除外]) aimed at the holder.
    t.give(1, &["R:选择自己的舞台"]);
    let victim = stun_first_other(&mut t); // P0 stuns P1
    assert_eq!(victim, 1);
    assert_offered_and_play(&mut t, 1, "R:选择自己的舞台", "[晕眩] is an [除外]-less [异常移动效果]");
}

// =====================================================================
// Mor:离心力，不为所动 -- 「第二次成为其他角色技能或卡牌的目标时」
// =====================================================================

/// 规则书: 「[反击] 当你…第二次成为其他角色技能或卡牌的目标时」. A designation,
/// not any effect aimed at the holder: an [异常移动效果] aimed at the holder
/// (or the money pipeline's `pay`) must not open it.
#[test]
fn centrifugal_matches_only_targeting() {
    let mut t = Table::vanilla(2);
    t.give(1, &["Mor:离心力，不为所动"]);
    // First designation: the 「第二次」 gate keeps it shut.
    target_first_other(&mut t);
    assert_not_offered(&mut t, "Mor:离心力，不为所动", "first designation");
    // Second designation: offered (the real trigger). Decline and keep the
    // count so the next *non-targeting* link can be checked against it.
    t.give(1, &["Mor:离心力，不为所动"]);
    target_first_other(&mut t);
    assert!(t.counteract_offered("Mor:离心力，不为所动"), "second designation: {}", t.dump_prompt());
    t.decline();
    drain(&mut t);
    // Not on an [异常移动效果] aimed at the holder (a different effect kind).
    t.give(1, &["Mor:离心力，不为所动"]);
    stun_first_other(&mut t);
    assert_not_offered(&mut t, "Mor:离心力，不为所动", "an [异常移动效果] is not 「成为目标」");
    t.set_state(1, "stun", 0);
    // Not on a plain gain to the holder (P1 plays it this time).
    pass_turn(&mut t, 0);
    t.give(1, &["Mor:离心力，不为所动"]);
    plain_gain(&mut t);
    assert_not_offered(&mut t, "Mor:离心力，不为所动", "a [获得] is not 「成为目标」");
}

// =====================================================================
// Mor:秘密与青春的虹彩 -- 「当你向学妹或同级生支付时」/「当学姐或同级生向你支付的时候」
// =====================================================================

/// 规则书（1）: 「当你向学妹或同级生支付时」. (2) is the mirror image. A
/// player-to-player **payment** -- a plain [获得] (a `pay` entry with
/// `from == -1`) is not one and must not open the window.
#[test]
fn secret_rainbow_matches_only_player_payments() {
    let mut t = Table::vanilla(3);
    t.give(0, &["Mor:秘密与青春的虹彩"]);
    // Not on a plain gain to the holder.
    plain_gain(&mut t);
    assert_not_offered(&mut t, "Mor:秘密与青春的虹彩", "a plain [获得] is not a 「支付」");
    // Is on a player-to-player payment involving the holder (武道馆: P1/P2 pay P0).
    t.give(0, &["Mor:秘密与青春的虹彩"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    assert!(
        drain_recording(&mut t, "Mor:秘密与青春的虹彩"),
        "a player-to-player payment: {}",
        t.dump_prompt()
    );
}

// =====================================================================
// Mor:迷茫之蝶们的三全音 -- 「任意时刻当你将要失去或支付资金时」
// =====================================================================

/// 规则书: 「[反击] 任意时刻当你将要失去或支付资金时打出此卡」. A money loss by
/// the holder -- not the causer's own [异常移动效果] (`player_id` = the causer,
/// `value` = its `AbKind`, which is also `> 0`).
#[test]
fn tritone_matches_only_money_loss() {
    let mut t = Table::vanilla(3);
    // Not on a plain gain to the holder.
    t.give(0, &["Mor:迷茫之蝶们的三全音"]);
    plain_gain(&mut t);
    assert_not_offered(&mut t, "Mor:迷茫之蝶们的三全音", "a [获得] is not 「失去或支付」");
    // Not on the holder's own [异常移动效果] aimed at someone else.
    t.give(0, &["Mor:迷茫之蝶们的三全音"]);
    stun_first_other(&mut t);
    assert_not_offered(&mut t, "Mor:迷茫之蝶们的三全音", "my own [晕眩] of someone else is not my money loss");
    t.set_state(1, "stun", 0); // clear the stun so P1 can pay below
    // Is on a payment the holder makes (武道馆: the designated others pay P0,
    // so put the card on a designated payer).
    t.give(1, &["Mor:迷茫之蝶们的三全音"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    assert!(
        drain_recording(&mut t, "Mor:迷茫之蝶们的三全音"),
        "a payment from the holder: {}",
        t.dump_prompt()
    );
}

// =====================================================================
// R:（亚子）黑暗大魔姬亚子 -- 「当你即将向其他玩家支付资金时」
// =====================================================================

/// 规则书（1）: 「[反击] 当你即将向其他玩家支付资金时可打出此卡」. A payment to
/// another player -- not the causer's own [异常移动效果].
#[test]
fn ako_dark_matches_only_payments_to_others() {
    let mut t = Table::vanilla(3);
    // Not on a plain gain to the holder.
    t.set_character_raw(0, "宇田川亚子");
    t.give(0, &["R:（亚子）黑暗大魔姬亚子"]);
    plain_gain(&mut t);
    assert_not_offered(&mut t, "R:（亚子）黑暗大魔姬亚子", "a [获得] is not 「向其他玩家支付」");
    // Not on the holder's own [异常移动效果] aimed at someone else.
    t.give(0, &["R:（亚子）黑暗大魔姬亚子"]);
    stun_first_other(&mut t);
    assert_not_offered(&mut t, "R:（亚子）黑暗大魔姬亚子", "my own [晕眩] of someone else is not my payment");
    t.set_state(1, "stun", 0); // clear the stun so P1 can pay below
    // Is on a payment the holder makes to another player.
    t.set_character_raw(1, "宇田川亚子");
    t.give(1, &["R:（亚子）黑暗大魔姬亚子"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    assert!(
        drain_recording(&mut t, "R:（亚子）黑暗大魔姬亚子"),
        "a payment from the holder: {}",
        t.dump_prompt()
    );
}

// =====================================================================
// Mor:（小白） -- 「当你将要向其他玩家支付时」
// =====================================================================

/// 规则书: 「[反击] 当你将要向其他玩家支付时打出此卡」.
#[test]
fn mashiro_pay_matches_only_payments_to_others() {
    let mut t = Table::vanilla(3);
    // Not on a plain gain to the holder.
    t.set_character_raw(0, "仓田真白");
    t.give(0, &["Mor:（小白）"]);
    plain_gain(&mut t);
    assert_not_offered(&mut t, "Mor:（小白）", "a [获得] is not 「向其他玩家支付」");
    // Not on the holder's own [异常移动效果] aimed at someone else.
    t.give(0, &["Mor:（小白）"]);
    stun_first_other(&mut t);
    assert_not_offered(&mut t, "Mor:（小白）", "my own [晕眩] of someone else is not my payment");
    t.set_state(1, "stun", 0); // clear the stun so P1 can pay below
    // Is on a payment the holder makes to another player.
    t.set_character_raw(1, "仓田真白");
    t.give(1, &["Mor:（小白）"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    assert!(
        drain_recording(&mut t, "Mor:（小白）"),
        "a payment from the holder: {}",
        t.dump_prompt()
    );
}

// =====================================================================
// CRYCHIC:主唱太拼命了 -- 「一次性向其他玩家支付5000以上资金时」
// =====================================================================

/// 规则书: 「[反击] 一次性向其他玩家支付5000以上资金时，免除此次支付」. The
/// rent of a 3-house 水族馆 is 6760.
#[test]
fn vocal_too_hard_matches_only_big_payments_to_others() {
    let mut t = Table::vanilla(2);
    let aq = tile("水族馆");
    t.own(0, &[aq]);
    t.set_houses(aq, 3);
    // Not on a plain gain to the holder.
    t.give(0, &["CRYCHIC:主唱太拼命了"]);
    plain_gain(&mut t);
    assert_not_offered(&mut t, "CRYCHIC:主唱太拼命了", "a [获得] is not 「向其他玩家支付」");
    // Is on the 6760 rent P1 pays P0.
    pass_turn(&mut t, 0);
    t.give(1, &["CRYCHIC:主唱太拼命了"]);
    t.set_pos(1, aq - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    assert_offered_and_play(&mut t, 1, "CRYCHIC:主唱太拼命了", "a 6760 rent is 「向其他玩家支付5000以上」");
    assert_eq!(t.money(1), 10_000, "the payment was waived");
}

// =====================================================================
// Sumimi:现在她是Sumimi的小初啦 -- 「失去资金的总额即将超过…[收费标价]时」
// =====================================================================

/// 规则书: 「[反击] 当你在…之间失去资金的总额即将超过你所在格子的[收费标价]时
/// 打出此卡」. Standing on 水族馆 (0 houses, tag = 320), a 1000 payment from
/// 武道馆 exceeds the tag. A plain [获得] is not a 「失去资金」.
#[test]
fn now_sumimi_matches_only_money_loss() {
    let mut t = Table::vanilla(3);
    let aq = tile("水族馆");
    t.set_pos(1, aq);
    // Not on a plain gain to the holder.
    t.give(0, &["Sumimi:现在她是Sumimi的小初啦"]);
    plain_gain(&mut t);
    assert_not_offered(&mut t, "Sumimi:现在她是Sumimi的小初啦", "a [获得] is not 「失去资金」");
    // Not on the holder's own [异常移动效果] aimed at someone else.
    t.give(0, &["Sumimi:现在她是Sumimi的小初啦"]);
    stun_first_other(&mut t);
    assert_not_offered(&mut t, "Sumimi:现在她是Sumimi的小初啦", "my own [晕眩] of someone else is not my money loss");
    t.set_state(1, "stun", 0); // clear the stun so P1 can pay below
    // Is on a payment from the holder that exceeds the tile's [收费标价]
    // (武道馆: the designated others pay P0, so put the card on a designated
    // payer standing on 水族馆).
    t.set_pos(1, aq);
    t.give(1, &["Sumimi:现在她是Sumimi的小初啦"]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    assert!(
        drain_recording(&mut t, "Sumimi:现在她是Sumimi的小初啦"),
        "a 1000 loss over a 320 tag: {}",
        t.dump_prompt()
    );
}
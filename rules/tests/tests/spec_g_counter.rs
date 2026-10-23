//! G-COUNTER scenario tests -- [反击] windows, order, negation, and the
//! surrounding 专有名词.
//!
//! Per-act clauses live in [`rules_spec::global::counteract::CLAUSES`] and are
//! soaked by `tests/global_soak.rs`. This file holds the **scripted**
//! scenarios: the ask ring of 其他规则注意事项 3 (the live prompt is not in
//! `Step`), the "new timing only after this one closes" half of G-COUNTER-03,
//! multi-[指定] order, mandatory bodies, the [限] play gate, and the
//! 特别注意 3 override.
//!
//! Terminology: "counteraction" for [反击] windows, "hook" for passive
//! listeners. Never "reaction".
//!
//! Cards are real sheet entries (`docs/rulebook/cards.json`). The engine is
//! driven through `Table::counteract_offered` / `Table::counteract`.

use rules_spec::global::counteract::{check_ask_ring, turn_order_rank, CLAUSES};
use rules_spec::{forall, Table};

/// `通用:网络链接异常` -- 「[反击]1张手卡的[手]效果或事件卡的效果生效前」.
/// Negates the [手] body. Used as a counteraction against a hand play.
const NET: &str = "通用:网络链接异常";

/// `通用:10次招募（1回限定）` -- a plain mandatory [手] body
/// (「[消耗]1500资金，抽1张卡。」). The X of a counteraction window.
const RECRUIT: &str = "通用:10次招募（1回限定）";

/// `AG:宣战布告` -- 「[反击]当你或你拥有的格子被其他玩家的卡效果影响时：
/// [指定]那名玩家。被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡。」
const VOW: &str = "AG:宣战布告";

/// `通用:登上武道馆` -- 「[指定][使用者]以外的所有玩家，被[指定]的玩家
/// [支付][使用者]X资金。」 A multi-player [指定] fan-out.
const BUDOKAN: &str = "通用:登上武道馆";

/// `CRYCHIC:主唱太拼命了` -- 「[反击] 一次性向其他玩家支付至少5000资金时，
/// 免除此次支付。」 A card that overrides the base payment rule.
const ENCORE: &str = "CRYCHIC:主唱太拼命了";

/// `MyGO:[千早爱音]Anon Tokyo` -- 「[限]：自己所在的格子是[可购买格子]。」
const ANON: &str = "MyGO:[千早爱音]Anon Tokyo";

/// Drain every open [反击] offer by declining, collecting who was asked.
fn drain_counteract_ring(t: &mut Table) -> Vec<usize> {
    let mut asked_order = Vec::new();
    for _ in 0..16 {
        t.settle();
        let Some(p) = t.prompt() else { break };
        if p.title.key() != "ask.counteract.title" {
            break;
        }
        let asked = t.asked();
        if asked.is_empty() {
            break;
        }
        asked_order.push(asked[0]);
        let _ = t.decline_one(asked[0], p.id, p.fallback);
    }
    asked_order
}

// =====================================================================
// G-COUNTER-01 / G-COUNTER-07 -- 「结算优先于X」 / card overrides the book
// =====================================================================

/// G-COUNTER-01: a negated activation does not run its body.
///
/// Quote: 「[反击]…且结算优先于X」. `网络链接异常` answers a [手] play and
/// negates it; the [消耗]1500 body must not land.
#[test]
fn g_counter_01_negated_play_runs_no_body() {
    let mut t = Table::vanilla(3);
    t.set_draw(0, &[RECRUIT]); // so a draw would be visible if it ran
    t.give(0, &[RECRUIT]);
    t.give(1, &[NET]);
    t.set_money(0, 10_000);
    t.observe(CLAUSES);

    t.play(0, RECRUIT).unwrap();
    assert!(
        t.counteract_offered(NET),
        "the [反击] window must offer 网络链接异常: {}",
        t.dump_prompt()
    );
    t.counteract(1, NET).unwrap();
    // Drain whatever is left (a quiet lap).
    let _ = drain_counteract_ring(&mut t);
    t.settle();

    // The play flashed negated and its body did not run: money is unchanged.
    assert_eq!(
        t.money(0),
        10_000,
        "negated [手] body must not [消耗]1500 (G-COUNTER-01)"
    );
}

/// G-COUNTER-07 (特别注意 3): a card body overrides the base payment.
///
/// Quote: 「技能和卡的效果优先于规则书中的所有规则。」 `主唱太拼命了` cancels
/// a 5000+ payment the base rules would complete.
#[test]
fn g_counter_07_card_overrides_base_payment() {
    let mut t = Table::vanilla(3);
    let tile = rules_spec::tile("购物中心");
    t.own(0, &[tile]);
    t.set_houses(tile, 3); // rent[3] = 6680 >= 5000
    t.set_pos(1, 0);
    t.set_money(1, 20_000);
    t.give(1, &[ENCORE]);
    t.observe(CLAUSES);

    t.begin_turn(1);
    t.dice(&[1]); // CiRCLE -> 购物中心
    t.roll(1).unwrap();
    assert!(
        t.counteract_offered(ENCORE),
        "主唱太拼命了 must answer a 6680 rent: {}",
        t.dump_prompt()
    );
    t.counteract(1, ENCORE).unwrap();
    let _ = drain_counteract_ring(&mut t);
    t.settle();

    assert_eq!(
        t.money(1),
        20_000,
        "主唱太拼命了 「免除此次支付」 must override the base rent (G-COUNTER-07)"
    );
}

// =====================================================================
// G-COUNTER-02 -- ask ring order (scenario-only)
// =====================================================================

/// G-COUNTER-02: among seats able to counteract one timing, the ask ring
/// walks turn order from the player after the trigger.
///
/// Quote: 「从行动顺序上在触发[反击]时点的玩家的下一位玩家开始依次决定」.
/// P0 plays the [手] (the trigger); P1 and P2 hold `网络链接异常`.
#[test]
fn g_counter_02_ask_ring_starts_after_trigger_in_turn_order() {
    let mut t = Table::vanilla(3);
    t.give(0, &[RECRUIT]);
    t.give(1, &[NET]);
    t.give(2, &[NET]);
    t.observe(CLAUSES);

    t.play(0, RECRUIT).unwrap();
    let mut asked_order = Vec::new();
    for _ in 0..8 {
        t.settle();
        if t.prompt().is_none() {
            break;
        }
        let asked = t.asked();
        if asked.is_empty() {
            break;
        }
        assert!(
            t.counteract_offered(NET),
            "still the same [反击] window on {}: {}",
            RECRUIT,
            t.dump_prompt()
        );
        asked_order.push(asked[0]);
        let p = t.expect_prompt();
        let _ = t.decline_one(asked[0], p.id, p.fallback);
    }
    assert!(
        asked_order.len() >= 2,
        "two seats are able to counteract, so the ring must ask both: {asked_order:?}"
    );
    // P0 triggered the [手]; the book starts at P1.
    check_ask_ring(0, &asked_order, 3).unwrap_or_else(|e| panic!("{e} (asked {asked_order:?})"));
    assert_eq!(
        asked_order[0], 1,
        "the first able seat is the one after the trigger (P0): {asked_order:?}"
    );
}

/// G-COUNTER-02 (record): the trigger seat itself is not offered its own
/// link under `网络链接异常`. The book's 「下一位」 start and the engine's
/// §10 "initial user first" reading agree on the first *able* seat here,
/// because the trigger seat is not able.
#[test]
fn g_counter_02_trigger_seat_not_offered_own_link() {
    let mut t = Table::vanilla(3);
    t.give(0, &[RECRUIT, NET]);
    t.give(1, &[NET]);
    t.give(2, &[NET]);
    t.observe(CLAUSES);

    t.play(0, RECRUIT).unwrap();
    let mut asked_order = Vec::new();
    for _ in 0..8 {
        t.settle();
        if t.prompt().is_none() {
            break;
        }
        let asked = t.asked();
        if asked.is_empty() {
            break;
        }
        asked_order.push(asked[0]);
        let p = t.expect_prompt();
        let _ = t.decline_one(asked[0], p.id, p.fallback);
    }
    assert!(
        !asked_order.contains(&0),
        "the trigger seat is not asked under 网络链接异常 (card/engine self-guard): {asked_order:?}"
    );
    assert_eq!(
        asked_order,
        vec![1, 2],
        "turn order from the trigger: {asked_order:?}"
    );
}

// =====================================================================
// G-COUNTER-03 -- several counteractions to one timing; next timing after
// =====================================================================

/// G-COUNTER-03: two seats counteract the **same** timing; X settles only
/// after both.
///
/// Quote: 「多个效果可[反击]同一个时点…」.
#[test]
fn g_counter_03_two_counters_one_timing_settle_before_x() {
    let mut t = Table::vanilla(3);
    t.set_draw(0, &[RECRUIT]);
    t.give(0, &[RECRUIT]);
    t.give(1, &[NET]);
    t.give(2, &[NET]);
    t.set_money(0, 10_000);
    t.observe(CLAUSES);

    t.play(0, RECRUIT).unwrap();
    assert!(t.counteract_offered(NET), "{}", t.dump_prompt());
    let mark = t.mark();
    t.counteract(1, NET).unwrap();
    // The X-round is still open: P2 is asked about the same [手] play, not
    // about P1's counter (「…后可对新的时点发动[反击]」).
    assert!(
        t.counteract_offered(NET),
        "after one declaration the ring is still on X: {}",
        t.dump_prompt()
    );
    assert_eq!(
        t.asked(),
        vec![2],
        "the next seat on X: {}",
        t.dump_prompt()
    );
    t.counteract(2, NET).unwrap();
    let _ = drain_counteract_ring(&mut t);
    t.settle();

    assert_eq!(
        t.money(0),
        10_000,
        "X's [消耗]1500 must not land once the timing is countered (G-COUNTER-01/03)"
    );
    // Both counteractions are in the window, and X is negated.
    let evs = t.events_since(mark);
    let counters = evs
        .iter()
        .filter(|e| e.r#type == "card" && e.kind == "counter")
        .count();
    assert!(
        counters >= 2,
        "two counteractions on one timing (G-COUNTER-03): {:?}",
        t.keys_since(mark)
    );
}

/// G-COUNTER-03 (second half): a declared counter's own timing does not open
/// while the X-round is still collecting.
#[test]
fn g_counter_03_new_timing_waits_for_the_round_to_close() {
    let mut t = Table::vanilla(4);
    t.set_draw(0, &["通用:安可", RECRUIT]);
    t.give(0, &[BUDOKAN]);
    t.give(1, &[VOW]);
    t.give(2, &[VOW]);
    t.observe(CLAUSES);

    t.play(0, BUDOKAN).unwrap();
    assert!(
        t.counteract_offered(VOW),
        "宣战布告 must answer 「被其他玩家的卡效果影响」: {}",
        t.dump_prompt()
    );
    t.counteract(1, VOW).unwrap();
    // Still on the 登上武道馆 effect for the next designated player -- the
    // counter's own link is a **new** timing and has not opened yet.
    assert!(
        t.counteract_offered(VOW),
        "the ring is still on X (登上武道馆): {}",
        t.dump_prompt()
    );
    let p = t.expect_prompt();
    let detail = format!("{:?}", p.text);
    assert!(
        detail.contains(BUDOKAN) || detail.contains("登上武道馆"),
        "the open window still names X, not the declared counter: {}",
        t.dump_prompt()
    );
    // Close the rest of the ring.
    let _ = drain_counteract_ring(&mut t);
    t.settle();
}

// =====================================================================
// G-COUNTER-04 -- multi-[指定] in turn order from the user
// =====================================================================

/// G-COUNTER-04: `登上武道馆` designates every other player; the pay legs
/// run in turn order from the user.
///
/// Quote: 「[指定]多名玩家的效果从发动的玩家开始依次对行动顺序上的被[指定]
/// 玩家进行判定。」
///
/// X = 2000 ÷ (存活-1) 向上取整10 = 670 with 4 players.
#[test]
fn g_counter_04_multi_designation_pays_in_turn_order() {
    let mut t = Table::vanilla(4);
    t.give(0, &[BUDOKAN]);
    t.observe(CLAUSES);

    let mark = t.mark();
    t.play(0, BUDOKAN).unwrap();
    // Decline any counteract offers so the body settles.
    let _ = drain_counteract_ring(&mut t);
    t.settle();

    let evs = t.events_since(mark);
    let pays: Vec<usize> = evs
        .iter()
        .filter(|e| e.r#type == "pay" && e.other == 0)
        .map(|e| e.player_id as usize)
        .collect();
    assert_eq!(
        pays,
        vec![1, 2, 3],
        "multi-[指定] pay legs in turn order from user 0 (G-COUNTER-04)"
    );
    // Exact numbers: each designated player pays 670 to the user.
    assert_eq!(t.money(0), 10_000 + 670 * 3);
    for who in 1..4 {
        assert_eq!(t.money(who), 10_000 - 670, "player {who} pays 670");
    }
}

/// G-COUNTER-04 over randomized money pre-states (the fan-out amount is a
/// function of the alive count only, so the legs are always 670 with 4
/// players).
#[test]
fn g_counter_04_multi_designation_forall_money() {
    forall(24, 0xC0_C0_7E42, |rng, i| {
        let mut t = Table::vanilla(4);
        // Randomize the user's money so the pre-state is not one hand-picked
        // case; the amount owed does not depend on it.
        let m0 = 8_000 + rng.range(0, 40) * 100;
        t.set_money(0, m0);
        t.give(0, &[BUDOKAN]);
        t.observe(CLAUSES);
        let mark = t.mark();
        t.play(0, BUDOKAN).map_err(|e| format!("play: {e}"))?;
        let _ = drain_counteract_ring(&mut t);
        t.settle();
        let evs = t.events_since(mark);
        let pays: Vec<usize> = evs
            .iter()
            .filter(|e| e.r#type == "pay" && e.other == 0)
            .map(|e| e.player_id as usize)
            .collect();
        if pays != vec![1, 2, 3] {
            return Err(format!("iter {i}: turn order {pays:?}"));
        }
        if t.money(0) != m0 + 670 * 3 {
            return Err(format!("iter {i}: user money {}", t.money(0)));
        }
        for who in 1..4 {
            if t.money(who) != 10_000 - 670 {
                return Err(format!("iter {i}: player {who} money {}", t.money(who)));
            }
        }
        Ok(())
    })
    .expect("G-COUNTER-04 forall");
}

// =====================================================================
// G-COUNTER-05 -- mandatory when not written optional
// =====================================================================

/// G-COUNTER-05: `宣战布告`'s body (pay 500 + draw 1) is not written
/// optional, so both halves must run once the counteraction is played.
///
/// Quote: 「非写明可选择的效果在可发动时必须发动。」
#[test]
fn g_counter_05_mandatory_vow_body_runs_both_halves() {
    let mut t = Table::vanilla(4);
    t.set_draw(1, &["通用:安可"]); // so the draw has a card to take
    t.give(0, &[BUDOKAN]);
    t.give(1, &[VOW]);
    t.set_money(0, 10_000);
    t.set_money(1, 10_000);
    t.observe(CLAUSES);

    t.play(0, BUDOKAN).unwrap();
    assert!(t.counteract_offered(VOW), "{}", t.dump_prompt());
    let mark = t.mark();
    t.counteract(1, VOW).unwrap();
    let _ = drain_counteract_ring(&mut t);
    t.settle();

    // 「被[指定]的玩家[支付][使用者]500资金」 -- P0 (the one who affected P1)
    // pays the [使用者] of 宣战布告 (P1) 500.
    // 「且[使用者]抽1张卡」 -- P1 draws 1.
    // Both are mandatory.
    let evs = t.events_since(mark);
    let paid_500 = evs.iter().any(|e| e.r#type == "pay" && e.value == 500);
    assert!(
        paid_500,
        "mandatory [支付]500 must run: {:?}",
        t.keys_since(mark)
    );
    assert_eq!(
        t.hand(1).len(),
        1,
        "mandatory 抽1张卡 must run (hand: the drawn card; the played VOW left): {:?}",
        t.hand(1)
    );
}

// =====================================================================
// G-COUNTER-14 / terminology -- [限] play gate
// =====================================================================

/// G-COUNTER-14: a [限] card is only playable when X holds.
///
/// Quote: 「带有“[限]X”的卡只有在X成立时才可从手中打出。」
/// `Anon Tokyo`'s X is 「自己所在的格子是[可购买格子]」.
#[test]
fn g_counter_14_limit_gate_refuses_off_gate_play() {
    let mut t = Table::vanilla(3);
    t.set_character_raw(0, "千早爱音");
    t.observe(CLAUSES);

    // CiRCLE is not a [可购买格子].
    t.set_pos(0, 0);
    t.give(0, &[ANON]);
    let r = t.play(0, ANON);
    assert!(
        r.is_err(),
        "[限] must refuse the play from CiRCLE (G-COUNTER-14): {r:?}"
    );
    assert!(
        t.hand(0).contains(&ANON.to_string()),
        "a refused [限] play stays in hand: {:?}",
        t.hand(0)
    );

    // 购物中心 is a [可购买格子].
    t.set_pos(0, rules_spec::tile("购物中心"));
    t.give(0, &[ANON]);
    let r = t.play(0, ANON);
    assert!(
        r.is_ok(),
        "[限] X holds on 购物中心, so the play must go through: {r:?} {}",
        t.dump_prompt()
    );
}

// =====================================================================
// G-COUNTER-10 / G-COUNTER-15 -- [使用者] / [手]
// =====================================================================

/// G-COUNTER-10 + G-COUNTER-15: the [使用者] of a [手] play is the seat that
/// sent the act, and the [手] body runs for them.
#[test]
fn g_counter_10_and_15_hand_play_user_and_body() {
    let mut t = Table::vanilla(3);
    t.set_draw(0, &["通用:安可"]);
    t.give(0, &[RECRUIT]);
    t.set_money(0, 10_000);
    t.observe(CLAUSES);

    let mark = t.mark();
    t.play(0, RECRUIT).unwrap();
    let _ = drain_counteract_ring(&mut t);
    t.settle();

    // [消耗]1500 + draw 1: hand is 0 (played) + 1 (drawn) = 1, money -1500.
    assert_eq!(t.money(0), 8_500, "[手] body [消耗]1500 (G-COUNTER-15)");
    assert_eq!(t.hand(0).len(), 1, "[手] body 抽1张卡");
    let evs = t.events_since(mark);
    let act = evs
        .iter()
        .find(|e| e.r#type == "card" && e.kind == "play" && e.card == RECRUIT)
        .expect("a play activation");
    assert_eq!(
        act.player_id, 0,
        "[使用者] is the seat that played (G-COUNTER-10)"
    );
}

// =====================================================================
// G-COUNTER-06 -- band / character targeting uses currently held cards
// =====================================================================

/// G-COUNTER-06: 「对乐队或角色生效的效果根据玩家当前拥有的乐队卡和角色卡
/// 进行判定且玩家执行效果。」
///
/// `丸山彩` (1) gives every **non-Pastel✽Palettes** player her (2) skill.
/// The filter is the band card the player currently holds.
#[test]
fn g_counter_06_band_filter_uses_held_band_cards() {
    // 丸山彩 is Pastel✽Palettes; 户山香澄 is Poppin' Party. Setup deals each
    // player their character card and the matching band card
    // (「拿取选择角色的角色卡和对应的乐队卡」).
    let mut t = Table::new(&["丸山彩", "户山香澄"]);
    t.observe(CLAUSES);
    assert_eq!(t.p(0).persona, "丸山彩");
    assert_eq!(t.p(1).persona, "户山香澄");

    // 丸山彩 (1): 「所有非Pastel✽Palettes玩家获得丸山彩的（2）技能」. The
    // filter is the band card the player currently holds -- P1 is not PP, so
    // P1's field carries the gifted (2) (`skill:丸山彩:With~`).
    assert!(
        t.skills(1).iter().any(|s| s.contains("丸山彩")),
        "non-PP seat holds the gifted 丸山彩 (2): {:?}",
        t.skills(1)
    );
    // Pastel✽Palettes band (1): 「游戏开始时非Pastel✽Palettes角色获得1个
    // 反面[P✽P粉丝]」. P1 (non-PP) gets 1 反面; P0 (PP) does not.
    // Fan counters live in the per-player state dict (`ppFansUp`/`ppFansDown`).
    assert_eq!(
        t.state(1, "ppFansDown"),
        1,
        "non-PP seat gets the band (1) 反面 fan"
    );
    assert_eq!(
        t.state(0, "ppFansDown"),
        0,
        "PP seat does not get the band (1) 反面 fan"
    );
    // 丸山彩 (1) also: 「游戏开始后获得5个正面[P✽P粉丝]」 for herself.
    assert_eq!(t.state(0, "ppFansUp"), 5);
}

// =====================================================================
// helper
// =====================================================================

#[test]
fn turn_order_rank_walks_the_ring() {
    assert_eq!(turn_order_rank(0, 0, 3), 0);
    assert_eq!(turn_order_rank(0, 1, 3), 1);
    assert_eq!(turn_order_rank(0, 2, 3), 2);
    assert_eq!(turn_order_rank(2, 0, 3), 1);
    assert_eq!(turn_order_rank(2, 1, 3), 2);
    assert_eq!(turn_order_rank(2, 2, 3), 0);
}

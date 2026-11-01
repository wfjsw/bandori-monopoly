//! G02 pay pipeline -- §9 black-box rows of
//! `docs/rulebook/spec/abi/G02-pay-pipeline.md` (row 7 lives in `fire.rs`).
//!
//! Every test cites the 规则书 fragment it pins. Where the engine and the book
//! disagree the row is `#[ignore]`d with the row id, per
//! `docs/rulebook/TEST-FINDINGS.md`.

mod common;

use common::*;
use game_core::engine::CardRules;
use game_core::msg::{Arg, Msg};

const RUI_DEVIL: &str = "Mor:（Rui）正论恶魔";
const MASHIRO_PAY: &str = "Mor:（小白）";
const KOKORO_CIRCLE: &str = "HHW:（kkr）前往笑容集结的地方！";
const SAKIKO_LEAD: &str = "CRYCHIC:（祥子）带领着大家";
const REPAINT: &str = "RAS:Repaint";
const STUDIO_STORM: &str = "RAS:练习室里的风暴";
const TAKI_SERIOUS: &str = "MyGO:（立希）想认真去做";
const RANA_FUNNY: &str = "MyGO:（乐奈）有趣的女人";
const TOMORI: &str = "MyGO:（灯）不再迷茫";
const ANON_TOKYO: &str = "MyGO:[千早爱音]Anon Tokyo";
const FIRE_BIRD: &str = "R:Fire bird";
const DOOR: &str = "PPP:Tomorrow's Door";
const TRITONE: &str = "Mor:迷茫之蝶们的三全音";
const AYA: &str = "PP:[丸山彩]憧憬的前方";
const MOCA: &str = "AG:（摩卡）0.5倍速";
const GUERRILLA: &str = "RAS:游击演出";
const COUNCIL: &str = "R:学生会的检查";
const PARKING: &str = "通用:[都筑诗船]Parking Space";
const VOCAL: &str = "CRYCHIC:主唱太拼命了";
const SAKI_MOVE: &str = "Mujica:祥，移动";
const POPI: &str = "PPP:[衍生]Popipapapipopa";
const HAGUMI: &str = "HHW:（育美）";
const CRIT: &str = "skill:八潮瑠唯:正论暴击";
const UMIRI: &str = "skill:八幡海铃:熟练的支援贝斯手";
const X: &str = "skill.yuriCrit.x";
const SPLIT_KEY: &str = "skill.kaedeSupport.split.";

fn place_on_tile(t: &mut Table, who: usize, card: &str, tile: usize) {
    let d = data();
    let props = rules().card_props(card);
    t.m.world_mut()
        .place_card_on(&d, who as i32, tile as i32, card, Msg::default(), props);
}

/// `docs/rulebook/spec/ROUNDING.md`: round up to a multiple of 10 once, after
/// every add/mul.
fn redeem_cost(tile: usize) -> i32 {
    (data().tiles[tile].price * 3 + 2) / 5
}

fn ceil10(x: i32) -> i32 {
    if x <= 0 {
        x
    } else {
        ((x as i64 + 9) / 10 * 10) as i32
    }
}

fn drain(t: &mut Table) {
    for _ in 0..500 {
        let Some(p) = t.prompt() else { return };
        // An auction's 0 fallback is below the 100 minimum, so every asked
        // seat is *passed* (`value < 0`) instead.
        if p.kind == "auction" {
            for who in t.asked() {
                let _ = t.answer(who, -1);
            }
            continue;
        }
        // A mortgage raise wants deeds chosen, not a yes/no.
        if p.kind == "mortgage" {
            let need = match p.text.args.get("need") {
                Some(Arg::Number(n)) => *n as i32,
                _ => 0,
            };
            let mut covered = 0;
            let mut picks: Vec<&str> = Vec::new();
            for it in &p.items {
                if covered >= need {
                    break;
                }
                let tile: usize = it.parse().unwrap_or(0);
                covered += data().tiles[tile].price / 2;
                picks.push(it);
            }
            let who = *p.players.first().unwrap_or(&0) as usize;
            t.answer_items(who, &picks).unwrap();
            continue;
        }
        t.decline();
    }
    panic!("drain livelock at the {}. prompt: {}", 500, t.dump_prompt());
}

fn pass(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

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

/// Walk `who` one tile forward onto `tile` with a loaded die.
fn land_on(t: &mut Table, who: usize, tile: usize) {
    until_turn(t, who);
    let n = data().tiles.len();
    t.set_pos(who, (tile + n - 1) % n);
    t.dice(&[1]);
    t.roll(who).unwrap();
    drain(t);
}

/// Walk `who` one tile forward onto `tile` with a loaded die and take CiRCLE's
/// card half of the [pass-reward], so the money figures stay untouched.
fn land_on_card(t: &mut Table, who: usize, tile: usize) {
    until_turn(t, who);
    let n = data().tiles.len();
    t.set_pos(who, (tile + n - 1) % n);
    t.dice(&[1]);
    t.roll(who).unwrap();
    while let Some(p) = t.prompt() {
        let reward = format!("{:?}", p.title).contains("pass_reward");
        let k = if reward && p.options.len() > 1 {
            1
        } else {
            p.fallback
        };
        for w in t.asked() {
            let _ = t.answer(w, k);
        }
        t.settle();
    }
}

/// Put `who`'s Tomorrow's Door into their **play area** (route finished,
/// `tile = -1`), which is when its (3) tax applies.
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

fn answer_player(t: &mut Table, who: usize, target: i32) {
    let p = t.expect_prompt();
    let k = p
        .options
        .iter()
        .position(|o| match o.args.get("who") {
            Some(Arg::PlayerId(v)) => *v == target,
            Some(Arg::Integer(v)) | Some(Arg::Number(v)) => *v == target as i64,
            _ => false,
        })
        .unwrap_or_else(|| panic!("player {target} not offered: {}", t.dump_prompt()));
    t.answer(who, k as i32).unwrap();
}

// =====================================================================
// 1 -- the fixed leg (row 1)
// =====================================================================

// 规则书 `Mor:（Rui）正论恶魔`: 「向全场玩家支付100资金（该数值不可被任何效果
// 影响），之后全场玩家向你支付300资金（可被影响）。在此卡结算过程中，每当你的
// 技能被触发后，立即将x设置为5」.
//
// The fixed 100 leaves the modifier stages alone (a crit hit voids a loss, so an
// unfixed leg would settle at 0); the 300-back leg is a plain transfer and
// still doubles.
#[test]
fn g02_rui_fixed_leg_ignores_pay_modifiers() {
    // (a) every leg is hit (X=20 and the card's fire hook re-writes 5 at each
    // leg): the fixed 100 leg still pays 100 -- the hit's voided loss is not
    // folded in -- while the plain 300-back leg doubles.
    let mut t = Table::new(&["八潮瑠唯", "高松灯"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t.place_raw(0, CRIT);
    t.set_state(0, X, 20);
    t.dice(&[1; 8]);
    t.give_play(0, RUI_DEVIL).unwrap();
    drain(&mut t);
    assert_eq!(t.state(0, X), 5, "the card overrides X after each leg");
    assert_eq!(
        t.money(0),
        10_000 - 100 + 600,
        "the 100 stayed 100 (not voided by the hit): keys {:?}",
        t.recent_keys(8)
    );
    assert_eq!(t.money(1), 10_000 + 100 - 600);

    // (b) only the 300-back leg is hit: the crit doubles it even though the
    // 100 leg was missed. (The first die 6 exceeds the hook's X=5, the second
    // die 1 is under the re-written 5.) The missed fixed leg still paid 100.
    let mut t = Table::new(&["八潮瑠唯", "高松灯"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t.place_raw(0, CRIT);
    t.set_state(0, X, 1);
    t.dice(&[6, 1, 1, 1]);
    t.give_play(0, RUI_DEVIL).unwrap();
    drain(&mut t);
    assert_eq!(
        t.money(0),
        10_000 - 100 + 600,
        "the 300 doubled: {:?}",
        t.recent_keys(8)
    );
    assert_eq!(t.money(1), 10_000 + 100 - 600);

    // (c) crit misses every leg: the plain 100 / 300.
    let mut t = Table::new(&["八潮瑠唯", "高松灯"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t.place_raw(0, CRIT);
    t.set_state(0, X, 0);
    t.dice(&[20; 8]);
    t.give_play(0, RUI_DEVIL).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000 - 100 + 300);
    assert_eq!(t.money(1), 10_000 + 100 - 300);
    // Both legs landed on the fixed/plain figures, not 0 and not 200.
    let keys = t.recent_keys(12);
    assert!(
        keys.iter().any(|k| k.contains("rui_devil")),
        "the card resolved: {keys:?}"
    );
}

// =====================================================================
// 2 -- 小白: loss redirect + payee half (row 2)
// =====================================================================

// 规则书 `Mor:（小白）`: 「[反击]当你将要向其他玩家支付时打出此卡，此次支付改为
// 失去同等的资金并令此次支付的对象失去此次金额一半的资金」.
//
// 「此次金额」 is the **settled** figure: with a 0.5 pay factor in force, a
// declared 1000 settles at 500 and the payee loses 250 -- not half the declared
// 1000. The payer's 500 goes to the bank, not to the payee.
#[test]
fn g02_mashiro_redirect_is_loss_and_half_settles() {
    let mut t = Table::new(&["仓田真白", "高松灯"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    // P0 is the payer (the card's 「你」), so the 0.5 modifier sits on P0.
    t.place_raw(0, MOCA);
    t.give(0, &[MASHIRO_PAY]);
    // A second 0.5 through the plan: 1000 -> 500 (moca) -> 250 (plan).
    t.m.world_mut().turn.plan.pay_factor = 0.5;
    let before = (t.money(0), t.money(1));
    // P0 pays P1 1000 (x0.5 moca x0.5 plan = 250 settled).
    t.give_play(0, "TEST:xfer1000").unwrap();
    assert!(
        t.counteract_offered(MASHIRO_PAY),
        "the redirect is offered: {}",
        t.dump_prompt()
    );
    t.counteract(0, MASHIRO_PAY).unwrap();
    drain(&mut t);
    assert_eq!(
        before.0 - t.money(0),
        250,
        "the payer loses the settled amount to the bank: keys {:?}",
        t.recent_keys(10)
    );
    assert_eq!(
        before.1 - t.money(1),
        125,
        "the payee loses exactly half of the settled figure"
    );
}

// =====================================================================
// 3 -- sakiko: two halves compose, the move consumes them (row 3)
// =====================================================================

// 规则书 `CRYCHIC:（祥子）带领着大家`: 「使你的下次主要移动结果对那些玩家一起
// 执行…触发结算时进行的[支付]价格减半」.
//
// Two live copies give two `NextMove` halves -> x0.25 on that move's settle; the
// following move is full again (the scope is *the next* move).
#[test]
fn g02_sakiko_two_halves_compose_and_leave_with_move() {
    let mut t = Table::new(&["丰川祥子（CRYCHIC）", "椎名立希", "要乐奈"]);
    t.strip_skills();
    t.clean();
    let land = tile("水族馆");
    t.own(1, &[land]);
    t.begin_turn(0);
    // P1 and P2 share P0's tile -- the play gate.
    t.set_pos(1, 0);
    t.set_pos(2, 0);
    t.give(0, &[SAKIKO_LEAD, SAKIKO_LEAD]);
    t.play(0, SAKIKO_LEAD).unwrap();
    t.play(0, SAKIKO_LEAD).unwrap();
    drain(&mut t);
    assert_eq!(
        t.m.world().turn.next_move_pay_factor,
        250,
        "two halves compose to a quarter (milli) on the next move"
    );
    // P0 walks onto P1's 320-rent tile and pays a quarter.
    t.set_pos(0, land - 2);
    t.dice(&[2]);
    let (m0, m1) = (t.money(0), t.money(1));
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(m0 - t.money(0), 80, "320 x0.25 = 80");
    assert_eq!(t.money(1) - m1, 80);

    // The next move is back at full price. (The followers moved inside P0's
    // turn, so the turn still has to be ended first.)
    t.end(0).unwrap();
    drain(&mut t);
    until_turn(&mut t, 0);
    t.set_pos(0, land - 1);
    t.dice(&[1]);
    let m0 = t.money(0);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(m0 - t.money(0), 320, "the next move pays full");
}

// =====================================================================
// 4 -- kokoro's 10000 is a Buy and must (row 4)
// =====================================================================

// 规则书 `HHW:（kkr）前往笑容集结的地方！`: 「支付10000资金（视为买地花费）并将此
// 卡置于CiRCLE上」.
//
// 「视为买地花费」 stamps the payment `Buy`, which the settle scale
// (`scale_settle_payment`) skips -- a 0.5 pay factor leaves the 10000 alone --
// and the mandatory builder default makes a short payer mortgage and then go out
// rather than pay part of it.
#[test]
fn g02_kokoro_10000_is_buy_and_must() {
    // (a) Buy: a x0.5 factor on the payer does not touch the 10000.
    let mut t = Table::new(&["弦卷心", "高松灯"]);
    t.strip_skills();
    t.clean();
    t.set_money(0, 10_000);
    t.m.world_mut().turn.plan.pay_factor = 0.5;
    t.give_play(0, KOKORO_CIRCLE).unwrap();
    drain(&mut t);
    assert_eq!(
        t.money(0),
        0,
        "the 10000 landed unscaled: keys {:?}",
        t.recent_keys(8)
    );
    assert!(t.on_field(0, KOKORO_CIRCLE), "the card is placed");

    // (b) must: 5000 plus mortgageable deeds -> the shortfall is raised by
    // mortgaging, never by underpaying 5000.
    let mut t = Table::new(&["弦卷心", "高松灯"]);
    t.strip_skills();
    t.clean();
    let deeds = [tile("弦卷豪宅"), tile("武道馆"), tile("旧古河庭园")];
    t.own(0, &deeds);
    t.set_money(0, 5000);
    t.give_play(0, KOKORO_CIRCLE).unwrap();
    drain(&mut t);
    assert!(
        !t.p(0).bankrupt,
        "the shortfall was raisable: keys {:?}",
        t.recent_keys(12)
    );
    assert!(
        deeds.iter().any(|&d| t.mortgaged(d)),
        "the shortfall was raised by mortgaging"
    );
    // Over-raising leaves mortgage change; the point is the full 10000 landed
    // (5000 of it raised), never a 5000 underpayment.
    assert!(t.money(0) < 5000, "the full 10000 left: {}", t.money(0));
    assert!(t.on_field(0, KOKORO_CIRCLE), "the card is placed");

    // (c) nothing to mortgage: out rather than an underpayment.
    let mut t = Table::new(&["弦卷心", "高松灯"]);
    t.strip_skills();
    t.clean();
    t.set_money(0, 5000);
    t.give_play(0, KOKORO_CIRCLE).unwrap();
    drain(&mut t);
    assert!(
        t.p(0).bankrupt,
        "the payer went out: keys {:?}",
        t.recent_keys(12)
    );
    assert_eq!(t.money(0), 0);
}

// =====================================================================
// 5 -- kokoro's 6000 is a rent (row 5)
// =====================================================================

// 规则书 `HHW:（kkr）前往笑容集结的地方！`: 「若其他玩家在该格[触发结算]则向所有者
// 支付6000资金，视为格子的收款」.
//
// 「视为格子的收款」 stamps `Rent`: a `pay_is_rent` hook (`umiri` 状态2) sees it,
// and a rent factor halves it.
#[test]
fn g02_kokoro_6000_is_rent_kind() {
    let mut t = Table::vanilla(3);
    t.strip_skills();
    t.clean();
    // P1 owns the placed 6000; P2 has 海铃 in 状态2 splitting tile 0.
    place_on_tile(&mut t, 1, KOKORO_CIRCLE, 0);
    t.place_raw(2, UMIRI);
    t.set_state(2, "skillState", 2);
    t.set_state(2, &format!("{SPLIT_KEY}0"), 1);
    t.m.world_mut().turn.plan.rent_factor = 0.5;
    let before = (t.money(0), t.money(1), t.money(2));
    let m = t.mark();
    land_on_card(&mut t, 0, 0);
    let evts: String = t
        .events_since(m)
        .iter()
        .map(|e| format!("{:?}", e.msg))
        .collect();
    // The rent factor halves the 6,000 to 3,000 and the umiri split halves the
    // owner's take again; `pay_is_rent` is what let both stages see it.
    assert_eq!(
        t.m.world().paid_in_settle_by(0),
        1500,
        "6000 x0.5 (factor) x0.5 (split) left the payer: keys {:?}",
        t.recent_keys(12)
    );
    assert_eq!(t.money(1) - before.1, 1500, "the owner's halved take");
    assert_eq!(t.money(2) - before.2, 3000, "the pay_is_rent hook fired");
    assert!(evts.contains("umiri_split"), "the split ran: {evts}");
}

// =====================================================================
// 6 -- kokoro's 6000 carries CiRCLE as its tile (row 6)
// =====================================================================

// 规则书 `HHW:（kkr）前往笑容集结的地方！`: 「在该格…视为格子的收款」 -- the
// 6,000 is a collection of **CiRCLE**.
//
// Both tile-keyed riders see it: `PPP:Tomorrow's Door` (支付门票加价, keyed on the
// settle tile when it is 梦开始的地方 or an owner tile) and `umiri` 状态2's
// tile-keyed split.
#[test]
fn g02_kokoro_6000_carries_circle_tile() {
    let mut t = Table::vanilla(3);
    t.strip_skills();
    t.clean();
    let circle = tile("CiRCLE");
    // P0: the door in its play area, owns CiRCLE, 2 houses on the hill.
    t.own(0, &[circle]);
    door_to_play_area(&mut t, 0);
    t.own(0, &[tile("星之鼓动山丘")]);
    t.set_houses(tile("星之鼓动山丘"), 2);
    // P1 keeps the 6000 (the card sits on CiRCLE), P2 splits on tile 0.
    place_on_tile(&mut t, 1, KOKORO_CIRCLE, circle);
    t.place_raw(2, UMIRI);
    t.set_state(2, "skillState", 2);
    t.set_state(2, &format!("{SPLIT_KEY}{circle}"), 1);
    let before = (t.money(0), t.money(1), t.money(2));
    let m = t.mark();
    land_on_card(&mut t, 0, circle);
    let evts: String = t
        .events_since(m)
        .iter()
        .map(|e| format!("{:?}", e.msg))
        .collect();
    assert!(
        evts.contains("tomorrows_door_tax"),
        "the door taxed: {evts}"
    );
    assert!(
        evts.contains("umiri_split"),
        "umiri split on the tile: {evts}"
    );
    // The 6,000 is CiRCLE's own collection, so both tile-keyed riders see it:
    // the split halves the payer's charge and the tax lands on the owner.
    assert_eq!(t.m.world().paid_in_settle_by(0), 3000, "half of the 6,000");
    assert_eq!(t.money(1) - before.1, 3000, "the owner's half");
    assert!(
        t.money(2) - before.2 > 1500,
        "the tax rode in: {}",
        t.money(2) - before.2
    );
}

// =====================================================================
// 8 -- popi crystals at redeem and auction (row 8)
// =====================================================================

// 规则书 `PPP:[衍生]Popipapapipopa` (2): 「消耗或支付时可使用此卡多个奇迹水晶，每个
// 将资金变动减少150（最少0）」.
//
// The `spendBefore` window carries the **full** cost of a raw spend, so the
// crystals can cut it; 2 crystals cut 300.
#[test]
fn g02_popi_crystal_choice_on_redeem_and_auction() {
    // (a) redeem.
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.clean();
    let deed = tile("水族馆");
    t.own(0, &[deed]);
    t.set_mortgaged(deed, true);
    t.place_raw(0, POPI);
    t.set_crystals(0, POPI, 5);
    let cost = redeem_cost(deed);
    let before = t.money(0);
    t.redeem(0, deed).unwrap();
    let p = t.expect_prompt();
    assert_eq!(
        p.kind,
        "choice",
        "the spendBefore hook asks: {}",
        t.dump_prompt()
    );
    // The window saw the full cost.
    assert!(
        format!("{p:?}").contains(&cost.to_string()),
        "the window carries the full cost {cost}: {}",
        t.dump_prompt()
    );
    t.answer_one(2).unwrap();
    drain(&mut t);
    assert!(!t.mortgaged(deed), "the deed was redeemed");
    assert_eq!(t.money(0), before - (cost - 300), "2 crystals cut 300");
    assert!(
        t.recent_keys(10)
            .iter()
            .any(|k| k.contains("popipapapipopa_used")),
        "the crystals were spent: {:?}",
        t.recent_keys(10)
    );

    // (b) auction: a leftover auction's bid is a raw spend with the same window.
    let mut t = Table::vanilla(3);
    t.strip_skills();
    t.clean();
    let rent_owned = tile("购物中心");
    t.own(2, &[rent_owned]);
    t.set_houses(rent_owned, 3);
    let deed = tile("天文馆");
    t.own(0, &[deed]);
    t.set_mortgaged(deed, true);
    t.set_money(0, 0);
    // P1 bids 500 on the auctioned deed, with popi armed.
    t.place_raw(1, POPI);
    t.set_crystals(1, POPI, 5);
    // Roll by hand: `land_on`'s default drain would answer the auction prompt
    // with its 0 fallback, which is below the 100 minimum.
    t.begin_turn(0);
    let n = data().tiles.len();
    t.set_pos(0, (rent_owned + n - 1) % n);
    t.dice(&[1]);
    t.roll(0).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.kind, "auction", "a live auction: {}", t.dump_prompt());
    let bidder = t.asked()[0];
    let other = t.asked().into_iter().find(|&s| s != bidder).unwrap();
    t.answer(bidder, 500).unwrap();
    // The other seat passes (`value < 0`), so the bid stands.
    t.answer(other, -1).unwrap();
    let p = t.expect_prompt();
    assert_eq!(
        p.kind,
        "choice",
        "the bid opened spendBefore: {}",
        t.dump_prompt()
    );
    t.answer_one(2).unwrap();
    drain(&mut t);
    assert_eq!(t.owner(deed), Some(bidder), "the bidder owns the deed");
    if bidder == 1 {
        assert_eq!(t.money(bidder), 10_000 - 200, "the 500 bid was cut by 300");
    }
    assert!(
        t.recent_keys(20)
            .iter()
            .any(|k| k.contains("popipapapipopa_used")),
        "the crystals were spent: {:?}",
        t.recent_keys(20)
    );
}

// =====================================================================
// 9 -- aya's 1500 rides the total stage, once (row 9)
// =====================================================================

// 规则书 `PP:[丸山彩]憧憬的前方` [持续] (2): 「[共鸣][反击][消耗]或[支付]时将金额
// 降低1500（最低0）」.
//
// A 分摊 command raises it **once** at the command total (the payment stage is
// 「支付计算时」, before the legs), and a sibling add modifier folds in on the
// same figure.
#[test]
fn g02_aya_1500_once_per_command_at_stage_two() {
    let mut t = Table::vanilla(4);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    // The payers are the other seats; P1 is the first of them, so aya (its
    // 「你」) sits with P1 and the 共鸣 fuel is in P1's hand.
    t.place_raw(1, AYA);
    t.place_raw(1, "TEST:totalCut");
    t.give(1, &["PP:[衍生]共鸣"]);
    // Not the poorest, so (1)'s X shave stays out of the arithmetic.
    t.set_money(1, 50_000);
    let before = (t.money(0), t.money(1), t.money(2), t.money(3));
    t.give_play(0, "TEST:split12k").unwrap();
    // One 共鸣 offer on the command total (12,000 - 1,500), not per leg.
    let p = t.expect_prompt();
    assert!(
        format!("{:?}", p.title).contains("resonance"),
        "the 共鸣 link is offered: {}",
        t.dump_prompt()
    );
    t.answer_one(0).unwrap();
    drain(&mut t);
    // 12000 - 1500 (aya) - 500 (sibling add) = 10,000, split three ways: the
    // shaped total goes to 3,334 a head, ceil10 = 3,340. The 共鸣 fuel was
    // spent once -- a per-leg cut would give 4000 - 1500 - 500 = 2,000 a head.
    let hand: Vec<String> = t.m.world().hidden[1]
        .hand
        .iter()
        .map(|c| c.to_string())
        .collect();
    assert!(
        !hand.iter().any(|c| c.contains("共鸣")),
        "the fuel was spent: {hand:?}"
    );
    let legs = (
        before.1 - t.money(1),
        before.2 - t.money(2),
        before.3 - t.money(3),
    );
    assert_eq!(legs, (3340, 3340, 3340), "one cut on the total, split flat");
    assert_eq!(
        t.money(0) - before.0,
        3 * 3340,
        "the receiver got every leg"
    );
}

// =====================================================================
// 10 -- the door surcharge needs the settle tile (row 10)
// =====================================================================

// 规则书 `PPP:Tomorrow's Door` (3): 「此卡在自身游玩区域时[拥有者]以外的玩家在
// [拥有者]拥有的格子或梦开始的地方[结算]时额外支付…」.
//
// The clause keys on the **settle**, so a rent-shaped 收费 outside a settle
// (`charge_rent`, stamped `by_card`) collects nothing, while a settle on the
// owner's tile collects the tax.
#[test]
fn g02_door_surcharge_needs_settle_tile() {
    let mut t = Table::vanilla(3);
    t.strip_skills();
    t.clean();
    let lh = tile("Live House Galaxy");
    let storm_at = tile("DUB MUSIC EXPERIMENT");
    t.own(0, &[lh]);
    door_to_play_area(&mut t, 0);
    t.own(0, &[tile("星之鼓动山丘")]);
    t.set_houses(tile("星之鼓动山丘"), 1);
    // P2 plants the storm on their own Live House.
    t.own(2, &[storm_at]);
    t.set_pos(2, storm_at);
    t.give(2, &[STUDIO_STORM]);
    t.begin_turn(2);
    t.play(2, STUDIO_STORM).unwrap();
    drain(&mut t);
    t.settle();
    // P2 passes CiRCLE twice for the storm's crystals.
    for k in 0..2 {
        if k > 0 {
            pass(&mut t, 2);
            until_turn(&mut t, 2);
        }
        t.set_pos(2, 59);
        t.dice(&[1]);
        t.roll(2).unwrap();
        drain(&mut t);
    }
    assert_eq!(
        t.crystals(2, STUDIO_STORM),
        Some(2),
        "two CiRCLE passes charged it"
    );

    // (a) the storm's remote 收费 from a tile 2 away (x2 = half): no settle, no
    // tax. The victim settles on an unowned tile.
    let victim = 1;
    let at = storm_at + 2;
    t.set_pos(victim, at - 1);
    let before = t.money(victim);
    // A plain settle one tile from the storm's tile fires it; that settle is on
    // an unowned tile, so the door has no settle tile of P0's to tax.
    t.begin_turn(victim);
    t.dice(&[1]);
    t.roll(victim).unwrap();
    drain(&mut t);
    let paid = before - t.money(victim);
    assert_eq!(
        paid,
        120,
        "only the storm's 240 x (4-2)/4: {:?}",
        t.recent_keys(16)
    );
    assert!(
        !t.recent_keys(16)
            .iter()
            .any(|k| k.contains("tomorrows_door_tax")),
        "no tax outside the owner's settle"
    );

    // (b) a settle on P0's own tile collects the tax as well.
    let mut t = Table::vanilla(3);
    t.strip_skills();
    t.clean();
    let owned = tile("购物中心");
    t.own(0, &[owned]);
    door_to_play_area(&mut t, 0);
    t.own(0, &[tile("星之鼓动山丘")]);
    t.set_houses(tile("星之鼓动山丘"), 1);
    let before = t.money(1);
    land_on(&mut t, 1, owned);
    let keys = t.recent_keys(12);
    assert_eq!(
        before - t.money(1),
        400,
        "300 rent + 100 tax on the settle: {keys:?}"
    );
    assert!(
        keys.iter().any(|k| k.contains("tomorrows_door_tax")),
        "the tax joined: {keys:?}"
    );
}

// =====================================================================
// 11 -- anon's link charges half of the real charge (row 11)
// =====================================================================

// 规则书 `MyGO:[千早爱音]Anon Tokyo` [手]1: 「被[奇迹水晶]连接的格子收费时，会额外
// 收取被连接的其他格子收费的一半」.
//
// 「收费的一半」 is the partner's real quote: a mortgaged partner adds 0, and no
// in-flight factor of the triggering payment is applied to the quote again.
#[test]
fn g02_anon_half_of_real_charge() {
    let build = |t: &mut Table| {
        let a = tile("购物中心");
        let b = tile("天文馆");
        t.own(1, &[a, b]);
        t.set_pos(1, a);
        t.give(1, &[ANON_TOKYO]);
        t.begin_turn(1);
        t.play(1, ANON_TOKYO).unwrap();
        // The card asks which adjacent tile to link.
        t.answer_tile(1, b).unwrap();
        drain(t);
        assert!(t.on_field(1, ANON_TOKYO), "the link stand-in is placed");
        (a, b)
    };

    // (a) mortgaged partner: no quote, no bonus.
    let mut t = Table::new(&["高松灯", "千早爱音"]);
    t.strip_skills();
    t.clean();
    t.set_money(0, 50_000);
    let (a, b) = build(&mut t);
    t.set_mortgaged(b, true);
    let before = t.money(0);
    land_on(&mut t, 0, a);
    assert_eq!(
        before - t.money(0),
        300,
        "only the landed rent: {:?}",
        t.recent_keys(12)
    );

    // (b) the partner's real charge: 260 / 2 = 130, and moca (0.5 on the payer)
    // halves the whole 430 once -- the quote is not halved on its own.
    let mut t = Table::new(&["高松灯", "千早爱音"]);
    t.strip_skills();
    t.clean();
    t.set_money(0, 50_000);
    let (a, _b) = build(&mut t);
    t.place_raw(0, MOCA);
    let before = t.money(0);
    land_on(&mut t, 0, a);
    let expect = ceil10((300 + 130) / 2);
    assert_eq!(
        before - t.money(0),
        expect,
        "300 + half of 260, halved once: {:?}",
        t.recent_keys(12)
    );
    assert!(
        t.recent_keys(12)
            .iter()
            .any(|k| k.contains("anon_tokyo_link_rent")),
        "the link bonus ran: {:?}",
        t.recent_keys(12)
    );
}

// =====================================================================
// 12 -- guerrilla counts only the owner's settle payments (row 12)
// =====================================================================

// 规则书 `RAS:游击演出` [特]: 「此卡进入弃卡区的回合结束时，如果本回合的[结算]向
// 其他玩家支付了至少1000资金…则可选择将此卡置于场上」.
//
// 「本回合的[结算]向其他玩家支付」 is the owner's own ledger
// (`paid_in_settle_by`): another seat paying 1,670 into it does not revive the
// card, the owner paying 6,680 does.
#[test]
fn g02_guerrilla_counts_only_own_payments() {
    let lh = [
        tile("DUB MUSIC EXPERIMENT"),
        tile("武道馆"),
        tile("Space"),
        tile("Live House Galaxy"),
    ];
    // Arrange: all Live Houses held and mortgaged so the 「没有可盖房的 live house」
    // clauses hold.
    let arrange = |t: &mut Table| {
        t.own(0, &lh);
        for &x in &lh {
            t.set_mortgaged(x, true);
        }
        // An over-full hand lets the owner discard the card outright (limit 5).
        t.set_hand(
            0,
            &[
                GUERRILLA,
                "PP:[衍生]共鸣",
                "PP:[衍生]共鸣",
                "PP:[衍生]共鸣",
                "PP:[衍生]共鸣",
                "PP:[衍生]共鸣",
                "PP:[衍生]共鸣",
            ],
        );
        // A spare draw card keeps the discard out of a reshuffle.
        t.set_draw(0, &["PP:[衍生]共鸣"]);
    };

    // (a) the owner's own settle payment.
    let mut t = Table::new(&["高松灯", "椎名立希", "要乐奈"]);
    t.strip_skills();
    t.clean();
    arrange(&mut t);
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_houses(land, 3);
    t.begin_turn(0);
    t.discard_card(0, GUERRILLA).unwrap();
    assert!(
        t.discard(0).iter().any(|c| c == GUERRILLA),
        "the card is in the discard"
    );
    t.set_hand(0, &["PP:[衍生]共鸣"]); // back under the limit
    land_on(&mut t, 0, land); // the owner's own 6,680 settle payment
    assert_eq!(t.money(0), 10_000 - 6_680);
    t.end(0).unwrap();
    let p = t.expect_prompt();
    assert_eq!(
        p.kind,
        "choice",
        "the turn-end revive offer: {}",
        t.dump_prompt()
    );
    assert!(
        format!("{p:?}").to_lowercase().contains("guerrilla") || format!("{p:?}").contains("游击"),
        "the guerrilla revive: {}",
        t.dump_prompt()
    );

    // (b) another seat's half rent into the same turn: no revive.
    let mut t = Table::new(&["高松灯", "椎名立希", "要乐奈"]);
    t.strip_skills();
    t.clean();
    arrange(&mut t);
    let land = tile("购物中心");
    t.own(0, &[land]);
    t.set_houses(land, 3);
    let free = tile("天文馆");
    t.begin_turn(0);
    t.discard_card(0, GUERRILLA).unwrap();
    t.set_hand(0, &["PP:[衍生]共鸣"]); // back under the limit
                                       // 祥，移动 walks P1 three tiles forward onto P0's charged tile, settling.
    t.set_pos(1, (land + data().tiles.len() - 3) % data().tiles.len());
    t.give_play(0, SAKI_MOVE).unwrap();
    answer_player(&mut t, 0, 1);
    t.answer_one(0).unwrap(); // forward
    drain(&mut t);
    // P0 settles on an unowned tile (no payment of its own).
    land_on(&mut t, 0, free);
    assert!(
        t.money(0) > 10_000,
        "P1 paid the half rent into P0 (got {})",
        t.money(0)
    );
    assert_eq!(
        t.m.world().paid_in_settle_by(0),
        0,
        "P0 paid nothing in a settle"
    );
    assert!(
        t.m.world().paid_in_settle_by(1) >= 1000,
        "P1's ledger has it"
    );
    t.end(0).unwrap();
    let asked = if t.prompt().is_some() {
        t.dump_prompt()
    } else {
        String::from("<no prompt>")
    };
    assert!(
        !asked.to_lowercase().contains("guerrilla") && !asked.contains("游击"),
        "another seat's payment does not revive: {asked}"
    );
}

// =====================================================================
// 13 -- two Repaints compose on the one move (row 13)
// =====================================================================

// 规则书 `RAS:Repaint`: 「[反击] 当任意其他玩家进行移动掷骰并进入移动阶段后，打出
// 此卡…对方此次结算的支付减半」.
//
// Two copies on the same roll each halve it (x0.25); the next move is full.
#[test]
fn g02_repaint_two_halves_compose() {
    let mut t = Table::vanilla(3);
    t.strip_skills();
    t.clean();
    let land = tile("武道馆");
    // P2 owns the destination, P1 (the Repaint holder) nothing on the path, so
    // each card's X is 0 and the walk still reaches the tile.
    t.own(2, &[land]);
    t.begin_turn(0);
    t.set_pos(0, (land + data().tiles.len() - 4) % data().tiles.len());
    t.give(1, &[REPAINT, REPAINT]);
    t.dice(&[4]);
    let before = t.money(0);
    t.roll(0).unwrap();
    for _ in 0..2 {
        assert!(
            t.counteract_offered(REPAINT),
            "the roll window offers it: {}",
            t.dump_prompt()
        );
        // No drain in between: the second window is the second copy.
        t.counteract(1, REPAINT).unwrap();
    }
    drain(&mut t);
    assert_eq!(
        before - t.money(0),
        90,
        "340 x0.25 = 85, ceil10: {:?}",
        t.recent_keys(16)
    );

    // The next move is full.
    pass(&mut t, 0);
    until_turn(&mut t, 0);
    t.set_pos(0, (land + data().tiles.len() - 1) % data().tiles.len());
    t.dice(&[1]);
    let before = t.money(0);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(before - t.money(0), 340, "the next move pays full");
}

// =====================================================================
// 14 -- taki's quarter is spent by the forced settle (row 14)
// =====================================================================

// 规则书 `MyGO:（立希）想认真去做`: 「…立刻在所在格子前后2格内你选择的一个格子进行
// 一次[触发结算]，本次结算导致的所有[支付]变为原价的四分之一」.
//
// The quarter rides `NextSettle` and is consumed by the forced settle, so the
// stayer's own later landing pays full.
#[test]
fn g02_taki_quarter_stops_after_settle() {
    let mut t = Table::new(&["椎名立希", "高松灯", "要乐奈"]);
    t.strip_skills();
    t.clean();
    let land = tile("水族馆");
    t.own(1, &[land]);
    t.begin_turn(0);
    // P2 holds [停留] and stands within 2 of the tile.
    t.set_state(2, bandori_std::state_key::STAY, 1);
    t.set_pos(2, land - 2);
    let before = t.money(2);
    t.give_play(0, TAKI_SERIOUS).unwrap();
    t.answer_tile(0, land).unwrap();
    drain(&mut t);
    assert!(
        before - t.money(2) == 80,
        "the forced settle paid a quarter (of 320): keys {:?}",
        t.recent_keys(16)
    );
    // The stayer's own next landing is at full price again -- drop the [停留]
    // latch (its turn-start move gate would otherwise block the roll).
    t.set_state(2, bandori_std::state_key::STAY, 0);
    until_turn(&mut t, 2);
    t.m.world_mut().st.skip_move = false;
    t.set_pos(2, land - 1);
    t.dice(&[1]);
    let before = t.money(2);
    t.roll(2).unwrap();
    drain(&mut t);
    assert_eq!(before - t.money(2), 320, "full price on the next move");
}

// =====================================================================
// 15 -- rana's half composes with a walk factor (row 15)
// =====================================================================

// 规则书 `MyGO:（乐奈）有趣的女人`: 「由此卡效果导致[触发结算]时需支付资金减半」.
//
// `mul` (not `set`) on `NextSettle`: a walk already at 0.5 pays 0.25.
#[test]
fn g02_rana_half_composes_with_walk_factor() {
    let mut t = Table::vanilla(3);
    t.strip_skills();
    t.clean();
    let land = tile("水族馆");
    t.own(1, &[land]);
    // P1 (not the walker) plants the card with the trap armed; P0 walks over it
    // while a 0.5 walk factor is in force.
    place_on_tile(&mut t, 1, RANA_FUNNY, land);
    t.set_crystals(1, RANA_FUNNY, 5);
    t.begin_turn(0);
    t.set_pos(0, land - 2);
    t.m.world_mut().turn.plan.pay_factor = 0.5;
    t.dice(&[2]);
    t.roll(0).unwrap();
    // The trap asks: lose a 抹茶芭菲 or be stopped -- with none in hand the only
    // way on is the stop, which the fallback answers.
    let p = t.expect_prompt();
    assert_eq!(p.kind, "choice", "the trap asks: {}", t.dump_prompt());
    drain(&mut t);
    let paid = 10_000 - t.money(0);
    assert_eq!(
        paid,
        80,
        "320 x0.5 (walk) x0.5 (card) = 80: keys {:?}",
        t.recent_keys(20)
    );
}

// =====================================================================
// 16 -- fire bird's discounted payment still places (row 16)
// =====================================================================

// 规则书 `R:Fire bird`: 「支付1600资金将此卡放置在自己场地上并为其添加X（自选）个
// 奇迹水晶」.
//
// A discounted-but-settled payment counts (「支付1600」 is not 「付满1600」): with
// moca the 1600 settles at 800, the card is placed and X is asked.
#[test]
fn g02_fire_bird_discounted_payment_places() {
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t.place_raw(0, MOCA);
    let before = t.money(0);
    t.give_play(0, FIRE_BIRD).unwrap();
    // The X prompt: the first option is X = 1, so the second is X = 2.
    t.answer_one(1).unwrap();
    drain(&mut t);
    assert_eq!(
        before - t.money(0),
        800,
        "1600 halved: {:?}",
        t.recent_keys(10)
    );
    assert!(t.on_field(0, FIRE_BIRD), "the card is placed");
    assert_eq!(t.crystals(0, FIRE_BIRD), Some(2), "X crystals landed");
}

// =====================================================================
// 17 -- council refunds a cancelled settle and a payer-out (row 17)
// =====================================================================

// 规则书 `R:学生会的检查`: 「若此卡进入弃牌堆时结算获得的资金小于本卡原应拿到的
// 资金或未结算则获得发动此卡消耗的资金」.
//
// Both a cancelled settle and a payer who goes out read a non-`Settled`
// outcome, so the copy leaves the field, the placement cost comes back, the
// stop latch clears and the *next* settle collects normally.
#[test]
fn g02_council_cancelled_settle_refunds() {
    let space = tile("Space");
    let own = space - 2; // P1's own deed, the settle tile of its own turn
    let support = space - 1;
    // Arrange: P1 owns the tiles around Space and plays the check off its own
    // settle, placing it on Space.
    let arrange = |t: &mut Table| {
        t.own(1, &[own, support, space]);
        t.set_pos(1, own - 1);
        t.set_money(1, 10_000);
        t.give(1, &[COUNCIL]);
    };
    let play_check = |t: &mut Table| {
        t.begin_turn(1);
        t.dice(&[1]);
        t.roll(1).unwrap();
        assert!(
            t.counteract_offered(COUNCIL),
            "the settle window offers it: {}",
            t.dump_prompt()
        );
        t.counteract(1, COUNCIL).unwrap();
        // Yes to the placement, then Space as the tile.
        t.answer_one(0).unwrap();
        t.answer_tile(1, space).unwrap();
        drain(t);
    };

    // (a) the settle is cancelled by Parking Space.
    let mut t = Table::vanilla(3);
    t.strip_skills();
    t.clean();
    arrange(&mut t);
    place_on_tile(&mut t, 2, PARKING, space);
    play_check(&mut t);
    assert!(
        t.on_field(1, COUNCIL),
        "the copy is on Space: keys {:?}",
        t.recent_keys(20)
    );
    assert_eq!(
        t.money(1),
        10_000 - 250,
        "half the Space house cost was paid"
    );

    // P0 walks over Space: the council stops it there, parking cancels the
    // settle.
    t.set_pos(0, space - 5);
    t.begin_turn(0);
    // 6 steps: the walk has a step left after the card's tile, as `pass_tile`
    // requires (`move.remaining > 0`).
    t.dice(&[6]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), space, "the walk was stopped on the card's tile");
    assert!(
        !t.on_field(1, COUNCIL),
        "the copy left the field: keys {:?}",
        t.recent_keys(24)
    );
    assert_eq!(t.money(1), 10_000, "the placement cost came back");
    assert_eq!(t.money(0), 10_000, "a cancelled settle charges nothing");
    // The plan still carries the 0.5 shape, but a fresh move drops it
    // (`drop_stale_plan_shape`); the next settle below collects full rent.

    // The next settle collects the ordinary rent (no second refund). Space
    // itself stays cancelled by the parking card, so it is taken on P1's other
    // deed.
    pass(&mut t, 0);
    until_turn(&mut t, 2);
    assert_eq!(t.turn(), 2, "it is P2's turn");
    t.set_pos(2, own - 1);
    t.dice(&[1]);
    t.roll(2).unwrap();
    drain(&mut t);
    assert_eq!(t.money(2), 10_000 - 340, "the deed's rent is charged");
    assert_eq!(
        t.money(1),
        10_000 + 340,
        "the rent came in, no second refund"
    );

    // (b) a payer who goes out.
    let mut t = Table::vanilla(3);
    t.strip_skills();
    t.clean();
    arrange(&mut t);
    play_check(&mut t);
    t.set_pos(0, space - 5);
    t.set_money(0, 0);
    t.begin_turn(0);
    // Again a step past the tile, so the card's stop fires.
    t.dice(&[6]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert!(t.p(0).bankrupt, "the payer went out");
    assert!(!t.on_field(1, COUNCIL), "the copy left the field");
    assert_eq!(
        t.money(1),
        10_000,
        "the cost came back on the payer-out too"
    );
}

// =====================================================================
// 18 -- a normal CiRCLE settle and the 6000 (row 18)
// =====================================================================

// 规则书 `HHW:（kkr）前往笑容集结的地方！`: 「若其他玩家在该格[触发结算]则向所有者
// 支付6000资金」 -- the book prints no carve-out for a normal CiRCLE walk-in.
//
// The row pins 「a walk-in settle charges 6000; a card-forced (redirected)
// settle on CiRCLE charges nothing」 -- the engine's settle-redirect carve-out runs on
// the redirect only (the CiRCLE reward already ran there), spec §2.19.
#[test]
fn g02_kokoro_normal_settle_skips_6000() {
    // Walk-in: the 6,000 is charged on top of the tile's own reward. Take the
    // reward as a card (`land_on_card`) so the money delta is the 6,000 alone.
    let mut t = Table::vanilla(3);
    t.strip_skills();
    t.clean();
    place_on_tile(&mut t, 1, KOKORO_CIRCLE, 0);
    let before = t.money(0);
    land_on_card(&mut t, 0, 0);
    assert_eq!(before - t.money(0), 6000, "a walk-in settle charges 6000");

    // Card-forced settle on CiRCLE: `CircleNormal` already ran, so no 6,000.
    let mut t = Table::vanilla(3);
    t.strip_skills();
    t.clean();
    place_on_tile(&mut t, 1, KOKORO_CIRCLE, 0);
    t.set_state(2, bandori_std::state_key::STAY, 1);
    t.set_character_raw(0, "椎名立希");
    t.set_pos(2, 1);
    t.begin_turn(0);
    drain(&mut t);
    t.give_play(0, TAKI_SERIOUS).unwrap();
    t.answer_tile(0, 0).unwrap();
    drain(&mut t);
    assert_eq!(t.money(2), 10_000, "a card-forced settle charges nothing");
}

// =====================================================================
// 19 -- 主唱太拼命了 ignores a 分摊 leg (row 19)
// =====================================================================

// 规则书 `CRYCHIC:主唱太拼命了`: 「[反击] 一次性向其他玩家支付至少5000资金时，免除
// 此次支付」.
//
// 「一次性」 excludes a 分摊 share: the window rides the command total only, so
// the 4,000 legs never qualify and the 12,000 command does.
#[test]
#[ignore = "row 19 -- TODO(ABI): pay_total_stages raises no `effect` window, so a ChainKind::Effect counter with pay_total_stage never fires on a split command total"]
fn g02_vocal_ignores_split_legs() {
    // The payers are the other seats; the first of them (P1) holds the card.
    let mut t = Table::vanilla(4);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t.give(1, &[VOCAL]);
    let before = (t.money(0), t.money(1), t.money(2), t.money(3));
    t.give_play(0, "TEST:split12k").unwrap();
    assert!(
        t.counteract_offered(VOCAL),
        "the command total offers it: {}",
        t.dump_prompt()
    );
    t.counteract(1, VOCAL).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), before.0, "nothing reaches the receiver either");
    assert_eq!(t.money(1), before.1);
    assert_eq!(t.money(2), before.2);
    assert_eq!(t.money(3), before.3);

    // Passed, it is not re-offered on the legs (a 4,000 leg is under 5,000).
    let mut t = Table::vanilla(4);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t.give(1, &[VOCAL]);
    t.give_play(0, "TEST:split12k").unwrap();
    t.decline();
    drain(&mut t);
    assert!(
        !t.counteract_offered(VOCAL),
        "no leg-level offer: {}",
        t.dump_prompt()
    );
    assert_eq!(t.money(1), 10_000 - 4000, "the leg of 4,000 was paid");
    assert_eq!(t.money(0), 10_000 + 3 * 4000, "every leg was collected");
}

// =====================================================================
// 20 -- tritone answers a raw loss and a redeem (row 20)
// =====================================================================

// 规则书 `Mor:迷茫之蝶们的三全音`: 「[反击] 任意时刻当你将要失去或支付资金时打出此
// 卡，立刻获得此次失去的资金金额」.
//
// `lose_raw` and a redeem are raw spends: each raises the `spendBefore` window
// with the full cost, and the card gains exactly that figure. A reduction
// (`aya`) lowers the debit.
#[test]
#[ignore = "row 20 -- TODO(ABI): `ctx::adjust_money` answers inline instead of pausing, so a raw `lose_raw` never raises the `spendBefore` counter window (the redeem half works)"]
fn g02_tritone_answers_raw_loss() {
    // (a) a raw loss.
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t.give(0, &[TRITONE]);
    let before = t.money(0);
    t.give_play(0, "TEST:loseRaw").unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.kind, "choice", "the loss window: {}", t.dump_prompt());
    assert!(
        format!("{p:?}").contains("1000"),
        "the window carries the full cost: {}",
        t.dump_prompt()
    );
    t.counteract(0, TRITONE).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), before, "lost 1000 and gained it back");
    assert!(t.on_field(0, TRITONE), "the card is placed");

    // (b) a redeem with aya in play: the window is raised with the full cost and
    // the accepted reduction lowers the debit.
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.clean();
    let deed = tile("水族馆");
    t.own(0, &[deed]);
    t.set_mortgaged(deed, true);
    t.place_raw(0, AYA);
    t.give(0, &["PP:[衍生]共鸣"]);
    let cost = redeem_cost(deed);
    let before = t.money(0);
    t.redeem(0, deed).unwrap();
    // Both cards answer the same `spendBefore` window, which carries the full cost.
    let p = t.expect_prompt();
    assert!(
        t.counteract_offered(TRITONE),
        "the spend window: {}",
        t.dump_prompt()
    );
    assert!(
        format!("{p:?}").contains(&cost.to_string()),
        "full cost {cost}: {}",
        t.dump_prompt()
    );
    // Tritone gains the lost figure back ...
    t.counteract(0, TRITONE).unwrap();
    // ... and aya's 共鸣 link lowers the debit by 1500 (its body asks for the
    // fuel instead of a second [反击] offer).
    let r = t.expect_prompt();
    assert!(
        format!("{:?}", r.title).contains("resonance"),
        "the 共鸣 link: {}",
        t.dump_prompt()
    );
    t.answer_one(0).unwrap();
    drain(&mut t);
    assert!(!t.mortgaged(deed), "the deed was redeemed");
    assert_eq!(
        t.money(0) - before,
        1500,
        "the gain covered the debit and the 1500 stayed: keys {:?}",
        t.recent_keys(16)
    );
}

// =====================================================================
// 21 -- tomori covers a skill cost only (row 21)
// =====================================================================

// 规则书 `MyGO:（灯）不再迷茫` (2): 「你使用**角色技能**时可移除此卡上的一个[奇迹
// 水晶]以代替此次技能的火罐消耗」.
//
// Scope is a skill cost (`charge_is_skill`): a card-effect spend opens the
// window but is not offered; a skill cost is.
#[test]
fn g02_tomori_skill_only() {
    // (a) a card-effect spend: no offer.
    let mut t = Table::new(&["椎名立希", "高松灯"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t.place_raw(0, TOMORI);
    t.set_crystals(0, TOMORI, 2);
    t.set_fire(0, 1, 4);
    t.give_play(0, "TEST:spendFire").unwrap();
    drain(&mut t);
    assert_eq!(t.crystals(0, TOMORI), Some(2), "no crystal was traded");
    assert_eq!(t.charge(0), 0, "the pot paid instead");

    // (b) a skill cost: one crystal replaces the whole cost.
    let mut t = Table::new(&["椎名立希", "高松灯"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t.place_raw(0, TOMORI);
    t.place_raw(0, MOCA); // something on the field for 立希 to charge
    t.set_crystals(0, TOMORI, 2);
    t.set_fire(0, 2, 5);
    t.give_play(0, "skill:椎名立希:决定练习日的会议").unwrap();
    // 立希 asks which card to charge; then the cover asks.
    let p = t.expect_prompt();
    assert_eq!(p.kind, "choice", "which card: {}", t.dump_prompt());
    // Pick moca (index 1) so the skill's own crystal lands beside tomori.
    t.answer_one(1).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.kind, "choice", "the cover: {}", t.dump_prompt());
    t.answer_one(0).unwrap();
    drain(&mut t);
    assert_eq!(t.crystals(0, TOMORI), Some(1), "one crystal was traded");
    assert_eq!(t.charge(0), 2, "the pot did not pay");
}

// =====================================================================
// 22 -- hagumi counts every credit (row 22)
// =====================================================================

// 规则书 `HHW:（育美）` (2): 「当前回合内你每获得过一次资金，此卡的投掷次数+1」.
//
// The (2) is spent by another seat's turn end, so the credit has to land inside
// someone else's turn: own-turn gains are wiped by the turn-start reset. Every
// credit counts (a settle payment into the owner, a raw credit); a debit does
// not.
#[test]
fn g02_hagumi_counts_every_credit() {
    let land = tile("水族馆");
    let opened = |case: usize| -> Table {
        let mut t = Table::vanilla(3);
        t.strip_skills();
        t.clean();
        t.set_hand(1, &[HAGUMI]);
        t.begin_turn(0);
        match case {
            0 => {
                // A settle payment into the owner.
                t.own(1, &[land]);
                t.set_pos(0, land - 1);
                t.dice(&[1]);
                t.roll(0).unwrap();
                drain(&mut t);
            }
            1 => {
                // A direct credit outside the payment pipeline.
                let w = t.m.world_mut();
                w.credit(1, 500);
            }
            _ => unreachable!(),
        }
        t
    };

    for case in 0..2 {
        let mut t = opened(case);
        assert!(t.m.world().gains_this_turn(1) > 0, "case {case} credited");
        t.m.world_mut().st.skip_move = true;
        t.end(0).unwrap();
        assert!(
            t.counteract_offered(HAGUMI),
            "case {case}: the (2) is offered: {}",
            t.dump_prompt()
        );
    }

    // A debit does not bump the gain counter: 立希 forces P1 to settle on P0's
    // tile, so P1 pays out and nothing is offered.
    let mut t = Table::new(&["椎名立希", "高松灯", "要乐奈"]);
    t.strip_skills();
    t.clean();
    t.set_hand(1, &[HAGUMI]);
    t.own(0, &[tile("购物中心")]);
    t.set_state(1, bandori_std::state_key::STAY, 1);
    t.set_pos(
        1,
        (tile("购物中心") + data().tiles.len() - 2) % data().tiles.len(),
    );
    t.begin_turn(0);
    t.give_play(0, TAKI_SERIOUS).unwrap();
    t.answer_tile(0, tile("购物中心")).unwrap();
    drain(&mut t);
    assert_eq!(t.m.world().gains_this_turn(1), 0, "a debit is not a gain");
    t.m.world_mut().st.skip_move = true;
    t.end(0).unwrap();
    let d = if t.prompt().is_some() {
        t.dump_prompt()
    } else {
        String::from("<no prompt>")
    };
    assert!(
        !d.contains("育美") && !d.contains("hagumi"),
        "not offered: {d}"
    );
}

// =====================================================================
// 23 -- a settle factor does not leak into the move (row 23)
// =====================================================================

// 规则书 `R:学生会的检查`: 「因此卡强制停下的玩家的结算地租价格为原价格一半」.
//
// 「本次结算」 scoped: the half is a settle factor, so the mover's next walk is
// full again.
#[test]
fn g02_settle_set_factor_does_not_leak_to_the_move() {
    let land = tile("购物中心");
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.clean();
    t.own(1, &[land]);
    t.set_money(1, 10_000);
    t.give(1, &[COUNCIL]);
    // P1 (the owner) settles its own landing and leaves the check on the deed.
    t.set_pos(1, land + 1);
    t.begin_turn(1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    assert!(
        t.counteract_offered(COUNCIL),
        "the settle window offers it: {}",
        t.dump_prompt()
    );
    t.counteract(1, COUNCIL).unwrap();
    t.answer_one(0).unwrap();
    t.answer_tile(1, land).unwrap();
    drain(&mut t);
    assert_eq!(t.money(1), 10_000 - 1000, "half of the 2000 house cost");
    t.end(1).unwrap();
    drain(&mut t);

    // P0 passes the deed mid-walk: the check stops it and the rent is halved.
    until_turn(&mut t, 0);
    t.set_pos(0, (land + data().tiles.len() - 2) % data().tiles.len());
    t.dice(&[4]);
    let before = t.money(0);
    t.roll(0).unwrap();
    // CiRCLE lies on the way; take its card half so the money stays clean.
    while let Some(p) = t.prompt() {
        let reward = format!("{:?}", p.title).contains("pass_reward");
        let k = if reward && p.options.len() > 1 {
            1
        } else {
            p.fallback
        };
        for w in t.asked() {
            let _ = t.answer(w, k);
        }
        t.settle();
    }
    assert_eq!(t.pos(0), land, "the walk was stopped on the card's tile");
    assert_eq!(before - t.money(0), 150, "half of the 300 rent");
    t.end(0).unwrap();
    drain(&mut t);

    // The next walk is full price again.
    until_turn(&mut t, 0);
    t.set_pos(0, (land + data().tiles.len() - 1) % data().tiles.len());
    t.dice(&[1]);
    let before = t.money(0);
    t.roll(0).unwrap();
    while let Some(p) = t.prompt() {
        let reward = format!("{:?}", p.title).contains("pass_reward");
        let k = if reward && p.options.len() > 1 {
            1
        } else {
            p.fallback
        };
        for w in t.asked() {
            let _ = t.answer(w, k);
        }
        t.settle();
    }
    assert_eq!(before - t.money(0), 300, "full price again");
}

// =====================================================================
// 24 -- a paySplitAdd shape write is too late (row 24)
// =====================================================================

// 规则书 `docs/rulebook/spec/abi/G02-pay-pipeline.md` §5.5: a `set_pay_shape`
// write only lands before the first split leg, which `paySplitAdd` is behind.
//
// The write reports false and the command keeps `Normal`: the modifier stages
// still run (moca halves the 320).
#[test]
fn g02_pay_shape_too_late() {
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.clean();
    let land = tile("水族馆");
    t.own(1, &[land]);
    place_on_tile(&mut t, 0, "TEST:shapeLate", land);
    t.place_raw(0, MOCA);
    let m = t.mark();
    let before = t.money(0);
    land_on(&mut t, 0, land);
    let keys = t.keys_since(m);
    assert!(
        keys.iter().any(|k| k.contains("shape_late_done")),
        "the write ran: {keys:?}"
    );
    let wrote: Vec<i64> = t
        .events_since(m)
        .into_iter()
        .filter(|e| log_key(e).contains("shape_late_done"))
        .filter_map(|e| int_arg(&e.msg, "wrote"))
        .collect();
    assert_eq!(wrote, vec![0], "the write reports false");
    assert_eq!(
        before - t.money(0),
        160,
        "still Normal: moca halved the 320"
    );
}

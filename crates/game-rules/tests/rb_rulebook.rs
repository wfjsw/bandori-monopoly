//! Core-rulebook conformance: the 规则书 tab of the live Google Doc
//! (`docs/rulebook/rulebook-doc.md`), checked against the engine.
//!
//! Scope is the **rulebook** itself -- glossary, start/win, game flow, the
//! 时点流程 tables, payment phase, 基础[结算]规则, markers -- not card or skill
//! text (those live in the `rb_<group>` suites). Card effects override the
//! rulebook (特别注意 3), so a card that bends a clause is not a discrepancy
//! here.
//!
//! Evidence first from the existing suites (`rb_cross_*`, `rb_gap_*`,
//! `game-core`'s `play::tests`); this file only adds what those leave open,
//! plus every clause the 时点流程 / 支付阶段 tables add over `data/rules.txt`
//! (which stubs those tables out).
//!
//! Discrepancies are `#[ignore = "DISCREPANCY: …"]`, ambiguities
//! `#[ignore = "RULING: …"]`. Never weaken an assertion.

mod common;

use common::*;

/// Answer every open prompt with its decline. An auction prompt is *passed*
/// (`value < 0`), not declined -- its fallback is a bid of 0, which the engine
/// rejects as below the 100 minimum and would loop forever.
fn drain(t: &mut Table) {
    while let Some(p) = t.prompt() {
        if p.kind == "auction" {
            for who in t.asked() {
                let _ = t.m.act(
                    who as i32 + 1,
                    &game_core::net::NetMessage {
                        prompt: p.id,
                        value: -1,
                        ..game_core::net::NetMessage::act("answer")
                    },
                );
            }
            t.settle();
        } else {
            t.decline();
        }
    }
}

fn pass(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..20 {
        if t.turn() == who && t.step() == OPS {
            return;
        }
        let cur = t.turn();
        if t.turn() == who {
            // Mid-turn (e.g. left at 结束 by a previous landing): finish it.
            t.end(who).unwrap();
            drain(t);
            continue;
        }
        // A seat that went out on its own turn cannot act; nudge the turn on.
        if t.p(cur).out() {
            t.m.world_mut().next_turn_pending = true;
            t.settle();
            continue;
        }
        pass(t, cur);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

/// Advance to `who`'s 运营 stage and land them on `tile` with a one-step main
/// move from the tile before it. Prompts are left open for the caller.
fn settle_on(t: &mut Table, who: usize, tile: usize) {
    until_turn(t, who);
    t.set_pos(who, (tile + 59) % 60);
    t.dice(&[1]);
    t.roll(who).unwrap();
}

/// `give_stay` / `give_stun` set the `Tick::TurnEnd` expiry the engine ticks;
/// a bare `set_state` does not, and the layer would never wear off.
fn give_stay(t: &mut Table, who: usize, n: i32) {
    t.m.world_mut().give_stay(who as i32, n);
}

fn give_stun(t: &mut Table, who: usize, n: i32) {
    t.m.world_mut().give_stun(who as i32, n);
}

fn give_exile(t: &mut Table, who: usize, n: i32, to: usize) {
    t.m.world_mut().give_exile(who as i32, n, to as i32);
}

/// 「向上取整10」 on a half charge: `ceil(full / 2 / 10) * 10`.
fn half_ceil10(full: i32) -> i32 {
    ((full as f64 / 2.0 / 10.0).ceil() as i32) * 10
}

fn price(t: usize) -> i32 {
    data().tiles[t].price
}

fn house_cost(t: usize) -> i32 {
    data().tiles[t].house
}

// =====================================================================
// 游戏流程 -- start state, hand limit, mortgage / redeem rates
// =====================================================================

// 规则书 游戏流程 1: 「每名玩家的初始格子为1号CiRCLE，拥有10000初始资金并抽取2张手卡」.
#[test]
fn gf01_start_tile_money_and_two_cards() {
    let t = Table::new(&["美竹兰", "青叶摩卡"]);
    for who in 0..2 {
        assert_eq!(t.pos(who), tile("CiRCLE"), "P{who} starts on 1号CiRCLE");
        assert_eq!(t.money(who), 10_000, "P{who} starts with 10000");
        assert_eq!(t.hand(who).len(), 2, "P{who} opens with 2 cards");
    }
}

// 规则书 游戏流程 1: 「仅一次，可将抽取的手卡全部洗回抽卡区并重抽2张」.
// The opening deal offers exactly one mulligan; `Table::new` answers it with
// the keep fallback and no second offer comes up.
#[test]
fn gf02_mulligan_is_offered_once_then_gone() {
    let mut t = Table::new(&["美竹兰", "青叶摩卡"]);
    assert!(
        t.prompt().is_none(),
        "the mulligan offer is answered by Table::new: {}",
        t.dump_prompt()
    );
    for who in 0..2 {
        assert_eq!(t.hand(who).len(), 2, "P{who} kept the opening 2");
    }
    t.begin_turn(0);
    assert!(t.prompt().is_none(), "no second mulligan at turn 1");
}

// 规则书 游戏流程 2: 「基础手卡上限为5张，任何时刻若持有手卡超过上限则需要立刻将手卡弃至上限张」.
#[test]
fn gf03_hand_limit_is_five_and_overdraw_discards_down() {
    let mut t = Table::vanilla(2);
    assert_eq!(t.p(0).hand_limit(), 5, "base hand limit is 5");
    t.set_hand(
        0,
        &[
            "通用:GREAT!",
            "通用:PERFECT!",
            "通用:FEVER!",
            "R:[衍生] 觉悟",
            "R:[衍生] 觉悟",
            "通用:THANKS PARTY!",
        ],
    );
    assert_eq!(t.hand(0).len(), 6, "one over the limit");
    t.discard_card(0, "通用:THANKS PARTY!").unwrap();
    assert_eq!(t.hand(0).len(), 5, "discarded down to the limit");
    assert!(t.draw_pile(0).contains(&"通用:THANKS PARTY!".to_string()));
}

// 规则书 游戏流程 2: 「当抽卡区抽光时将弃卡区洗卡并放回抽卡区」.
#[test]
fn gf04_empty_draw_reshuffles_the_discard() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_draw(0, &[]);
    t.set_discard(0, &["通用:GREAT!", "通用:PERFECT!", "通用:FEVER!"]);
    // 江户川乐器店's settle draws 1; with an empty draw pile that reshuffles
    // the discard first.
    settle_on(&mut t, 0, tile("江户川乐器店"));
    drain(&mut t);
    assert_eq!(t.hand(0).len(), 1, "drew one after the reshuffle");
    assert!(t.discard(0).is_empty(), "the discard was consumed");
    assert_eq!(t.draw_pile(0).len(), 2, "the rest went back on the draw pile");
}

// 规则书 游戏流程 5 (bold): 「抵押拥有的地契，抵押后[获得]地契购买价格50%的资金」.
#[test]
fn gf05_mortgage_pays_fifty_percent_of_the_price() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(0, &[land]);
    t.set_money(0, 1_000);
    t.mortgage(0, land).unwrap();
    assert!(t.mortgaged(land), "the deed is now mortgaged");
    assert_eq!(t.money(0), 1_000 + price(land) / 2, "gained 50% of the price");
    assert_eq!(t.money(0), 1_000 + 1_500, "购物中心 is 3000 -> 1500");
}

// 规则书 游戏流程 6 (bold, 60% red): 「赎回拥有的地契，赎回后[消耗]地契购买价格60%的资金」.
#[test]
fn gf06_redeem_costs_sixty_percent_of_the_price() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(0, &[land]);
    t.set_mortgaged(land, true);
    t.set_money(0, 10_000);
    t.redeem(0, land).unwrap();
    assert!(!t.mortgaged(land), "the deed is free again");
    // 60% of 3000 = 1800.
    assert_eq!(t.money(0), 10_000 - 1_800, "paid 60% of the price");
}

// 规则书 游戏流程 8 (bold 「投掷1d20」) + 移动 5: the path excludes the start
// and includes the end. A 1d20 roll of 3 from CiRCLE walks tiles 1, 2, 3 and
// settles only the end.
#[test]
fn gf07_main_move_is_1d20_and_the_path_skips_the_start() {
    let mut t = Table::vanilla(2);
    until_turn(&mut t, 0);
    t.set_pos(0, tile("CiRCLE"));
    t.dice(&[3]);
    t.roll(0).unwrap();
    assert_eq!(t.pos(0), 3, "three steps clockwise from CiRCLE");
    // The end settled: the unowned 水族馆 announced its buy (the buy itself is
    // an end-step action, not a prompt).
    assert!(
        t.recent_keys(6).iter().any(|k| k.contains("land")),
        "the end tile settled: {:?}",
        t.recent_keys(6)
    );
    assert_eq!(t.owner(3), None, "unowned");
}

// =====================================================================
// 基础[结算]规则 -- the settle bodies
// =====================================================================

// 规则书 基础[结算] 1: 「CiRCLE和江户川乐器店的[结算]是：抽取一张手卡」.
#[test]
fn s01_landing_on_circle_draws_one_card() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_draw(0, &["通用:GREAT!", "通用:PERFECT!", "通用:FEVER!"]);
    settle_on(&mut t, 0, tile("CiRCLE"));
    drain(&mut t);
    assert_eq!(t.hand(0).len(), 1, "settle = draw 1");
}

// 规则书 基础[结算] 1: same sentence -- 江户川乐器店 draws one.
#[test]
fn s02_landing_on_edogawa_draws_one_card() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_draw(0, &["通用:GREAT!", "通用:PERFECT!", "通用:FEVER!"]);
    settle_on(&mut t, 0, tile("江户川乐器店"));
    drain(&mut t);
    assert_eq!(t.hand(0).len(), 1, "settle = draw 1");
}

// 规则书 基础[结算] 2: 「CiRCLE咖啡厅和流星堂的的[结算]是：抽取一张手卡，然后抽取一个事件卡」.
// The event does not enter the hand (2.1).
#[test]
fn s03_landing_on_cafe_draws_one_and_an_event() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_draw(0, &["通用:GREAT!", "通用:PERFECT!", "通用:FEVER!"]);
    settle_on(&mut t, 0, tile("CiRCLE 咖啡厅"));
    drain(&mut t);
    assert_eq!(t.hand(0).len(), 1, "draw 1 hand card, not the event");
}

// 规则书 基础[结算] 1.1: 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得[CiRCLE奖励]」
// and 专有名词 11: 「[CiRCLE奖励]：[获得]2000资金或抽1张卡」.
#[test]
fn s04_passing_circle_from_away_gives_the_reward() {
    let mut t = Table::vanilla(2);
    until_turn(&mut t, 0);
    t.set_pos(0, tile("CiRCLE") + 58);
    t.dice(&[2]);
    t.roll(0).unwrap();
    // The reward asks money-or-card; take the money half.
    let p = t.expect_prompt();
    assert_eq!(p.title.key(), "ask.circle.title", "{}", t.dump_prompt());
    t.answer_one(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), tile("CiRCLE"), "ended on CiRCLE");
    assert_eq!(t.money(0), 12_000, "took the 2000 half of the reward");
}

// 规则书 基础[结算] 1.1: the same sentence with 「且[移动起点]不为CiRCLE」 --
// starting ON CiRCLE must not pay the reward.
#[test]
fn s05_no_reward_when_the_move_started_on_circle() {
    // The clause needs a [经过]CiRCLE whose 移动起点 is CiRCLE. A full lap is
    // out of reach of a 1d20, but 「本回合的主要移动设为移动60格子」 is the
    // board's length: the walk leaves CiRCLE and [经过]s it again at the wrap,
    // with 移动起点 == CiRCLE. (行动阶段 12 「移动起点不触发[经过],移动终点
    // 触发[经过]」 -- the start is not itself a pass; the wrap is.)
    let mut t = Table::vanilla(2);
    until_turn(&mut t, 0);
    t.set_pos(0, tile("CiRCLE"));
    t.set_pos(1, 20);
    let before = t.money(0);
    let mark = t.mark();
    t.give_play(0, "PPP:向着未来的路标").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), tile("CiRCLE"), "60 tiles is a full lap");
    let keys = t.keys_since(mark);
    // The wrap [经过]'d CiRCLE -- the reward step was reached and declined.
    assert!(
        keys.iter().any(|k| k.contains("pass") || k.contains("circle") || k.contains("land")),
        "the wrap passed CiRCLE: {keys:?}"
    );
    assert_eq!(
        t.money(0),
        before,
        "no CiRCLE reward: the move started on CiRCLE ({keys:?})"
    );
}

// 专有名词 11 「[CiRCLE奖励]：[获得]2000资金或抽1张卡」 with 回合階段&註釋 `C44`
// 「拥有眩晕时无法进行支付和收款（仍可被指定），不可打出手牌」: the money half
// is blocked, so the reward is forced to the card. (`团技能` `D13` is the
// parallel that decides the card half for a hand-flood.)
#[test]
fn s05b_stunned_pass_takes_the_card_half() {
    let mut t = Table::vanilla(2);
    until_turn(&mut t, 0);
    t.set_pos(0, tile("CiRCLE") + 58);
    t.set_pos(1, 20);
    // A [晕眩] that landed mid-turn (a counter's stun) -- the reward's
    // 「无法收付款」 check reads the status at payout time.
    give_stun(&mut t, 0, 1);
    t.set_draw(0, &["通用:GREAT", "通用:GREAT"]);
    let before = t.money(0);
    let hand = t.hand(0).len();
    t.dice(&[2]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), tile("CiRCLE"), "ended on CiRCLE");
    assert_eq!(
        t.money(0),
        before,
        "the money half is blocked while stunned: {}",
        t.dump_prompt()
    );
    // Two draws: the reward's card half (forced) plus the landing's own
    // 「[结算]是：抽取一张手卡」.
    assert_eq!(
        t.hand(0).len(),
        hand + 2,
        "the reward was forced to the card half: {:?}",
        t.hand(0)
    );
}

// 规则书 基础[结算] 3 (bold): 「无主的[可购买格子]的[结算]是：可选择[消耗]购买格子地契
// 和建造已有房子的资金总价，获得格子地契和拥有权」.
#[test]
fn s06_unowned_buy_price_includes_the_houses() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.set_houses(land, 2);
    t.set_money(0, 10_000);
    settle_on(&mut t, 0, land);
    // The buy is the end step's offer (「[主要移动]和所需[结算]完成后进入结束阶段」).
    t.buy(0).unwrap();
    drain(&mut t);
    assert_eq!(t.owner(land), Some(0), "deed and ownership gained");
    assert_eq!(
        t.money(0),
        10_000 - (price(land) + 2 * house_cost(land)),
        "paid the deed plus the houses"
    );
}

// 规则书 基础[结算] 5.2: 「如果格子地契已抵押则无效果」.
#[test]
fn s07_own_mortgaged_tile_settles_to_nothing() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(0, &[land]);
    t.set_mortgaged(land, true);
    t.set_money(0, 10_000);
    settle_on(&mut t, 0, land);
    assert!(
        t.prompt().is_none(),
        "no build prompt on a mortgaged deed: {}",
        t.dump_prompt()
    );
    assert_eq!(t.st().build_cost, -1, "nothing buildable");
    assert_eq!(t.houses(land), 0, "no house was built");
    assert_eq!(t.money(0), 10_000, "nothing was spent");
}

// 规则书 基础[结算] 5.1 (bold): 「可选择[消耗]格子地契所标注的房屋建筑费进行升级建造，
// 每块地有标注的等级上限（例：所有RiNG不可升级，购物中心最大等级为三栋房屋）」.
#[test]
fn s08_build_costs_the_house_and_respects_the_cap() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    let ring = tile("RiNG 1");
    t.own(0, &[land, ring]);
    t.set_money(0, 100_000);
    // RiNG: buildMax 0 -- 「所有RiNG不可升级」.
    settle_on(&mut t, 0, ring);
    assert!(
        t.prompt().is_none(),
        "RiNG takes no house: {}",
        t.dump_prompt()
    );
    assert_eq!(t.st().build_cost, -1, "RiNG cannot be built on");
    assert_eq!(t.houses(ring), 0);
    // 购物中心: up to 3 houses, each at the house cost.
    for n in 1..=3 {
        settle_on(&mut t, 0, land);
        t.build(0).unwrap();
        drain(&mut t);
        assert_eq!(t.houses(land), n, "house {n} up");
    }
    assert_eq!(
        t.money(0),
        100_000 - 3 * house_cost(land),
        "three houses at the build cost"
    );
    // Cap reached: the fourth build is refused.
    settle_on(&mut t, 0, land);
    assert!(t.build(0).is_err() || t.houses(land) == 3, "购物中心 caps at 3");
    assert_eq!(t.houses(land), 3);
}

// 规则书 基础[结算] 5.1: 「升级所[消耗]的资金不可通过[抵押]正在升级的格子地契获得」.
// Emergent in the engine: a mortgaged deed refuses the build outright, so the
// deed being upgraded can never fund its own upgrade.
#[test]
fn s09_upgrade_cannot_be_funded_by_mortgaging_that_deed() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(0, &[land]);
    t.set_money(0, 100); // far below the 2000 build cost
    settle_on(&mut t, 0, land);
    assert!(t.build(0).is_err(), "no build at 100 money");
    t.mortgage(0, land).unwrap();
    assert!(t.mortgaged(land));
    settle_on(&mut t, 0, land);
    assert!(t.build(0).is_err(), "a mortgaged deed refuses the build");
    assert_eq!(t.houses(land), 0, "the upgrade never happened");
}

// 规则书 基础[结算] 6.2 (bold, red): 「[支付]…购买格子地契和建造已有房子的资金总价的两倍，
// 从该玩家处强行购买该格地契，获得的地契仍为抵押状态」.
#[test]
fn s10_force_buy_is_double_the_total_and_stays_mortgaged() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_houses(land, 1);
    t.set_mortgaged(land, true);
    t.set_money(0, 10_000);
    settle_on(&mut t, 0, land);
    let p = t.expect_prompt();
    assert_eq!(p.kind, "choice", "a force-buy prompt: {}", t.dump_prompt());
    t.answer_one(0).unwrap();
    drain(&mut t);
    let want = 2 * (price(land) + house_cost(land));
    assert_eq!(t.owner(land), Some(0), "deed changed hands");
    assert!(t.mortgaged(land), "「获得的地契仍为抵押状态」");
    assert_eq!(t.money(0), 10_000 - want, "paid 2x (deed + houses)");
    assert_eq!(t.money(1), 10_000 + want, "the owner received it in full");
}

// 规则书 基础[结算] 6.2: 「此次购买的价格不受任何资金变动效果影响」.
// O6 says the same for 抵押 / 赎回 / 强制购买. The engine moves the force-buy
// sum outside the pay pipeline entirely, so a payMul cannot touch it.
#[test]
fn s11_force_buy_ignores_a_pay_multiplier() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_mortgaged(land, true);
    t.m.world_mut().turn.plan.pay_factor = 0.5;
    t.set_money(0, 10_000);
    settle_on(&mut t, 0, land);
    t.answer_one(0).unwrap();
    drain(&mut t);
    let want = 2 * price(land);
    assert_eq!(t.money(0), 10_000 - want, "full double, not halved");
}

// 规则书 基础[结算] 7: 「半价收费的[结算]（向上取整10）」.
#[test]
fn s12_agent_half_charge_rounds_up_to_ten() {
    let mut t = Table::vanilla(2);
    let agent = tile("主要街道"); // group 1
    let group: Vec<usize> = (0..data().tiles.len())
        .filter(|&x| data().tiles[x].is_buyable() && data().tiles[x].group == 1)
        .collect();
    for &x in &group {
        t.own(1, &[x]);
    }
    t.set_money(0, 10_000);
    settle_on(&mut t, 0, agent);
    drain(&mut t);
    // Every same-colour purchasable charges half rent, each rounded up to 10.
    let mut want = 0;
    for &x in &group {
        want += half_ceil10(data().tiles[x].rent[0]);
    }
    assert_eq!(t.money(0), 10_000 - want, "half-rent per tile, ceil 10");
    // The rounding is 「向上取整10」: a half of 150.5 charges 160, not 150.
    assert_eq!(half_ceil10(301), 160, "301/2 = 150.5 -> 160");
    assert_eq!(half_ceil10(300), 150, "300/2 = 150 is already a multiple");
}

// =====================================================================
// 时点流程 -- the 19 action windows that `data/rules.txt` stubs out
// =====================================================================

// 行动阶段 4: 「眩晕效果发动：跳过经营和主要移动阶段」.
#[test]
fn t01_stun_skips_operate_and_main_move() {
    let mut t = Table::vanilla(2);
    give_stun(&mut t, 0, 1);
    t.begin_turn(0);
    // The turn auto-ends: no 运营, no roll.
    assert!(t.prompt().is_none(), "nothing to do while stunned");
    assert_ne!(t.turn(), 0, "the stunned turn was skipped on");
    assert_eq!(t.state(0, "stun"), 0, "window 18 dropped a layer at 结束阶段");
}

// 行动阶段 7: 「停留效果发动：跳过主要移动阶段」.
#[test]
fn t02_stay_skips_the_main_move() {
    let mut t = Table::vanilla(2);
    give_stay(&mut t, 0, 1);
    t.begin_turn(0);
    assert_eq!(t.step(), OPS, "reached 运营 with the move skipped");
    assert!(t.m.world().st.skip_move, "the main move is marked skipped");
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(t.state(0, "stay"), 0, "window 18 dropped the stay at 结束阶段");
}

/// `stage::OPS` = 2 (state.rs) -- the 运营阶段.
const OPS: i32 = 2;

// 行动阶段 1 / 18: 「负面效果：移除一层[除外]」 at 回合开始前, and
// 「移除一层[眩晕]/[停留]」 at 结束阶段.
#[test]
fn t03_exile_drops_at_the_turn_start() {
    let mut t = Table::vanilla(3);
    give_exile(&mut t, 0, 2, tile("CiRCLE"));
    // The turn end does not touch 除外.
    pass(&mut t, 0);
    assert_eq!(t.state(0, "exile"), 2, "unchanged at the turn end");
    // P0's next turn start drops one layer (and the turn is skipped while any
    // remain).
    t.begin_turn(0);
    assert_eq!(t.state(0, "exile"), 1, "除外 drops one layer at 回合开始前");
    assert_ne!(t.turn(), 0, "the exiled turn was skipped on");
    // The following turn start drops the last layer.
    t.begin_turn(0);
    assert_eq!(t.state(0, "exile"), 0, "the second start drops the last layer");
}

// 行动阶段 1: when the last 除外 layer goes, 「获得该状态的效果会写明状态结束后
// 在玩家回合开始时要[传送]到的格子」.
#[test]
fn t04_exile_ends_by_teleporting_to_the_written_tile() {
    let mut t = Table::vanilla(2);
    give_exile(&mut t, 0, 1, tile("购物中心"));
    t.set_pos(0, 20); // off somewhere else while exiled
    t.begin_turn(0);
    assert_eq!(t.state(0, "exile"), 0);
    assert_eq!(t.pos(0), tile("购物中心"), "teleported to the written tile");
}

// 行动阶段 12: 「移动起点不触发[经过],移动终点触发[经过]」 and 移动 5: the path
// excludes the start and includes the end.
#[test]
fn t05_pass_fires_per_step_and_never_for_the_start() {
    let mut t = Table::vanilla(2);
    until_turn(&mut t, 0);
    t.set_pos(0, 10);
    t.dice(&[3]);
    t.roll(0).unwrap();
    assert_eq!(t.pos(0), 13, "walked 11, 12, 13");
    // The end settled -- so the end is in the path.
    assert!(
        t.recent_keys(6).iter().any(|k| k.contains("land")),
        "the end tile settled: {:?}",
        t.recent_keys(6)
    );
    // The start (10) is not in the path: no land event for it.
}

// 行动阶段 13: 「移动后/[重叠]/」 -- 「移动终点触发[重叠]」 and
// 「原地[传送]/移动(移动0格)时触发[重叠]」. (要乐奈 (3) 「你与其他玩家重合时」
// is the table's example.)
#[test]
fn t06_overlap_fires_after_a_walk_onto_a_shared_tile() {
    let mut t = Table::vanilla(2);
    until_turn(&mut t, 0);
    t.set_pos(0, 10);
    t.set_pos(1, 13);
    t.dice(&[3]);
    let mark = t.mark();
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 13, "P0 ended on P1's tile");
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k.contains("passPlayer") || k.contains("overlap")),
        "the [重叠] window fired on the shared end tile: {keys:?}"
    );
}

// 回合階段&註釋 `B41` 「传送：空降至目标地格并在该格依次触发[经过],[重叠],和[结算]」
// and `E14` 「原地[传送]/移动(移动0格)时触发[重叠]」: a teleport fires [经过]
// then [重叠] then [结算] at its target **including onto the tile it started
// from** (the old 「原地，不算[经过]」 carve-out is wrong).
//
// The same-tile case is `通用:@Tsugu ycm`'s 「[传送]到"bandori车站"并[结算]」
// with the piece already on Bandori车站. [经过] (`passTile`) is the raise
// between the teleport's log and the [重叠]; it is not itself a log line (same
// as a walk's per-step [经过]), so the observable tail -- [重叠] then [结算],
// in that order -- is what is pinned here.
#[test]
fn t06b_same_tile_teleport_fires_overlap_then_settle() {
    let mut t = Table::vanilla(2);
    until_turn(&mut t, 0);
    let b = tile("Bandori车站");
    t.set_pos(0, b);
    t.set_pos(1, b);
    // P1 owns the tile, so the [结算] is a rent -- which logs, unlike the
    // unowned announce/offer split (`land_at_built_in`'s `main` branch).
    t.own(1, &[b]);
    // 3d10 sum 3 (< 26): the choice, take the 「传送…并[结算]」 half.
    t.dice(&[1, 1, 1]);
    let mark = t.mark();
    t.give_play(0, "通用:@Tsugu ycm").unwrap();
    t.answer(0, 1).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), b, "原地[传送]: the piece never left ({})", t.dump_prompt());
    let keys = t.keys_since(mark);
    let overlap = keys.iter().position(|k| k.contains("overlap"));
    let settle = keys
        .iter()
        .position(|k| k.contains("pay") || k.contains("rent"));
    assert!(
        overlap.is_some(),
        "the [重叠] window fired on the same tile: {keys:?}"
    );
    assert!(settle.is_some(), "the [结算] paid the rent: {keys:?}");
    assert!(
        overlap < settle,
        "B41 order: [重叠] before [结算]: {keys:?}"
    );
    assert!(t.money(0) < 10_000, "the rent moved: {}", t.money(0));
}

// 行动阶段 13 (`E14`) 「原地[传送]/移动(移动0格)时触发[重叠]」: a move of 0
// steps still raises [重叠] at the tile the mover stands on.
#[test]
fn t06c_zero_step_move_fires_overlap() {
    // 都筑诗船 (2) 「尽力了吗」 is the suite's 0-move: d20 − d6 = 0 settles
    // in place (see `rb_general::tsugu_skill_zero_move_settles_in_place`).
    let mut t = Table::new(&["都筑诗船", "户山香澄"]);
    t.clean();
    t.set_fire(0, 1, 2);
    t.begin_turn(0);
    let sid = t.skill_id(0, "尽力了吗");
    t.skill(0, &sid).unwrap();
    t.answer(0, 0).unwrap(); // partner P1
    t.set_pos(0, 10);
    t.set_pos(1, 10);
    t.dice(&[3, 3]); // d20=3, −d6=3 → 0 steps
    let mark = t.mark();
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 10, "0-step move stayed put");
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k.contains("passPlayer") || k.contains("overlap")),
        "the [重叠] window fired on a 0-step move: {keys:?}"
    );
    // TODO(规则书): `E14` names only [重叠] for the 0-step case. Whether [经过]
    // (`passTile`) also fires there is not stated -- `B41`+`E13` give the
    // general rule for a *teleport* and read as both-fire, but `E14` is the
    // only cell that names the 0-move and it names only [重叠]. Implemented
    // as `E14` writes it: [重叠] only.
}

// 行动阶段 7: 「一回合只能触发一次［主要移动］效果,多次触发［主要移动］时无效」.
#[test]
fn t07_only_one_main_move_per_turn() {
    let mut t = Table::vanilla(2);
    until_turn(&mut t, 0);
    t.set_pos(0, 10);
    t.dice(&[3, 5]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 13);
    // A second roll in the same turn is refused (the move is spent).
    let r = t.roll(0);
    assert!(r.is_err(), "the second main move is refused: {r:?}");
    assert_eq!(t.pos(0), 13, "no further movement");
}

// =====================================================================
// 专有名词 -- statuses
// =====================================================================

// 移动 11: 「[停留]…重复受到该效果时可叠加，玩家的每回合结束时移除一层」.
#[test]
fn m02_stay_stacks_and_one_layer_drops_per_turn_end() {
    let mut t = Table::vanilla(2);
    give_stay(&mut t, 0, 3);
    t.begin_turn(0);
    assert!(t.m.world().st.skip_move, "still stayed");
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(t.state(0, "stay"), 2, "one layer per 结束阶段");
    t.begin_turn(0);
    assert!(t.m.world().st.skip_move, "still stayed");
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(t.state(0, "stay"), 1, "one layer per 结束阶段");
}

// 移动 12: 「[晕眩]…重复受到该效果时可叠加，玩家的每回合结束时移除一层」.
#[test]
fn m03_stun_stacks_and_one_layer_drops_per_turn_end() {
    let mut t = Table::vanilla(2);
    give_stun(&mut t, 0, 2);
    t.begin_turn(0);
    // Window 4: stunned -> straight to 结束, dropping one layer.
    assert_ne!(t.turn(), 0, "the turn was skipped");
    assert_eq!(t.state(0, "stun"), 1, "one layer per 结束阶段");
}

// 移动 10: 「[无法移动]…无法使用非[传送]的移动效果」. A [停留]-ing player's
// main move is skipped entirely (window 7).
#[test]
fn m04_stay_blocks_the_main_move() {
    let mut t = Table::vanilla(2);
    give_stay(&mut t, 0, 1);
    t.begin_turn(0);
    assert!(t.m.world().st.skip_move, "the walk is skipped");
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 0, "never moved");
}

// =====================================================================
// 游戏流程 9 -- bankruptcy and the leftover auction
// =====================================================================

// 规则书 专有名词 6 [破产]: 「当玩家无法支付某笔支出时（包括抵押），将所有可折现的
// 资产折现并将所有资金[消耗]或[支付]给导致破产的效果对象」.
#[test]
fn b01_bankruptcy_cash_in_and_pays_the_creditor() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_houses(land, 3);
    t.set_money(0, 7);
    // Nothing to mortgage: straight to bankruptcy on the rent.
    settle_on(&mut t, 0, land);
    drain(&mut t);
    assert!(t.p(0).bankrupt, "P0 is out");
    assert_eq!(t.money(0), 0, "every fund went to the creditor");
    assert!(
        t.money(1) >= 10_007,
        "the creditor received the funds (got {})",
        t.money(1)
    );
}

// 规则书 游戏流程 9.2 (bold, red): 「其拥有的[可购买格子]变成无主状态并抽取3张
// 其中格子的地契进行[拍卖]（少于3则全部）」.
#[test]
fn b02_bankruptcy_frees_the_deeds_and_auctions_three() {
    let mut t = Table::vanilla(3);
    let rent_land = tile("购物中心");
    t.own(2, &[rent_land]);
    t.set_houses(rent_land, 3);
    // P0 holds four deeds and cannot pay.
    let mine = [
        tile("天文馆"),
        tile("水族馆"),
        tile("富士见坂"),
        tile("成为人类桥"),
    ];
    t.own(0, &mine);
    t.set_money(0, 0);
    settle_on(&mut t, 0, rent_land);
    drain(&mut t);
    assert!(t.p(0).bankrupt);
    for &x in &mine {
        assert_eq!(
            t.owner(x),
            None,
            "deed {} is ownerless",
            data().tiles[x].name
        );
    }
    // The leftover auction ran (up to 3 of the four).
    let auctions: Vec<_> = t
        .keys_since(0)
        .into_iter()
        .filter(|k| k.contains("auction"))
        .collect();
    assert!(!auctions.is_empty(), "the leftover auction ran: {auctions:?}");
}

// 规则书 专有名词 10 [拍卖]: 「最低喊价金额为100，最低单次太高喊价金额为100」.
#[test]
fn b03_auction_min_bid_and_increment_are_100() {
    let mut t = Table::vanilla(3);
    let land = tile("购物中心");
    t.own(2, &[land]);
    t.set_houses(land, 3);
    let mine = tile("天文馆");
    t.own(0, &[mine]);
    t.set_mortgaged(mine, true); // nothing left to raise with
    t.set_money(0, 0);
    settle_on(&mut t, 0, land);
    // The leftover auction prompt is open -- do not drain it away.
    let p = t.expect_prompt();
    assert_eq!(p.kind, "auction", "a live auction: {}", t.dump_prompt());
    // Bidders are the surviving seats (1 and 2); P0 is out.
    let bidder = t.asked()[0];
    // A bid under 100 is refused.
    let r = t.answer(bidder, 50);
    assert!(r.is_err(), "a 50 bid is below the minimum: {r:?}");
    // 100 is the floor.
    assert!(t.answer(bidder, 100).is_ok(), "100 is the minimum bid");
    // And the next bid must be at least 200 (「最低单次太高喊价金额为100」).
    let other = t.asked().into_iter().find(|&s| s != bidder).unwrap();
    let r = t.answer(other, 150);
    assert!(r.is_err(), "150 is below the 100 increment: {r:?}");
    assert!(t.answer(other, 200).is_ok(), "200 = 100 + the 100 increment");
}

// 规则书 正规模式 4: 「仅剩一名[存活]玩家时游戏结束，那名玩家获胜」.
#[test]
fn w01_last_alive_wins() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(1, &[land]);
    t.set_houses(land, 3);
    t.set_money(0, 0);
    settle_on(&mut t, 0, land);
    drain(&mut t);
    let st = t.st();
    assert_eq!(st.phase, "ended", "the match ended");
    assert_eq!(st.end_reason.as_str(), "last", "last-alive end");
    assert_eq!(st.winner, 1, "the survivor wins");
}

// =====================================================================
// 其他规则注意事项 -- O6, and the payment-phase stage order
// =====================================================================

// 规则书 其他规则注意事项 6: 「抵押，赎回，和强制购买的资金变动不受任何效果影响，
// 除非效果写明改动抵押，赎回，或强制购买」.
#[test]
fn o06_mortgage_and_redeem_ignore_pay_modifiers() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.own(0, &[land]);
    t.set_money(0, 10_000);
    // A payMul halver armed for the turn must not touch the mortgage proceeds.
    t.m.world_mut().turn.plan.pay_factor = 0.5;
    t.mortgage(0, land).unwrap();
    assert_eq!(
        t.money(0),
        10_000 + price(land) / 2,
        "the mortgage paid full 50%, not 25%"
    );
    // And the redeem cost is the full 60%, not halved.
    t.m.world_mut().turn.plan.pay_factor = 0.5;
    t.redeem(0, land).unwrap();
    assert_eq!(
        t.money(0),
        10_000 + price(land) / 2 - (price(land) as f64 * 0.6).round_ties_even() as i32,
        "the redeem cost was not halved"
    );
}

// 支付阶段 4: 「支付前 支付减半/翻倍」 -- the halver runs on the payment before
// the money moves. (The add-then-multiply *order* of windows 2 and 4 is the
// open RULING pinned by `rb_gap_money::g02_half_speed_plus_expectations_floor_on_a_rent`.)
#[test]
fn p01_pay_mul_halves_a_settle_rent() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心"); // rent 300
    t.own(1, &[land]);
    t.set_money(0, 10_000);
    t.m.world_mut().turn.plan.pay_factor = 0.5;
    settle_on(&mut t, 0, land);
    drain(&mut t);
    assert_eq!(t.money(0), 10_000 - 150, "300 halved at 支付前");
}
// 规则书 游戏流程 9 -- bankruptcy retires the seat's effects (PIPELINE-AUDIT B2)
// =====================================================================

// 规则书 游戏流程 9.1 (red): 「将其控制的所有棋子，角色卡，乐队卡，和手卡移出
// 游戏。所有其正在生效的卡，技能效果停止生效。」 A bankrupt seat's placed cards
// must leave the field and stop answering -- before the fix they kept running
// buy / pay hooks for the rest of the match.
#[test]
fn bankruptcy_stops_field_effects() {
    let mut t = Table::vanilla(3);
    let rent_land = tile("购物中心");
    t.own(2, &[rent_land]);
    t.set_houses(rent_land, 3);
    // P1 holds a `payAdd` probe that would boost **every** payment by 100.
    t.place_raw(1, "TEST:payAddAny");
    assert!(!t.field(1).is_empty(), "the probe is on P1's field");
    // P1 goes under on the rent (0 cash, nothing raiseable).
    t.set_money(1, 0);
    settle_on(&mut t, 1, rent_land);
    drain(&mut t);
    assert!(t.p(1).bankrupt, "P1 is out");
    assert!(
        t.field(1).is_empty(),
        "the bankrupt seat's field is empty: {:?}",
        t.field_ids(1)
    );
    // P0 then gains money: the dead seat's probe must not boost it.
    until_turn(&mut t, 0);
    let before = t.money(0);
    let mark = t.mark();
    t.give_play(0, "通用:GREAT").unwrap();
    drain(&mut t);
    assert_eq!(
        t.money(0),
        before + 2_000,
        "no boost from the dead seat (events {:?})",
        t.keys_since(mark)
    );
}

// 规则书 游戏流程 9 + 专有名词 6 -- the seat is dead before `bankruptBefore`
// (PIPELINE-AUDIT B3 / BANKRUPTCY K2)
// =====================================================================

// 规则书 专有名词 6 [破产]: the state is entered when the payment fails, and
// 游戏流程 9.1 「所有其正在生效的卡，技能效果停止生效」 is what entering it does;
// 专有名词 5 [存活] excludes 破产. So the seat is marked out **before** the
// `bankruptBefore` window, and a hook in that window cannot move their money.
#[test]
fn bankrupt_before_sees_a_dead_player() {
    let mut t = Table::vanilla(3);
    let rent_land = tile("购物中心");
    t.own(2, &[rent_land]);
    t.set_houses(rent_land, 3);
    // A `bankruptBefore` probe on P1: it tries to pull 1 from the dying seat.
    t.place_raw(1, "TEST:deadPay");
    let mark = t.mark();
    // P0 goes under on the rent (0 cash, nothing raiseable).
    t.set_money(0, 0);
    settle_on(&mut t, 0, rent_land);
    drain(&mut t);
    assert!(t.p(0).bankrupt, "P0 is out");
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k.contains("dead_pay")),
        "the probe ran in the window: {keys:?}"
    );
    // The dying seat was already out, so the transfer was refused.
    assert_eq!(
        t.money(1),
        10_000,
        "the probe could not move the dead seat's money"
    );
}

// 规则书 专有名词 10 [拍卖] + 游戏流程 5 -- a short bidder may 抵押 to fund the
// bid (PIPELINE-AUDIT B4 / BANKRUPTCY K10b)
// =====================================================================

// 规则书 游戏流程 5 (bold): 「需[支付]或[消耗]资金且资金不足时可以选择抵押拥有的
// 地契」, and 专有名词 10 [拍卖] 「[消耗]同等资金并获得地契」. A winning bidder
// short on cash is offered the 抵抵押 path to fund the bid; only a bidder who
// cannot raise it (and therefore goes out) voids the auction.
#[test]
fn auction_winner_may_mortgage_to_fund_the_bid() {
    let mut t = Table::vanilla(3);
    let rent_land = tile("购物中心");
    t.own(2, &[rent_land]);
    t.set_houses(rent_land, 3);
    // P0 is going under: four deeds, nothing raiseable.
    let mine = [
        tile("天文馆"),
        tile("水族馆"),
        tile("富士见坂"),
        tile("成为人类桥"),
    ];
    t.own(0, &mine);
    for &x in &mine {
        t.set_mortgaged(x, true);
    }
    t.set_money(0, 0);
    // P1 is the bidder: 50 cash and one mortgageable deed (小豆岛, 抵押 500).
    let fund = tile("小豆岛");
    t.own(1, &[fund]);
    t.set_money(1, 50);
    settle_on(&mut t, 0, rent_land);
    // The leftover auction prompt is open -- do not drain it away.
    let p = t.expect_prompt();
    assert_eq!(p.kind, "auction", "a live auction: {}", t.dump_prompt());
    assert!(
        t.asked().contains(&1),
        "P1 is a bidder: {:?}",
        t.asked()
    );
    // P1 bids 100 even though they hold 50 -- 抵押 funds the difference.
    assert!(t.answer(1, 100).is_ok(), "the short bid is accepted");
    // P2 passes; the auction closes on P1's 100. (`value < 0` is a pass.)
    if t.prompt().is_some() {
        for who in t.asked() {
            let _ = t.m.act(
                who as i32 + 1,
                &game_core::net::NetMessage {
                    prompt: t.expect_prompt().id,
                    value: -1,
                    ..game_core::net::NetMessage::act("answer")
                },
            );
        }
        t.settle();
    }
    // The mortgage offer funds the bid.
    let mp = t.prompt();
    assert_eq!(
        mp.as_ref().map(|p| p.kind.as_str()),
        Some("mortgage"),
        "the bid is funded by 抵押: {}",
        t.dump_prompt()
    );
    t.answer_items(1, &[&fund.to_string()]).unwrap();
    drain(&mut t);
    // The pool is shuffled and capped at 3 of the four, so assert on any of
    // them landing with P1 rather than on one specific deed.
    let won: Vec<usize> = mine
        .iter()
        .copied()
        .filter(|&x| t.owner(x) == Some(1))
        .collect();
    assert_eq!(
        won.len(),
        1,
        "exactly one auctioned deed is awarded to the bidder: owners {:?}",
        mine.iter().map(|&x| (data().tiles[x].name.clone(), t.owner(x))).collect::<Vec<_>>()
    );
    assert!(t.mortgaged(fund), "the funding deed is mortgaged");
    // 50 + 500 (抵押) - 100 (bid) = 450.
    assert_eq!(t.money(1), 450, "the bid was funded and paid");
}

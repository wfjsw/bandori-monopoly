//! Connected tiles, their effects and counter-effects: §3 `T*` of
//! `docs/rulebook/CROSS-TESTS.md`.

mod common;

use common::*;
use game_core::engine::CardRules;
use game_core::msg::{Arg, Msg};

const FILL: &str = "R:[衍生] 觉悟";

/// Place `card` on `who`'s field at board `tile` (a sheet 「置于格子上」 card).
fn place_on_tile(t: &mut Table, who: usize, card: &str, tile: usize) {
    let d = data();
    let props = rules().card_props(card);
    t.m.world_mut().place_card_on(
        &d,
        who as i32,
        tile as i32,
        card,
        Msg::default(),
        props,
    );
}

fn set_draw_n(t: &mut Table, who: usize, n: usize) {
    let cards: Vec<&str> = vec![FILL; n];
    t.set_draw(who, &cards);
}

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
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

fn price(t: usize) -> i32 {
    data().tiles[t].price
}

fn house_cost(t: usize) -> i32 {
    data().tiles[t].house
}

fn rent(t: usize, h: usize) -> i32 {
    let tile = &data().tiles[t];
    tile.rent[(h).min(tile.rent.len().saturating_sub(1))]
}

// =====================================================================
// T1. Anon Tokyo links two tiles
// =====================================================================

// 规则书: Anon Tokyo: 「消耗其中地契购买价格中更高者的资金的一半…放置1个[奇迹水晶]（上限1）…
// 被连接的格子收费时，会额外收取被连接的其他格子收费的一半」.
#[test]
fn t01_anon_tokyo_link() {
    let mut t = Table::new(&["千早爱音", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    let t1 = tile("购物中心");
    let t2 = tile("天文馆");
    t.own(0, &[t1, t2]);
    t.set_pos(0, t1);
    t.give_play(0, "MyGO:[千早爱音]Anon Tokyo").unwrap();
    drain(&mut t);
    let half = price(t1).max(price(t2)) / 2;
    assert_eq!(t.money(0), 10_000 - half, "spends max(P1,P2)/2");
    // P1 lands on 1 and pays R(1,0) + R(2,0)/2.
    until_turn(&mut t, 1);
    t.set_pos(1, t1 - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    let expected = rent(t1, 0) + rent(t2, 0) / 2;
    assert_eq!(t.money(1), 10_000 - expected, "P1 pays {expected}");
    // A second Anon Tokyo on the same pair adds no second link (上限1).
    t.give(0, &["MyGO:[千早爱音]Anon Tokyo"]);
    t.set_pos(0, t1);
    let before = t.crystals(0, "MyGO:[千早爱音]Anon Tokyo");
    if t.play(0, "MyGO:[千早爱音]Anon Tokyo").is_ok() {
        drain(&mut t);
    }
    assert_eq!(
        t.crystals(0, "MyGO:[千早爱音]Anon Tokyo"),
        before,
        "no second link"
    );
}

// =====================================================================
// T2. Anon link plus Fire bird
// =====================================================================

// 规则书: Fire bird: 「自己的所有格子收费变成1.5倍」.
// RULING: whether the linked half uses the base rent or the boosted rent.
#[ignore = "DISCREPANCY: Anon Tokyo link + Fire bird arrangement"]
#[test]
fn t02_anon_link_plus_fire_bird() {
    let mut t = Table::new(&["千早爱音", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    let t1 = tile("购物中心");
    let t2 = tile("天文馆");
    t.own(0, &[t1, t2]);
    t.set_pos(0, t1);
    t.place_raw(0, "R:Fire bird");
    t.give_play(0, "MyGO:[千早爱音]Anon Tokyo").unwrap();
    drain(&mut t);
    until_turn(&mut t, 1);
    t.set_pos(1, t1 - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    let base = rent(t1, 0) + rent(t2, 0) / 2;
    let low = rent(t1, 0) * 3 / 2;
    eprintln!(
        "t02 record (RULING: linked half boosted?): paid = {}, base = {base}, 1.5*R1 = {low}",
        10_000 - t.money(1)
    );
    assert!(
        10_000 - t.money(1) >= low,
        "at least 1.5 × R(1)"
    );
}

// =====================================================================
// T3. Anon link plus Repaint
// =====================================================================

// 规则书: Repaint: 「对方此次结算的支付减半」. Expect (R1 + R2/2) / 2.
#[ignore = "DISCREPANCY: Anon Tokyo link + Repaint arrangement"]
#[test]
fn t03_anon_link_plus_repaint() {
    let mut t = Table::new(&["千早爱音", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    let t1 = tile("购物中心");
    let t2 = tile("天文馆");
    t.own(0, &[t1, t2]);
    t.set_pos(0, t1);
    t.give_play(0, "MyGO:[千早爱音]Anon Tokyo").unwrap();
    drain(&mut t);
    until_turn(&mut t, 1);
    t.give(1, &["RAS:Repaint"]);
    t.set_pos(1, t1 - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("RAS:Repaint") {
            t.counteract(1, "RAS:Repaint").unwrap();
        } else {
            t.decline();
        }
    }
    let full = rent(t1, 0) + rent(t2, 0) / 2;
    let half = ((full as f64 / 2.0 / 10.0).ceil() as i32) * 10;
    assert_eq!(t.money(1), 10_000 - half, "halved: {full} -> {half}");
}

// =====================================================================
// T4. Anon link with a mortgaged partner tile
// =====================================================================

// 规则书: 「如果格子地契已抵押则无效果」 -- a mortgaged tile collects nothing.
// RULING: whether the link still adds half of 2's 收费 when 2 is mortgaged.
#[ignore = "RULING: whether the link adds half of a mortgaged partner tile's 收费"]
#[test]
fn t04_anon_link_mortgaged_partner() {
    let mut t = Table::new(&["千早爱音", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    let t1 = tile("购物中心");
    let t2 = tile("天文馆");
    t.own(0, &[t1, t2]);
    t.set_pos(0, t1);
    t.set_mortgaged(t2, true);
    t.give_play(0, "MyGO:[千早爱音]Anon Tokyo").unwrap();
    drain(&mut t);
    t.set_pos(1, t1 - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    eprintln!(
        "t04 record (RULING: linked half when mortgaged): paid = {} (R1 = {})",
        10_000 - t.money(1),
        rent(t1, 0)
    );
}

// =====================================================================
// T5. ONE OF US shares settlement income
// =====================================================================

// 规则书: ONE OF US: 「先在原主人方结算完成，之后被分享方资金直接增加，不受其他任何效果影响」.
#[ignore = "DISCREPANCY: ONE OF US shared-settlement arrangement"]
#[test]
fn t05_one_of_us_shares() {
    let mut t = Table::new(&["美竹兰", "花园多惠", "青叶摩卡"]);
    t.clean();
    t.begin_turn(2);
    let t47 = tile("羽泽咖啡厅");
    let t48 = tile("山吹面包房");
    t.own(0, &[t47]);
    t.own(1, &[t48]);
    t.place_raw(0, "通用:[衍生]FEVER!");
    t.give_play(0, "AG:ONE OF US").unwrap();
    // Designate P0's 47 and P1's 48.
    loop {
        let Some(p) = t.prompt() else { break };
        if p.kind == "tile" {
            let want = if p.items.iter().any(|s| s == "47") { "47" } else { "48" };
            let k = p.items.iter().position(|s| s == want).unwrap() as i32;
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    // P2 lands on 48 and pays R(48). P1 receives it, then P0 gets half from P1.
    until_turn(&mut t, 2);
    t.set_pos(2, t48 - 1);
    t.dice(&[1]);
    t.roll(2).unwrap();
    drain(&mut t);
    let r = rent(t48, 0);
    eprintln!(
        "t05 record: P1 = {}, P0 = {}, R = {r}",
        t.money(1),
        t.money(0)
    );
    assert_eq!(t.money(2), 10_000 - r, "P2 pays R");
}

// =====================================================================
// T6. Tomorrow's Door's surcharge
// =====================================================================

// 规则书: Tomorrow's Door (3): 「额外支付[拥有者]星之鼓动山丘上房子数量×100的资金」.
#[test]
fn t06_tomorrows_door_surcharge() {
    // vanilla: no bound skills, so begin_turn leaves no hook routine pending and
    // the arrangement seams below may call world_mut. The Door's (3) is a card
    // effect and needs no skill.
    let mut t = Table::vanilla(2);
    let t44 = tile("星之鼓动山丘");
    t.own(0, &[t44]);
    t.set_houses(t44, 2);
    t.place_raw(0, "PPP:Tomorrow's Door");
    // Door is in P0's play area (field). P1 settles on P0's tile.
    until_turn(&mut t, 1);
    t.set_pos(1, t44 - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    let expected = rent(t44, 2) + 200;
    let paid = 10_000 - t.money(1);
    eprintln!("t06 record: paid = {paid} (want {expected} = R + 2*100)");
    // 规则书: Tomorrow's Door (3): extra = (houses on 星之鼓动山丘)×100 = 2×100.
    assert_eq!(paid, expected, "R(星之鼓动,2) + 2×100 surcharge");
}

// =====================================================================
// T7. Tomorrow's Door walks its sequence
// =====================================================================

// 规则书: Tomorrow's Door (2): 「每次[经过]此卡所在的格子时把此卡放置到…序列中的下一个」.
#[test]
fn t07_tomorrows_door_walks() {
    // vanilla: no bound skills → no hook routine pending after begin_turn.
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "户山香澄");
    t.begin_turn(0);
    // (2): 「将此卡放置在"流星堂"上」 — give_play drops it on 流星堂.
    t.set_pos(0, 40);
    t.give_play(0, "PPP:Tomorrow's Door").unwrap();
    // Walk past 45.
    t.dice(&[8]);
    t.roll(0).unwrap();
    drain(&mut t);
    eprintln!(
        "t07 record: P0 field = {:?} (Door may have moved off 45)",
        t.field_ids(0)
    );
    // (2): 「每次[经过]此卡所在的格子时把此卡放置到…序列中的下一个」
    // 流星堂 → 花咲川女子学院.
    let field = t.field(0);
    let f = field
        .iter()
        .find(|f| f.card.contains("Tomorrow's Door"))
        .expect("door still on the field");
    assert_eq!(
        f.tile,
        tile("花咲川女子学院") as i32,
        "door walked to the next in the sequence"
    );
}

// =====================================================================
// T8. 练习室里的风暴 forces a remote settle
// =====================================================================

// 规则书: 练习室里的风暴 (2): 「距此卡所在格子X个格子处[结算]时…进行一次此卡所在格子的[结算]…
// 地租为普通[结算]的(4-X)/4倍…X至少为1且小等于此卡[奇迹水晶]数量」.
#[test]
fn t08_practice_storm_remote_settle() {
    let mut t = Table::vanilla(2);
    let t50 = tile("Live House Galaxy");
    let t52 = tile("旭汤澡堂");
    t.own(0, &[t50]);
    // (1): 「可将此卡放置在当前格子上」 — the card sits on the livehouse.
    place_on_tile(&mut t, 0, "RAS:练习室里的风暴", t50);
    t.set_crystals(0, "RAS:练习室里的风暴", 2);
    // P1 settles on 52 (X = 2).
    until_turn(&mut t, 1);
    t.set_pos(1, t52 - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    let scaled = rent(t50, 0) * (4 - 2) / 4;
    eprintln!(
        "t08 record: P1 paid = {} (R(50) = {}, scaled = {scaled})",
        10_000 - t.money(1),
        rent(t50, 0)
    );
    // (2): 「那个玩家进行一次此卡所在格子的[结算]，此次[结算]的地租为普通[结算]的(4-X)/4倍」.
    assert_eq!(
        10_000 - t.money(1),
        scaled,
        "remote settle of the card's tile at (4-X)/4 rent"
    );
}

// 规则书: 同上. X = 3 > 2 crystals: nothing happens.
#[test]
fn t08b_practice_storm_x_too_big() {
    let mut t = Table::vanilla(2);
    let t50 = tile("Live House Galaxy");
    let t53 = tile("RiNG 4");
    t.own(0, &[t50]);
    t.place_raw(0, "RAS:练习室里的风暴");
    t.set_crystals(0, "RAS:练习室里的风暴", 2);
    until_turn(&mut t, 1);
    t.set_pos(1, t53 - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    assert_eq!(t.money(1), 10_000, "nothing happens when X > crystals");
}

// =====================================================================
// T9. Change the world
// =====================================================================

// 规则书: Change the world: 「下一次收费增加50*(y+1)*n且触发时获得等量资金…触发后将该卡放入弃牌堆」.
#[test]
fn t09_change_the_world() {
    let mut t = Table::vanilla(2);
    let t50 = tile("Live House Galaxy");
    t.own(0, &[t50]);
    t.set_houses(t50, 1);
    t.give(0, &["RAS:Change the world"]);
    // 「将此卡放置于你的一个有房屋的livehouse格子上」.
    place_on_tile(&mut t, 0, "RAS:Change the world", t50);
    t.set_crystals(0, "RAS:Change the world", 3);
    // P1 lands on 50: R(50,1) + 50*(y+1)*n with y = 3 crystals, n = 1 house.
    until_turn(&mut t, 1);
    t.set_pos(1, t50 - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    let extra = 50 * (3 + 1) * 1;
    let expected = rent(t50, 1) + extra;
    eprintln!(
        "t09 record: P1 paid = {} (want {expected}), P0 gained extra = {extra}",
        10_000 - t.money(1)
    );
    // 「下一次收费增加50*（y+1）*n且触发时获得等量资金」.
    assert_eq!(10_000 - t.money(1), expected, "R + 50*(y+1)*n");
}

// =====================================================================
// T10. Ringing Bloom raises house counts
// =====================================================================

// 规则书: Ringing Bloom (2): 「房屋数视为与你房屋数最多的格子等同，但…收费减半」.
// (3): 「任意非RiNG格子收费后…获得500*X资金，X为你收费格上的房屋数」.
#[test]
fn t10_ringing_bloom() {
    // vanilla: no bound skills → no hook routine pending after begin_turn, so the
    // arrangement seams may call world_mut. Ringing Bloom is a card effect.
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "白金燐子");
    t.begin_turn(0);
    let t1 = tile("购物中心");
    let t2 = tile("天文馆");
    t.own(0, &[t1, t2]);
    t.set_houses(t1, 3);
    t.set_houses(t2, 0);
    // (1): 「将此卡放置于自身场上」 — play it on P0's own turn.
    t.give_play(0, "R:（燐子）Ringing Bloom").unwrap();
    until_turn(&mut t, 1);
    t.set_pos(1, t2 - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    let expected = rent(t2, 3) / 2;
    let paid = 10_000 - t.money(1);
    eprintln!("t10 record: P1 paid = {paid} (want R(2,3)/2 = {expected}), P0 = {} (+500*3)", t.money(0));
    // (2): t2's houses count as the max (3) and the inflated ones charge half.
    assert_eq!(paid, expected, "R(天文馆,3)/2 — the inflated houses charge half");
    // (3): after charging a non-RiNG tile, +500*X with X = the charged tile's
    // (treated) house count = 3.
    assert_eq!(
        t.money(0),
        10_000 + expected + 500 * 3,
        "owner gets the halved rent + 500×X, X = 3"
    );
}

// =====================================================================
// T11. Fire bird plus HHW band (1) doubling
// =====================================================================

// 规则书: Fire bird 1.5× + HHW band (1) 「支付双倍价格」. RULING: order; both give 3×R.
#[test]
fn t11_fire_bird_plus_hhw_double() {
    let mut t = Table::new(&["弦卷心", "花园多惠"]);
    t.clean();
    t.begin_turn(1);
    let t48 = tile("山吹面包房");
    t.own(0, &[t48]);
    t.place_raw(0, "R:Fire bird");
    until_turn(&mut t, 1);
    t.set_pos(1, t48 - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    // P1 chooses double (HHW band (1)).
    loop {
        let Some(_) = t.prompt() else { break };
        let k = t.option("双倍").or_else(|| t.option("double"));
        if let Some(k) = k {
            t.answer(1, k).unwrap();
        } else {
            t.decline();
        }
    }
    let r = rent(t48, 0);
    eprintln!(
        "t11 record (RULING: order): paid = {} (want 3×R = {}), P1 band crystals = {:?}",
        10_000 - t.money(1),
        r * 3,
        t.crystals(1, &t.skill_id(1, "传播笑容"))
    );
}

// =====================================================================
// T12. 学生会的检查 on a tile
// =====================================================================

// 规则书: 学生会的检查: 「支付那格一层房屋的建造价格一半将此卡放于那个格子上…
// 下一个经过且移动终点不在此格的你以外的玩家强制停下并触发结算…地租价格为原价格一半」.
#[ignore = "DISCREPANCY: 学生会的检查 placement / forced stop"]
#[test]
fn t12_student_council_check() {
    let mut t = Table::vanilla(2);
    let t1 = tile("购物中心");
    t.own(0, &[t1]);
    t.set_pos(0, t1 + 2);
    t.give(0, &["R:学生会的检查"]);
    t.play(0, "R:学生会的检查").unwrap();
    // Pay H(t)/2 and place it on t.
    loop {
        let Some(p) = t.prompt() else { break };
        if p.kind == "tile" {
            let k = p.items.iter().position(|s| s == "1").unwrap() as i32;
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    let half_house = house_cost(t1) / 2;
    eprintln!(
        "t12 record: P0 paid H/2 = {half_house}, money = {}",
        t.money(0)
    );
    // 「可支付那格一层房屋的建造价格一半将此卡放于那个格子上」.
    assert_eq!(
        10_000 - t.money(0),
        half_house,
        "P0 pays half a house cost to place the card"
    );
    // P1 walks past t (end != t): forced to stop, pays R(t)/2.
    until_turn(&mut t, 1);
    t.set_pos(1, t1 - 2);
    t.dice(&[4]);
    t.roll(1).unwrap();
    drain(&mut t);
    let half_rent = rent(t1, 0) / 2;
    eprintln!(
        "t12 record: P1 pos = {} (want {t1}), paid = {} (want R/2 = {half_rent})",
        t.pos(1),
        10_000 - t.money(1)
    );
    // 「下一个经过且移动终点不在此格的你以外的玩家强制停下并触发结算…地租价格为原价格一半」.
    assert_eq!(t.pos(1), t1, "P1 force-stopped on the card's tile");
    assert_eq!(10_000 - t.money(1), half_rent, "the forced settle pays half rent");
}

// =====================================================================
// T13. 笑容大游行 swaps a tile with 弦卷集团
// =====================================================================

// 规则书: 笑容大游行 (1)(2)(3). Assert placement, crystals, discard; record the swap.
#[ignore = "DISCREPANCY: 笑容大游行 overflows the stack when it triggers"]
#[test]
fn t13_smile_parade() {
    let mut t = Table::vanilla(2);
    let t28 = tile("弦卷集团");
    t.set_pos(0, t28 - 2);
    t.give(0, &["HHW:笑容大游行"]);
    t.dice(&[4]);
    t.roll(0).unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("HHW:笑容大游行") {
            t.counteract(0, "HHW:笑容大游行").unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!(
        "t13 record: P0 field = {:?}, crystals = {:?}, marks = {:?}",
        t.field_ids(0),
        t.crystals(0, "HHW:笑容大游行"),
        t.marks()
    );
}

// =====================================================================
// T14. 黑衣人的补给 turns 弦卷集团 into a CiRCLE
// =====================================================================

// 规则书: 黑衣人的补给: 「在弦卷集团格子上放置一个奇迹水晶，该格上拥有奇迹水晶时，
// 该格获得CiRCLE格子的全部效果」.
#[test]
fn t14_black_clothes_supply() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 0);
    t.give(0, &["HHW:黑衣人的补给"]);
    t.dice(&[2]);
    t.roll(0).unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("HHW:黑衣人的补给") {
            t.counteract(0, "HHW:黑衣人的补给").unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!(
        "t14 record: marks = {:?}, P0 hand = {:?}",
        t.marks(),
        t.hand(0)
    );
}

// =====================================================================
// T15. （kkr）前往笑容集结的地方！ on CiRCLE, plus 爱心义演
// =====================================================================

// 规则书: （kkr）: 「支付10000资金…将此卡置于CiRCLE上，若其他玩家在该格[触发结算]则向所有者支付6000」.
// 爱心义演: 「若弦卷心已将专属卡置于CiRCLE上，则CiRCLE也算作属于弦卷心的格子」.
#[test]
fn t15_kkr_on_circle() {
    let mut t = Table::new(&["弦卷心", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.give_play(0, "HHW:（kkr）前往笑容集结的地方！").unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 0, "pays 10000");
    // P1 settles at CiRCLE and pays 6000.
    until_turn(&mut t, 1);
    t.set_pos(1, 59);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    assert_eq!(t.money(1), 10_000 - 6000, "P1 pays 6000");
    // P0 passing CiRCLE adds +2 to its move (爱心义演).
    until_turn(&mut t, 0);
    t.give(0, &["HHW:爱心义演"]);
    t.play(0, "HHW:爱心义演").unwrap();
    drain(&mut t);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    eprintln!("t15 record: P0 pos = {} (3 + 2 = 5 if CiRCLE counted)", t.pos(0));
}

// =====================================================================
// T16. （soyo）混合的颜色 and an agent tile
// =====================================================================

// 规则书: （soyo）: 「该格获得所有颜色…从在其他颜色的地产商格子触发结算的玩家处收费时，
// 收费在地产商的减半收费基础上额外减半」.
#[ignore = "DISCREPANCY: （soyo）混合的颜色 agent half-rent"]
#[test]
fn t16_soyo_mixed_colors() {
    let mut t = Table::new(&["长崎素世", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    let s = tile("购物中心");
    t.own(0, &[s]);
    t.set_pos(0, s);
    t.give_play(0, "MyGO:（soyo）混合的颜色").unwrap();
    drain(&mut t);
    // P1 settles on another colour's agent tile and chooses s.
    let agent = tile("主要街道");
    until_turn(&mut t, 1);
    t.set_pos(1, agent - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    loop {
        let Some(p) = t.prompt() else { break };
        if p.kind == "tile" {
            let k = p.items.iter().position(|x| x == &s.to_string()).unwrap() as i32;
            t.answer(1, k).unwrap();
        } else {
            t.decline();
        }
    }
    let expected = rent(s, 0) / 2 / 2;
    eprintln!(
        "t16 record: P1 paid = {} (want R(s)/2/2 = {expected})",
        10_000 - t.money(1)
    );
    // 「因该效果从在其他颜色的地产商格子触发结算的玩家处收费时，收费在地产商的减半收费基础上额外减半」.
    assert_eq!(10_000 - t.money(1), expected, "R/2/2 on the mixed-colour agent");
}

// =====================================================================
// T17. 那天的雨 colour zone
// =====================================================================

// 规则书: 那天的雨: 「投掷1d10…第n个地产商对应的颜色格子及这些格子相邻格子上的所有玩家获得一层[停留]」.
#[test]
fn t17_that_days_rain() {
    let mut t = Table::vanilla(3);
    t.give(0, &["MyGO:那天的雨"]);
    t.dice(&[1, 5]);
    t.play(0, "MyGO:那天的雨").unwrap();
    drain(&mut t);
    eprintln!(
        "t17 record: stays = {}, {}, {}; field = {:?}",
        t.state(0, "stay"),
        t.state(1, "stay"),
        t.state(2, "stay"),
        t.field_ids(0)
    );
    // 1d10 = 1 → the 1st agent (主要街道) → colour #4682B7 (购物中心/偶像经纪公司/
    // 星空齿科/江户川公园) and their neighbours, which include CiRCLE (0) where
    // everyone sits. 「…相邻格子上的所有玩家获得一层[停留]」.
    assert_eq!(t.state(0, "stay"), 1, "P0 gains a 停留");
    assert_eq!(t.state(1, "stay"), 1, "P1 gains a 停留");
    assert_eq!(t.state(2, "stay"), 1, "P2 gains a 停留");
    // 「将此卡放置于自己场上并为其放置5个奇迹水晶」.
    assert_eq!(
        t.crystals(0, "MyGO:那天的雨"),
        Some(5),
        "the card is placed with 5 crystals"
    );
}

// =====================================================================
// T18. （乐奈）有趣的女人 builds up, then stops someone
// =====================================================================

// 规则书: （乐奈）: 「当奇迹水晶总数为5或以上时使下一个经过的你以外的玩家选择失去一个抹茶芭菲
// 或强制停下并[触发结算]…支付资金减半」.
#[test]
fn t18_rana_interesting_woman() {
    // vanilla: no bound skills → no hook routine pending after begin_turn.
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "要乐奈");
    t.begin_turn(0);
    let yamabuki = tile("山吹面包房");
    // 「将此卡置于当前格子上」 — P0 stands on 48 and plays it there.
    t.set_pos(0, yamabuki);
    t.own(0, &[yamabuki]);
    t.give_play(0, "MyGO:（乐奈）有趣的女人").unwrap();
    t.set_crystals(0, "MyGO:（乐奈）有趣的女人", 5);
    // P1 passes the tile the card is on (path 47..50 passes 48, ends at 50).
    until_turn(&mut t, 1);
    t.set_pos(1, yamabuki - 2);
    t.dice(&[4]);
    t.roll(1).unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        // Prefer the force-stop branch so (4)/(5) are exercised.
        let k = t.option("停").or_else(|| t.option("芭菲")).or_else(|| t.option("yes"));
        if let Some(k) = k {
            t.answer(1, k).unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!(
        "t18 record: P1 pos = {}, paid = {}",
        t.pos(1),
        10_000 - t.money(1)
    );
    // (3)/(4): with no 抹茶芭菲 to lose the choice collapses to the force-stop,
    // which stops P1 on the card's tile and shuffles the card into the discard.
    assert_eq!(t.pos(1), yamabuki, "force-stopped on the card's tile");
    // (5): a settle this card forces pays half.
    assert_eq!(
        10_000 - t.money(1),
        rent(yamabuki, 0) / 2,
        "the forced settle pays half"
    );
    assert!(
        !t.on_field(0, "MyGO:（乐奈）有趣的女人"),
        "card left the field"
    );
    assert!(
        t.draw_pile(0).iter().any(|c| c.contains("有趣的女人")),
        "card filed and immediately reshuffled into the empty deck: {:?}",
        t.draw_pile(0)
    );
}

// =====================================================================
// T19. （沙绫）总有一天要给这片天空命名 relocates itself
// =====================================================================

// 规则书: （沙绫）(2): 「[经过]此卡后在回合结束后获得1个[火罐]，然后投掷3d20将此卡放置在投掷结果的格子上」.
#[test]
fn t19_saaya_names_the_sky() {
    // vanilla: no bound skills → no hook routine pending after begin_turn.
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "山吹沙绫");
    // vanilla strips the skill, which zeroes the fire cap; restore it so 「获得1个[火罐]」 can land.
    t.set_fire(0, 0, 5);
    t.begin_turn(0);
    // (1): 「将此卡放置在山吹面包房」 — the play body drops it there.
    let yamabuki = tile("山吹面包房");
    t.give_play(0, "PPP:（沙绫）总有一天要给这片天空命名").unwrap();
    t.set_pos(0, yamabuki - 2);
    t.dice(&[4, 5, 5, 5]);
    t.roll(0).unwrap();
    drain(&mut t);
    pass(&mut t, 0);
    eprintln!(
        "t19 record: fire = {}, field = {:?}, tiles = {:?} (3d20 = 5+5+5 -> tile #15 = index 14)",
        t.fire(0),
        t.field_ids(0),
        t.field(0).iter().map(|f| (f.card.clone(), f.tile)).collect::<Vec<_>>()
    );
    // (2): 「[经过]此卡后在回合结束后获得1个[火罐]」.
    assert_eq!(t.fire(0), 1, "+1 fire at turn end after passing the card");
    // (2): 「然后投掷3d20将此卡放置在投掷结果的格子上」 → 5+5+5 = 15 → tile #15.
    let field = t.field(0);
    let f = field
        .iter()
        .find(|f| f.card.contains("总有一天"))
        .expect("card still on the field");
    assert_eq!(f.tile, 14, "card moved to tile #15 = index 14");
}

// =====================================================================
// T20. 该清CP了 CP points spread
// =====================================================================

// 规则书: 该清CP了: 「下2回合开始时…在相邻的没有[CP点]的格子添加1个[CP点]…
// 在拥有[CP点]的格子上[结算]时移除…[获得]800资金」.
#[test]
fn t20_clear_cp() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "通用:该清CP了").unwrap();
    loop {
        let Some(p) = t.prompt() else { break };
        if p.kind == "tile" {
            let k = p.items.len().saturating_sub(1) as i32;
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!(
        "t20 record: marks = {:?}, field = {:?}",
        t.marks(),
        t.field_ids(0)
    );
    // 「在任意一个没有角色和[CP点]的格子上添加1个[CP点]」 — a CP mark lands on the
    // chosen tile. (The 「场上]6个[CP点]」 and the 800-on-settle halves are recorded
    // above; their representation is not pinned by the sheet.)
    assert!(!t.marks().is_empty(), "a [CP点] mark is placed: {:?}", t.marks());
}

// =====================================================================
// T21. （立希）想认真去做 plus an Anon link
// =====================================================================

// 规则书: （立希）: 「拥有[停留]的玩家立刻在所在格子前后2格内你选择的一个格子进行一次[触发结算]，
// 本次结算导致的所有[支付]变为原价的四分之一」.
#[ignore = "DISCREPANCY: （立希）想认真去做 quarter-rent on a linked tile"]
#[test]
fn t21_riki_plus_anon_link() {
    let mut t = Table::new(&["椎名立希", "千早爱音", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    let t1 = tile("购物中心");
    let t2 = tile("天文馆");
    t.own(1, &[t1, t2]);
    t.set_pos(1, t1);
    t.give_play(1, "MyGO:[千早爱音]Anon Tokyo").unwrap();
    drain(&mut t);
    t.set_state(2, "stay", 1);
    t.set_pos(2, t1 + 1);
    until_turn(&mut t, 0);
    t.give_play(0, "MyGO:（立希）想认真去做").unwrap();
    loop {
        let Some(p) = t.prompt() else { break };
        if p.kind == "tile" {
            let k = p
                .items
                .iter()
                .position(|s| s == &t1.to_string())
                .map(|x| x as i32)
                .unwrap_or(0);
            t.answer(0, k).unwrap();
        } else if p.kind == "player" {
            answer_player(&mut t, 0, 2);
        } else {
            t.decline();
        }
    }
    let full = rent(t1, 0) + rent(t2, 0) / 2;
    let quarter = full / 4;
    eprintln!(
        "t21 record: P2 paid = {} (want (R1+R2/2)/4 = {quarter}, full = {full})",
        10_000 - t.money(2)
    );
    // 「本次结算导致的所有[支付]变为原价的四分之一」.
    assert_eq!(10_000 - t.money(2), quarter, "all payments become a quarter");
}

// =====================================================================
// T22. 星之鼓动山丘 co-ownership and Tomorrow's Door
// =====================================================================

// 规则书: PPP band (3): 「非Poppin' Party角色对星之鼓动山丘的[结算]改为分摊支付给所有未抵押的PPP角色」.
// RULING: whether 44 counts as P0's owned tile for Tomorrow's Door's surcharge.
#[ignore = "RULING: whether the co-owned 星之鼓动山丘 counts as 「[拥有者]拥有的格子」 for Tomorrow's Door (3) surcharge"]
#[test]
fn t22_hill_coownership() {
    let mut t = Table::new(&["户山香澄", "花园多惠", "青叶摩卡"]);
    t.clean();
    t.begin_turn(2);
    let t44 = tile("星之鼓动山丘");
    t.place_raw(0, "PPP:Tomorrow's Door");
    t.set_pos(2, t44 - 1);
    t.dice(&[1]);
    t.roll(2).unwrap();
    drain(&mut t);
    let r = rent(t44, 0);
    eprintln!(
        "t22 record (RULING: Door surcharge on co-owned hill): P0 = {}, P1 = {}, P2 = {} (R = {r}, split)",
        t.money(0),
        t.money(1),
        t.money(2)
    );
}

// =====================================================================
// T23. 狂乱Hey Kids!! replaces a settlement
// =====================================================================

// 规则书: 狂乱Hey Kids!!: 「将本格上的房屋转移到属于你的可建造格子上…随后，你失去
// 转移后各格房屋造价总和－获得房屋数量×500 的资金」.
#[test]
fn t23_riot_hey_kids() {
    let mut t = Table::vanilla(2);
    let t1 = tile("购物中心");
    let t2 = tile("天文馆");
    t.own(0, &[t1, t2]);
    t.set_houses(t1, 1);
    t.give(0, &["RAS:狂乱Hey Kids!!"]);
    t.set_pos(0, t1 - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("RAS:狂乱Hey Kids!!") {
            t.counteract(0, "RAS:狂乱Hey Kids!!").unwrap();
        } else {
            let p = t.expect_prompt();
            if p.kind == "tile" {
                let k = p
                    .items
                    .iter()
                    .position(|s| s == &t2.to_string())
                    .map(|x| x as i32)
                    .unwrap_or(0);
                t.answer(0, k).unwrap();
            } else {
                t.decline();
            }
        }
    }
    eprintln!(
        "t23 record: houses t1 = {}, t2 = {}, money = {}",
        t.houses(t1),
        t.houses(t2),
        t.money(0)
    );
}

// =====================================================================
// T24. Parking Space plus 要乐奈 (2)
// =====================================================================

// 规则书: Parking Space (1)(2): 「此卡所在格子的[结算]改为回合结束后获得一层[停留]」
// 「位于此卡所在格子上的玩家无法使用角色及乐队技能」.
#[ignore = "DISCREPANCY: Parking Space + 要乐奈 (2) teleport arrangement"]
#[test]
fn t24_parking_space() {
    let mut t = Table::new(&["都筑诗船", "要乐奈"]);
    t.clean();
    t.begin_turn(1);
    let space = tile("Space");
    t.give_play(0, "通用:[都筑诗船]Parking Space").unwrap();
    drain(&mut t);
    // P1 teleports to Space via 要乐奈 (2).
    t.set_fire(1, 3, 3);
    let sid = t.skill_id(1, "投币式停车场的猫");
    t.skill(1, &sid).unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        eprintln!("t24 prompt: {}", t.dump_prompt());
        let k = t.option("Space").or_else(|| t.option(&space.to_string()));
        if let Some(k) = k {
            t.answer(1, k).unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!(
        "t24 record: P1 pos = {}, stay = {}, marks = {:?}",
        t.pos(1),
        t.state(1, "stay"),
        t.marks()
    );
}

// =====================================================================
// T25. 高贵的微蓝 vs tile-designating effects
// =====================================================================

// 规则书: 高贵的微蓝: 「在该地块上放置一个标记，有标记时此地块不能被指定」.
#[test]
fn t25_noble_blue_mark() {
    let mut t = Table::vanilla(2);
    let t1 = tile("购物中心");
    t.own(0, &[t1]);
    t.set_houses(t1, 3);
    t.set_pos(0, t1);
    t.give_play(0, "Mor:高贵的微蓝").unwrap();
    drain(&mut t);
    // 无路矢 by P1 designating P0's tile while P0 stands on the marked t: t must not be offered.
    until_turn(&mut t, 1);
    t.give_play(1, "MyGO:无路矢").unwrap();
    let p = t.expect_prompt();
    eprintln!(
        "t25 record: prompt = {} (t = {t1} must not be offered)",
        t.dump_prompt()
    );
    assert!(
        !p.options.iter().any(|o| format!("{o:?}").contains(&t1.to_string())),
        "marked tile must not be offered: {}",
        t.dump_prompt()
    );
}

// =====================================================================
// T26. 可爱又强壮的花朵 stops only its user
// =====================================================================

// 规则书: 可爱又强壮的花朵: 「[使用者][经过]此卡所在格子时…[强制停下]…此卡放入[使用者]弃卡区」.
#[test]
fn t26_cute_strong_flower() {
    let mut t = Table::new(&["花园多惠", "青叶摩卡"]);
    t.clean();
    t.begin_turn(0);
    t.place_raw(0, "PP:可爱又强壮的花朵");
    // P1 walking past is not stopped; P0 is.
    until_turn(&mut t, 1);
    t.set_pos(1, 0);
    t.dice(&[10]);
    t.roll(1).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(1), 10, "P1 is not stopped");
    until_turn(&mut t, 0);
    t.set_pos(0, 0);
    t.dice(&[10]);
    t.roll(0).unwrap();
    drain(&mut t);
    eprintln!(
        "t26 record: P0 pos = {}, discard = {:?}",
        t.pos(0),
        t.discard(0)
    );
}

// =====================================================================
// T27. （育美） marks
// =====================================================================

// 规则书: （育美）(1): 「投掷2次4d20并将一个育美标记放置到投掷结果之一的格子上，
// 你经过育美标记时可在那格强制停下并获得2000资金，然后移除该标记」.
#[test]
fn t27_ikumi_marks() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "北泽育美");
    t.give(0, &["HHW:（育美）"]);
    // 2×4d20 = 6 and 4 (dice 1,1,1,3 / 1,1,1,1); neither > 60 so a mark is placed.
    t.dice(&[1, 1, 1, 3, 1, 1, 1, 1]);
    t.play(0, "HHW:（育美）").unwrap();
    // Choose one of the two results; take the first offered.
    loop {
        let Some(_) = t.prompt() else { break };
        eprintln!("t27 play prompt: {}", t.dump_prompt());
        t.answer(0, 0).unwrap();
    }
    let marks = t.marks();
    eprintln!("t27 marks after play: {marks:?}");
    let mark_tile = marks.first().map(|m| m.tile as usize).expect("a 育美 mark was placed");
    // Walk past the mark's tile (start 2 before it; roll 4).
    t.set_pos(0, (mark_tile + 58) % 60);
    t.dice(&[4]);
    t.roll(0).unwrap();
    let mut saw_offer = false;
    loop {
        let Some(_) = t.prompt() else { break };
        if t.option("yes").is_some() || t.option("停").is_some() || t.option("2000").is_some() {
            saw_offer = true;
        }
        eprintln!("t27 prompt: {}", t.dump_prompt());
        // Take the force-stop (ask.yes).
        let k = t
            .option("yes")
            .or_else(|| t.option("停"))
            .or_else(|| t.option("2000"));
        if let Some(k) = k {
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!(
        "t27 record: P0 money = {}, marks = {:?}",
        t.money(0),
        t.marks()
    );
    // 「你经过育美标记时可在那格强制停下并获得2000资金，然后移除该标记」.
    assert!(saw_offer, "育美 mark: force-stop + 2000 must be offered");
    assert_eq!(t.pos(0), mark_tile, "force-stopped on the mark's tile");
    assert_eq!(t.money(0), 12_000, "gained 2000");
    assert!(t.marks().is_empty(), "the mark is removed");
}

// =====================================================================
// T28. 花园多惠 (1) rabbits
// =====================================================================

// 规则书: 花园多惠 (1): 「领取[CiRCLE奖励]时投掷3d20…在结果对应的格子放置一个[多惠兔子]…
// 经过有[多惠兔子]的格子时[移除]…并获得等量的[火罐]」.
#[test]
fn t28_hanae_rabbits() {
    let mut t = Table::new(&["花园多惠", "青叶摩卡"]);
    t.clean();
    // The trigger is 「领取[CiRCLE奖励]」, which PPP band (2) 「无法获取[CiRCLE奖励]」
    // starves in a live PPP game. Bind only the character skill so the reward
    // actually lands and the 3d20 runs.
    t.strip_skills();
    t.place_raw(0, "skill:花园多惠:花园警察，出警！");
    // Arrange before begin_turn: the turn-start skill hooks leave a routine
    // pending, and world_mut may not be used while one is.
    t.set_fire(0, 2, 4);
    t.set_pos(0, 59);
    t.dice(&[2, 2, 2, 3]);
    t.begin_turn(0);
    drain(&mut t);
    // Take the CiRCLE reward (walk past 0).
    t.roll(0).unwrap();
    let mut took_reward = false;
    loop {
        let Some(_) = t.prompt() else { break };
        eprintln!("t28 prompt: {}", t.dump_prompt());
        let k = t
            .option("2000")
            .or_else(|| t.option("抽"))
            .or_else(|| t.option("money"))
            .or_else(|| t.option("card"));
        if let Some(k) = k {
            took_reward = true;
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!(
        "t28 record: marks = {:?}, fire = {} (cap 4), took_reward = {took_reward}",
        t.marks(),
        t.fire(0)
    );
    // The reward path must actually run, or the exclusion below is vacuous.
    assert!(took_reward, "the CiRCLE reward must be offered and taken");
    // 「如果结果对应的格子不是"星空齿科"则…放置一个[多惠兔子]」 — 3d20 = 2+2+3 = 7
    // → tile #7 = 星空齿科, so the exclusion applies and no rabbit is placed.
    assert!(
        t.marks().is_empty(),
        "no rabbit on 星空齿科: {:?}",
        t.marks()
    );
}

// =====================================================================
// T29. 北泽精肉店 croquettes
// =====================================================================

// 规则书: 北泽育美 (1): 「其他角色经过格子上可乐饼时…向你支付50*X资金」.
// RULING: whether X for 50*X counts the croquette just taken (「持有的」 before
// or after the transfer). The take itself is decided; the amount is recorded.
#[test]
fn t29_croquettes() {
    let mut t = Table::new(&["北泽育美", "花园多惠"]);
    t.clean();
    // Arrange before begin_turn: the turn-start skill hooks leave a routine
    // pending, and world_mut may not be used while one is.
    t.set_pos(1, 48);
    t.begin_turn(0);
    drain(&mut t);
    eprintln!("t29 marks after begin_turn(0): {:?}", t.marks());
    until_turn(&mut t, 1);
    t.dice(&[3]);
    t.roll(1).unwrap();
    let mut took = false;
    loop {
        let Some(_) = t.prompt() else { break };
        let k = t
            .option("yes")
            .or_else(|| t.option("可乐饼"))
            .or_else(|| t.option("转移"));
        if let Some(k) = k {
            took = true;
            t.answer(1, k).unwrap();
        } else {
            eprintln!("t29 prompt (declined): {}", t.dump_prompt());
            t.decline();
        }
    }
    eprintln!(
        "t29 record (RULING: X before/after the take): P1 pos = {}, money = {}, P0 money = {}",
        t.pos(1),
        t.money(1),
        t.money(0)
    );
    // 「其他角色经过格子上可乐饼时可以将其转移到自己场上」 — the take is offered
    // and moves the croquette off the board tile.
    assert!(took, "the croquette take must be offered");
    assert!(
        t.marks().iter().all(|m| m.tile != 51),
        "the croquette is transferred off tile 51: {:?}",
        t.marks()
    );
}

//! Black-box rulebook tests for the settle/move stage model
//! (`docs/rulebook/SETTLE-STAGES.md`, user rulings 2026-10-07).
//!
//! Spec: `data/rules.txt` and the 规则书 timing tables in
//! `docs/rulebook/rulebook-doc.md` (行动阶段 12–16). Every case cites the
//! clause it is checking and shows the **behaviour difference** the re-homing
//! makes: a no-settle move still runs the move tail; a 「[结算]时」 clause dies
//! with a body replace; a mid-route pass (not the end-tile [重叠]) fires the
//! 经过 effects; a no-settle teleport still fires [经过]/[重叠] at its
//! destination; a settleBefore relocation redirects the settle.

mod common;
use common::*;

// ---------------------------------------------------------- local helpers

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..60 {
        if t.turn() == who {
            return;
        }
        let cur = t.turn();
        // Skip the other seats' main moves without rolling.
        t.m.world_mut().st.skip_move = true;
        let _ = t.end(cur);
        drain(t);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

// =====================================================================
// M1 -- 「移动后」 fires for a 「不触发结算」 move too
// =====================================================================

// 规则书 (Sumimi:儿时玩伴的鼓励): 「可在移动掷骰前打出此卡，使本次移动以
// "小豆岛"为起点并在移动后获得一个火罐。」
//
// 行动阶段 13 「移动后」 + 其他规则注意事项 1.2 「是否[结算]」 gates only the
// settle (14–15): a 「不触发结算」 move still runs the move tail, so the pot
// lands. (`SETTLE-STAGES.md` §4 M1, ruling R1.) Before the re-home the card
// hung off `settleBefore`+`settleAfter`+`teleported` and a no-settle walk
// granted nothing.
#[test]
fn m1_no_settle_move_still_grants_the_move_after_fire() {
    let mut t = Table::new(&["三角初华（Sumimi）", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.give(0, &["Sumimi:(初华（Sumimi）)儿时玩伴的鼓励"]);
    t.play(0, "Sumimi:(初华（Sumimi）)儿时玩伴的鼓励").unwrap();
    drain(&mut t);
    // The pot is granted at 「移动后」, not at play time -- snapshot here.
    let before = t.fire(0);
    // The main move is 「移动60格子并不触发结算」 (向着未来的路标): a
    // completed move that does not settle.
    t.give(0, &["PPP:向着未来的路标"]);
    t.play(0, "PPP:向着未来的路标").unwrap();
    drain(&mut t);
    assert!(
        t.fire(0) >= before + 1,
        "「并在移动后获得一个火罐」 fires on a 「不触发结算」 move: fire={} -> {} keys={:?}",
        before,
        t.fire(0),
        t.recent_keys(20)
    );
    assert!(
        t.discard(0).iter().any(|c| c.contains("儿时玩伴")),
        "the attachment is filed after 「移动后」: {:?}",
        t.discard(0)
    );
}

// =====================================================================
// M2 -- a 「[结算]时」 clause dies with a field-card body replace
// =====================================================================

// 规则书 (MyGO:哪怕这旅程没有终点)（2）: 「[持续] 触发结算时，获得X*60资金，
// X为你此次主要移动[经过]的格数。」 and 通用:[都筑诗船]Parking Space（1）:
// 「此卡所在格子的[结算]改为回合结束后获得一层[停留]。」
//
// 专有名词 5: 「[结算]：执行格子上的所有效果」 -- the clause is an entry in the
// settle's effect list, so 「将本次结算改为…」 (L6 「技能和卡的效果优先」)
// replaces it. (`SETTLE-STAGES.md` §4 M2.) Before the re-home the clause sat on
// `settleAfter` and kept running after the replacement.
#[test]
fn m2_body_replace_skips_the_settle_time_clause() {
    let mut t = Table::new(&["都筑诗船", "户山香澄", "花园多惠"]);
    t.clean();
    let space = tile("Space");
    // P0 plays Parking Space on their own turn (「将此卡放置于"Space"格子上」).
    t.begin_turn(0);
    drain(&mut t);
    t.give_play(0, "通用:[都筑诗船]Parking Space").unwrap();
    drain(&mut t);
    // P1 stands on Space, plants 哪怕这旅程没有终点 there (「将此卡放置于当前
    // 格子上」), then settles on Space themselves -- the clause is
    // 「触发结算时，获得X*60」 on the owner's own main move.
    until_turn(&mut t, 1);
    t.set_pos(1, space);
    t.give_play(1, "MyGO:哪怕这旅程没有终点").unwrap();
    drain(&mut t);
    let m1 = t.money(1);
    t.set_pos(1, (space + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    assert_eq!(
        t.money(1),
        m1,
        "「触发结算时」 is an entry in the replaced body: money={} keys={:?}",
        t.money(1),
        t.recent_keys(20)
    );
}

// The control for [`m2_body_replace_skips_the_settle_time_clause`]: without the
// replacement the clause runs.
#[test]
fn m2_settle_time_clause_runs_without_a_body_replace() {
    let mut t = Table::new(&["都筑诗船", "户山香澄", "花园多惠"]);
    t.clean();
    let space = tile("Space");
    t.begin_turn(0);
    drain(&mut t);
    until_turn(&mut t, 1);
    t.set_pos(1, space);
    t.give_play(1, "MyGO:哪怕这旅程没有终点").unwrap();
    drain(&mut t);
    let m1 = t.money(1);
    t.set_pos(1, (space + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    // 「触发结算时，获得X*60资金」 -- the payout ran (X = 1 tile walked). A
    // rent on the landing may net against it; the observable is the gain log.
    let keys = t.recent_keys(30);
    assert!(
        keys.iter().any(|k| k.contains("gain") || k.contains("endless_journey")),
        "「触发结算时」 pays out when the body runs: {} -> {} keys={:?}",
        m1,
        t.money(1),
        keys
    );
}

// =====================================================================
// M4 -- a mid-route pass, not the end-tile [重叠]
// =====================================================================

// 规则书 (skill:濑田薰:梦幻的王子殿下)（1）: 「每次[经过]或被[经过]时，若场上
// 不存在[怪盗标记]，获得1火罐（上限7）。」
//
// 行动阶段 12 「[经过]」 fires per path tile; 13 「[重叠]」 is the end tile. A
// walk that only *passes* the holder's tile (and ends elsewhere) must still
// grant the pot. (`SETTLE-STAGES.md` §4 M4.) Before the re-home the clause sat
// on `passPlayer` and missed every mid-route pass.
#[test]
fn m4_a_pass_by_not_an_overlap_triggers_kaoru() {
    let mut t = Table::new(&["濑田薰", "户山香澄"]);
    t.clean();
    t.begin_turn(1);
    drain(&mut t);
    // P0 (濑田薰) stands on tile 10; P1 walks 5..8 and passes 10? No --
    // put P0 on the path and end P1 past them.
    t.set_pos(0, 10);
    t.set_pos(1, 7);
    let before = t.fire(0);
    t.dice(&[5]); // 7 -> 8,9,10,11,12: passes 10 mid-route, ends on 12
    t.roll(1).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(1),
        12,
        "the walk ended past P0: pos={}",
        t.pos(1)
    );
    assert!(
        t.fire(0) > before,
        "「被[经过]」 grants the pot on a mid-route pass: fire={} -> {} keys={:?}",
        before,
        t.fire(0),
        t.recent_keys(20)
    );
}

// 规则书 (skill:凑友希那:来练习吧)（3）: 「当其他玩家移动[经过]您时，您可以
// 选择使用一个[火罐]令该玩家强制停下并触发结算。」
//
// 行动阶段 12 [经过], mid-route. Before the re-home the clause sat on
// `passPlayer`, where `move_remaining()` is always 0 and the guard
// `<= 0 -> return` meant the force-stop could never fire (`SETTLE-STAGES.md`
// §4 M4's latent bug).
#[test]
fn m4_kokoro_force_stop_stops_a_passer() {
    let mut t = Table::new(&["凑友希那", "户山香澄"]);
    t.clean();
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(0, 10);
    t.set_fire(0, 1, 1);
    t.set_pos(1, 7);
    t.dice(&[5]); // passes 10 mid-route
    t.roll(1).unwrap();
    // Accept the force-stop offer.
    let mut offered = false;
    while let Some(p) = t.prompt() {
        if format!("{p:?}").contains("kokoro_practice_force")
            || format!("{p:?}").contains("force")
        {
            offered = true;
            let _ = t.answer_one(0);
        } else {
            t.decline();
        }
    }
    assert!(offered, "the force-stop offer came up: {:?}", t.recent_keys(20));
    assert_eq!(
        t.pos(1),
        10,
        "「令该玩家强制停下」 stops them on P0's tile: pos={}",
        t.pos(1)
    );
    assert_eq!(t.fire(0), 0, "the pot was spent");
}

// =====================================================================
// M6a / R2 -- a 「不[触发结算]」 teleport still fires [经过]/[重叠] at its
// destination (专名词 9: the path is just the endpoint; B41 fires
// [经过],[重叠],[结算] there; 1.2 removes only the [结算]).
// =====================================================================

// 规则书 (其他规则注意事项 1.2 + 专名词 9 + B41): 「是否[结算]」 gates only the
// settle; a teleport's [路径] is just the endpoint, and B41 fires [经过],[重叠],
// [结算] there in that order. A 「不[触发结算]」 teleport therefore still raises
// [经过] (`passTile`) and [重叠] (`passPlayer`) at its destination -- never at
// mid-route squares. (`SETTLE-STAGES.md` §6 M6a / ruling R2.) The vehicle is
// `TEST:tele_nosettle`, a no-settle teleport move.
#[test]
fn m6a_no_settle_teleport_fires_overlap_at_the_destination() {
    let mut t = Table::vanilla(2);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 5);
    // P1 stands where P0 will land (5 + 5 = 10).
    t.set_pos(1, 10);
    t.give_play(0, "TEST:tele_nosettle").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 10, "teleported 5 forward: pos={}", t.pos(0));
    // 「移动终点触发[重叠]」 -- the overlap is at the destination. The
    // observable is the overlap log; 要乐奈 (3)'s 800 would also move money.
    let keys = t.recent_keys(30);
    assert!(
        keys.iter()
            .any(|k| k.contains("overlap") || k.contains("passPlayer") || k.contains("重合")),
        "a 「不[触发结算]」 teleport raises [重叠] at its destination: {keys:?}"
    );
}

// =====================================================================
// Q7 -- a settleBefore relocation redirects the settle and re-runs the
// window at the new tile (user ruling 2026-10-07).
// =====================================================================

// 规则书 (CRYCHIC:是我自己的问题) [反击]（1）: 「主要移动结束时，[触发结算]前
// 打出此卡，使自己额外远离绝对距离最近的玩家一格」 -- the settle follows the
// mover to the new tile and that tile's own [触发结算] runs.
#[test]
fn q7_settle_before_relocation_settles_at_the_new_tile() {
    let mut t = Table::vanilla(2);
    t.set_pos(1, 10);
    t.give(0, &["CRYCHIC:是我自己的问题"]);
    t.set_pos(0, 5);
    t.dice(&[4]); // 5 -> 9, next to P1 at 10
    t.roll(0).unwrap();
    let mut played = false;
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("CRYCHIC:是我自己的问题") {
            t.counteract(0, "CRYCHIC:是我自己的问题").unwrap();
            played = true;
        } else {
            t.decline();
        }
    }
    assert!(played, "the [反击] window offered the card");
    assert_eq!(
        t.pos(0),
        8,
        "the settle redirected to the new tile: pos={} keys={:?}",
        t.pos(0),
        t.recent_keys(20)
    );
}
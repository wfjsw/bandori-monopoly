//! Coverage-gap cases G18–G24 (counteract windows / targeting / status),
//! `docs/rulebook/CROSS-TESTS.md` §6 G5–G7. Black-box: expectations from the
//! sheet extracts (`target/scratch/rb/*.md`) and `data/rules.txt`.

mod common;

use common::*;
use game_core::msg::Arg;

/// Inert filler for draw piles / hands (never auto-plays).
const FILL: &str = "R:[衍生] 觉悟";

fn set_draw_n(t: &mut Table, who: usize, n: usize) {
    let cards: Vec<&str> = vec![FILL; n];
    t.set_draw(who, &cards);
}

/// Decline every open prompt.
fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// End `who`'s turn without a main move.
fn pass(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

/// Advance until it is `who`'s turn.
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

/// ceil-to-10 of `n`.
fn ceil10(n: i32) -> i32 {
    (n + 9) / 10 * 10
}

/// Answer a `player`-choice prompt with the option naming `target`.
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

/// True when the open prompt offers a player-choice naming `target`.
fn offers_player(t: &Table, target: i32) -> bool {
    let Some(p) = t.prompt() else { return false };
    p.options.iter().any(|o| match o.a.get("who") {
        Some(Arg::PlayerId(v)) => *v == target,
        Some(Arg::I(v)) | Some(Arg::N(v)) => *v == target as i64,
        _ => false,
    })
}

/// Was `card` offered as a counteract at any point while draining?
fn drain_recording(t: &mut Table, card: &str) -> bool {
    let mut offered = false;
    for _ in 0..30 {
        if t.prompt().is_none() {
            return offered;
        }
        if t.counteract_offered(card) {
            offered = true;
        }
        t.decline();
    }
    offered
}

/// Drain, declaring `card` as `who` whenever it is offered.
fn drain_playing(t: &mut Table, who: usize, card: &str) {
    for _ in 0..30 {
        if t.prompt().is_none() {
            return;
        }
        if t.counteract_offered(card) {
            t.counteract(who, card).unwrap();
        } else {
            t.decline();
        }
    }
}

// =====================================================================
// G18. negate-one x shut-window: 骰子已经掷下 while a multi-target pay is live
// =====================================================================

// 规则书 (骰子已经掷下): 「本回合内所有其他玩家无法从手牌中使用[反击]」.
// 规则书 (网络链接异常) 1: 「取消其对目标之一的[指定]」.
// Note: `c14` shows 骰子已经掷下 does not shut the windows; this case adds the
// negate-one leg.
#[ignore = "DISCREPANCY: 骰子已经掷下 does not shut the counteract windows (c14); 网络链接异常 still drops a 武道馆 target"]
#[test]
fn g18_shut_window_blocks_the_negate_one() {
    let mut t = Table::vanilla(2);
    t.give(1, &["通用:网络链接异常"]);
    t.give(0, &["Mujica:骰子已经掷下"]);
    t.play(0, "Mujica:骰子已经掷下").unwrap();
    // Let it resolve (P1 may counter the card itself; decline).
    drain(&mut t);
    t.give_play(0, "通用:登上武道馆").unwrap();
    // No window for P2's 网络链接异常: all three others pay. (2 players, so
    // only P1 is designated -- use 3 players for "all three others".)
    let offered = drain_recording(&mut t, "通用:网络链接异常");
    assert!(
        !offered,
        "the counter window is shut for 网络链接异常: {}",
        t.dump_prompt()
    );
    let x = ceil10(2000 / 1);
    assert_eq!(t.money(1), 10_000 - x, "P1 pays {x}");
    assert_eq!(t.money(0), 10_000 + x, "P0 receives {x}");
}

// =====================================================================
// G19. negate-one x join-window: 真奈 lets a third party drop one designation
// =====================================================================

// 规则书 (（真奈）歌唱大赛5连冠): 「此时场上其他玩家可如同自身的对应目标被指定
// 一般打出[反击]卡，且其反击卡中针对打出玩家自身的效果改为你。若以此种方式使你
// 免于受到该影响，打出那张[反击]卡的玩家可抽一张卡」.
// 规则书 (网络链接异常) 1: 「取消其对目标之一的[指定]」.
// Doc error: the design expects "P3 pays 0; P0 and P2 pay X" -- but the sheet
// says the joiner's self-targeted effect 「改为你」 (to 真奈's user), so P3's
// cancel drops **P0's** designation and P0 is the one exempted. P3 still pays.
#[ignore = "DISCREPANCY: 真奈's join + 网络链接异常 negates the whole 武道馆 (everyone keeps 10000); the sheet cancels one designation (P0's via 「改为你」) and P2/P3 still pay X"]
#[test]
fn g19_manas_join_window_lets_a_third_party_drop_one() {
    let mut t = Table::new(&["纯田真奈", "仓田真白", "花园多惠", "山吹沙绫"]);
    t.clean();
    t.begin_turn(1);
    drain(&mut t);
    set_draw_n(&mut t, 3, 3);
    t.give(0, &["Sumimi:（真奈）歌唱大赛5连冠"]);
    t.give(3, &["通用:网络链接异常"]);
    // P1 plays 武道馆, designating P0, P2 and P3 (X = ceil10(2000/3) = 670).
    t.give_play(1, "通用:登上武道馆").unwrap();
    // The ring on the [手] effect (ruling 2026-10-07: starts with the initial
    // user P1, who holds no counter and is skipped): P0 counters with 真奈,
    // P3 plays 网络链接异常 under the join (its self-directed cancel
    // 「改为你」, i.e. P0).
    for _ in 0..30 {
        if t.prompt().is_none() {
            break;
        }
        if t.counteract_offered("Sumimi:（真奈）歌唱大赛5连冠") {
            t.counteract(0, "Sumimi:（真奈）歌唱大赛5连冠").unwrap();
        } else if t.counteract_offered("通用:网络链接异常") {
            t.counteract(3, "通用:网络链接异常").unwrap();
        } else {
            t.decline();
        }
    }
    drain(&mut t);
    let x = ceil10(2000 / 3);
    eprintln!(
        "g19 record: P0={} P1={} P2={} P3={} (x = {x})",
        t.money(0),
        t.money(1),
        t.money(2),
        t.money(3)
    );
    assert_eq!(t.money(0), 10_000, "P0 is exempted (真奈's user): {}", t.money(0));
    assert_eq!(t.money(2), 10_000 - x, "P2 pays {x}");
    assert_eq!(t.money(3), 10_000 - x, "P3 pays {x}");
    assert_eq!(t.hand(3).len(), 1, "P3 draws 1 (真奈's draw for the joiner)");
}

// =====================================================================
// G20. shut-window x join-window: 真奈 during 骰子已经掷下
// =====================================================================

// 规则书 (骰子已经掷下): 「本回合内所有其他玩家无法从手牌中使用[反击]」.
// RULING: does 「无法使用[反击]」 also block a counter that another card
// (真奈) invites? Assert yes. The engine already offers no window, so this is
// a live pass.
#[test]
fn g20_shut_window_blocks_the_join_window() {
    let mut t = Table::new(&["纯田真奈", "仓田真白"]);
    t.clean();
    t.begin_turn(1);
    drain(&mut t);
    t.give(0, &["Sumimi:（真奈）歌唱大赛5连冠"]);
    t.give_play(1, "Mujica:骰子已经掷下").unwrap();
    // P0 cannot counter with 真奈: assert no window offers it.
    let offered = drain_recording(&mut t, "Sumimi:（真奈）歌唱大赛5连冠");
    assert!(
        !offered,
        "真奈 is not offered during 骰子已经掷下: {}",
        t.dump_prompt()
    );
}

// =====================================================================
// G21. immunity x absorb: 夏日合宿 plus 祥子 (1) status absorption
// =====================================================================

// 规则书 (夏日合宿): 「直到下个自己的回合开始前，你只会被自己发动的效果指定」.
// 规则书 (祥子 (1)): 「经过其他玩家时，可将其所有层数的停留，眩晕转移至自己身上…
// 每获得一层停留，眩晕，你获得1500资金」.
// Note: `s08` is green (祥子 absorbs); this stays open for the 夏日合宿 half.
#[ignore = "DISCREPANCY: 夏日合宿 should keep P1 un-designated so there is no stay to absorb (s08 absorb is fixed)"]
#[test]
fn g21_summer_camp_immune_to_the_rains_zone() {
    let mut t = Table::new(&["仓田真白", "花园多惠", "丰川祥子"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // P1 plays 夏日合宿: only its own effects may designate it.
    t.give(1, &["Mor:夏日合宿"]);
    until_turn(&mut t, 1);
    t.play(1, "Mor:夏日合宿").unwrap();
    drain(&mut t);
    until_turn(&mut t, 0);
    // P0 plays 那天的雨; a loaded 1d10 selects the first agent's colour.
    t.give(0, &["MyGO:那天的雨"]);
    t.dice(&[1, 1, 1]);
    t.play(0, "MyGO:那天的雨").unwrap();
    drain(&mut t);
    // P1 is in the zone (everyone starts at CiRCLE, adjacent to the first
    // colour) but 夏日合宿 shields it: no stay, so P2's pass absorbs nothing.
    assert_eq!(
        t.state(1, "stay"),
        0,
        "夏日合宿 keeps P1 un-designated: {}",
        t.state(1, "stay")
    );
    assert_eq!(t.fire(2), 0, "P2 absorbs nothing, gains no fire");
}

// =====================================================================
// G22. redirect x absorb: EXIST retargets a status card onto a 祥子
// =====================================================================

// 规则书 (EXIST): 「所有对单一玩家生效的手卡…的目标将改为你」.
// 规则书 (雨啊，快点来吧): 「[指定]X名玩家…获得一层[停留]」.
// 规则书 (祥子 (1)): a *pass* trigger, not a designation shield.
#[test]
fn g22_exist_redirects_a_stay_onto_shouko() {
    let mut t = Table::new(&["花园多惠", "仓田真白", "丰川祥子"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // P2 (祥子) holds EXIST.
    t.place_raw(2, "RAS:EXIST");
    t.give(0, &["通用:雨啊，快点来吧"]);
    t.dice(&[1, 1]);
    t.play(0, "通用:雨啊，快点来吧").unwrap();
    // Designate P1 -- EXIST retargets the single-player designation to P2.
    for _ in 0..10 {
        if t.prompt().is_none() {
            break;
        }
        if t.asked().contains(&0) && t.prompt().unwrap().options.len() > 2 {
            answer_player(&mut t, 0, 1);
        } else {
            t.decline();
        }
    }
    drain(&mut t);
    assert_eq!(
        t.state(2, "stay"),
        1,
        "P2 gains the retargeted stay: {}",
        t.state(2, "stay")
    );
    assert_eq!(t.state(1, "stay"), 0, "P1 is not designated");
}

// =====================================================================
// G23. exile x unstoppable: 初华 state 2 under 无路矢's exile
// =====================================================================

// 规则书 (无路矢): 「指定场上自己以外的一位玩家所在格子，获得2层[除外]并在[除外]
// 层数归0后[传送]至该格子」 -- the *user* gains the exile.
// 规则书 (51): [不可阻挡] 「无法获得新的[除外]层数」.
// Doc error: the design has P0 play 无路矢 "giving P1 2 [除外]"; per the sheet
// the exile goes to the user. P1 (初华, unstoppable) is the user.
// RULING: 无路矢's exile is the cost of the teleport. If the exile is refused,
// is the whole card negated? Assert the card is negated (P1 is not teleported).
// DISCREPANCY: the engine still grants the 2 [除外] to a [不可阻挡] holder
// (glossary 51: 「无法获得新的[除外]层数」).
#[ignore = "DISCREPANCY: glossary 51 says [不可阻挡] 「无法获得新的[除外]层数」, but 无路矢 still grants 2 [除外] to the holder"]
#[test]
fn g23_unstoppable_refuses_the_exile_cost() {
    let mut t = Table::new(&["三角初华", "仓田真白", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // P0 is 初华 in state 2: [不可阻挡].
    t.set_state(0, "skillState", 2);
    t.set_state(0, "unstoppable", 1);
    t.give(0, &["MyGO:无路矢"]);
    // P0 designates P1's tile and would gain 2 [除外].
    t.set_pos(1, 20);
    t.play(0, "MyGO:无路矢").unwrap();
    if t.prompt().is_some() && t.asked().contains(&0) {
        // Tile prompt: pick P1's tile (20).
        if t.prompt().unwrap().kind == "tile" {
            let _ = t.answer_tile(0, 20);
        } else {
            answer_player(&mut t, 0, 1);
        }
    }
    drain(&mut t);
    assert_eq!(
        t.state(0, "exile"),
        0,
        "[不可阻挡] gains no new [除外]: {}",
        t.state(0, "exile")
    );
    assert_eq!(
        t.pos(0),
        0,
        "and is not teleported later (the card is negated): {}",
        t.pos(0)
    );
}

// =====================================================================
// G24. unstoppable x clear-status: 壱雫空 vs a [不可阻挡] holder
// =====================================================================

// 规则书 (壱雫空): 「清除场上所有[停留]与[眩晕]效果，所有玩家因本效果每清除一种
// 效果则支付此卡使用者1000资金」.
// 规则书 (51): [不可阻挡] 「[停留]，[晕眩]，[除外]在适当时机依旧掉层」 -- a
// clear is not a new layer, so the stay is cleared.
#[test]
fn g24_unstoppable_still_loses_the_stay_to_a_clear() {
    let mut t = Table::new(&["花园多惠", "三角初华"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // P1 is 初华 in state 2 ([不可阻挡]) and already holds 1 [停留]. Arm the
    // [不可阻挡] gate directly (not skillState, which would also arm Ave
    // Mujica's 1.5x payment scale and muddy the 1000 charge).
    t.set_state(1, "unstoppable", 1);
    t.set_state(1, "stay", 1);
    t.give(0, &["MyGO:壱雫空"]);
    t.play(0, "MyGO:壱雫空").unwrap();
    drain(&mut t);
    assert_eq!(
        t.state(1, "stay"),
        0,
        "a clear is not a new layer: the stay goes",
    );
    // 1000 per cleared type: P1 had one type (stay).
    assert_eq!(t.money(1), 10_000 - 1000, "P1 pays 1000 for the cleared stay");
    assert_eq!(t.money(0), 10_000 + 1000, "P0 receives it");
}
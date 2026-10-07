//! Coverage-gap cases G01–G07 (money pipeline), `docs/rulebook/CROSS-TESTS.md`
//! §6 G1. Black-box: expectations from the sheet extracts
//! (`target/scratch/rb/*.md`) and `data/rules.txt`.

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

/// ceil-to-10 of `n` (the rulebook's 「向上取整10」).
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

/// Land `who` on `tile` with a loaded one-step move from the tile before it.
fn land_on(t: &mut Table, who: usize, tile: usize) {
    t.set_pos(who, (tile + 59) % 60);
    t.dice(&[1]);
    t.roll(who).unwrap();
    drain(t);
}

/// Declare `card` as `who` at the first window offering it, answering any
/// follow-up choice with `cancel` (the designation to drop, if asked).
fn counteract_cancelling(t: &mut Table, who: usize, card: &str, cancel: i32) {
    for _ in 0..30 {
        let Some(_) = t.prompt() else { return };
        if t.counteract_offered(card) {
            t.counteract(who, card).unwrap();
            // 网络链接异常 may ask which designation to cancel.
            for _ in 0..6 {
                let Some(_) = t.prompt() else { break };
                eprintln!("after {card} declare: {}", t.dump_prompt());
                if offers_player(t, cancel) {
                    answer_player(t, who, cancel);
                } else {
                    break;
                }
            }
            return;
        }
        t.decline();
    }
}

// =====================================================================
// G01. multiply x multiply: Fire bird 1.5x and Ave Mujica 1.5x on one rent
// =====================================================================

// 规则书 (Fire bird): 「自己场地上存在此卡时…自己的所有格子收费变成1.5倍」.
// 规则书 (Ave Mujica): 「你在状态2时从[收取]与[支付]的资金改为1.5倍（只对自己
// 结算，对应的其他玩家收支不变）」.
// Doc note: the design's setup ("P0 owns a tile with a Fire bird" but "P0
// lands on P1's tile") cannot put both multipliers on one rent -- Fire bird
// scales the *holder's* tiles. The landing direction is swapped: P1 lands on
// P0's tile, so Fire bird (1.5x charge) and Ave Mujica (1.5x 收取) both hit it.
// RULING: the order of two multipliers on one amount. Assert R * 1.5 * 1.5.
#[ignore = "RULING: order of two multipliers on one rent (Fire bird 1.5x x Ave Mujica 1.5x)"]
#[test]
fn g01_fire_bird_times_ave_mujica_on_one_rent() {
    let mut t = Table::new(&["三角初华", "仓田真白"]);
    t.clean();
    t.begin_turn(1);
    drain(&mut t);
    // P0 is Ave Mujica, in state 2 (its own [收取] scales by 1.5).
    t.set_state(0, "skillState", 2);
    // P0 holds Fire bird, so P0's tiles' charges are 1.5x.
    t.place_raw(0, "R:Fire bird");
    t.set_crystals(0, "R:Fire bird", 3);
    let tile = tile("弦卷豪宅");
    let rent = data().tiles[tile].rent[0];
    t.set_owner(tile, Some(0));
    // P1 lands on P0's tile: two multipliers on one rent.
    t.set_pos(1, (tile + 59) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    let charged = rent * 3 / 2;
    let received = charged * 3 / 2;
    assert_eq!(t.money(1), 10_000 - charged, "P1 pays the once-scaled 1.5*{rent}");
    assert_eq!(t.money(0), 10_000 + received, "P0 receives 1.5*1.5*{rent}");
}

// =====================================================================
// G02. multiply x clamp-floor: 摩卡 half-pay under 不要背负期待's +100
// =====================================================================

// 规则书 (0.5倍速): 「你的所有资金支付与消耗减半」.
// 规则书 (不要背负期待 (2)): 「[支付]资金时金额提高100」.
// RULING: the sheet's payment stages order it -- `回合階段&註釋` `C28` 「2.0 /
// 支付计算时 / 支付增加/减少(固定值)」 then `C30` 「4.0 / 支付前 / 支付减半/翻倍」.
// Two separate windows: flat add at 2, half/double at 4. So (R+100)/2, not
// R/2+100. The engine's money pipeline is `payAdd -> payMul -> …` (`play.rs`),
// which is that order.
//
// `place_raw` arms the card's hooks but not its 「生效2次」 stack counter (the
// play body sets that). One stack is what the ruling formula's +100 names;
// 「生效2次」 stacks two (+200) -- see `rb_pp::expect_pay_plus_100`.
#[test]
fn g02_half_speed_plus_expectations_floor_on_a_rent() {
    let mut t = Table::new(&["青叶摩卡", "仓田真白"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, "AG:（摩卡）0.5倍速");
    t.set_crystals(0, "AG:（摩卡）0.5倍速", 3);
    t.place_raw(0, "PP:不要背负期待");
    t.set_state(0, "no_expectation_stacks", 1);
    let tile = tile("弦卷豪宅");
    let rent = data().tiles[tile].rent[0];
    t.set_owner(tile, Some(1));
    // 不要背负期待 (1) cuts the roll by 2 and 0.5倍速 halves the final move:
    // roll 7 -> 5 -> 2 either way round, so start two tiles before `tile`.
    t.set_pos(0, (tile + 58) % 60);
    t.dice(&[7]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), tile, "landed on the rent tile: {}", t.dump_prompt());
    // Add-then-multiply: (R + 100) / 2.
    let paid = (rent + 100) / 2;
    assert_eq!(t.money(0), 10_000 - paid, "(R+100)/2 for R = {rent}");
    assert_eq!(t.money(1), 10_000 + paid, "the payee receives the paid amount");
}

// =====================================================================
// G03. cancel-one x split-share: 网络链接异常 drops one leg of a 分摊
// =====================================================================

// 规则书 (微笑的铁假面): 「其他玩家[分摊][支付][使用者]2000资金」.
// 规则书 (网络链接异常) 1: 「手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]」.
// RULING: does X (the share) recompute after the drop? Assert the pre-drop
// share ceil10(2000/n).
// TODO(ABI): no static targeting query to confirm the designations.
#[ignore = "TODO(ABI): no static targeting query for 微笑的铁假面's 分摊 designations"]
#[test]
fn g03_net_error_drops_one_leg_of_a_split() {
    let mut t = Table::new(&["白鹭千圣", "仓田真白", "花园多惠", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    set_draw_n(&mut t, 0, 2);
    t.give(2, &["通用:网络链接异常"]);
    t.give_play(0, "PP:[白鹭千圣]微笑的铁假面").unwrap();
    counteract_cancelling(&mut t, 2, "通用:网络链接异常", 2);
    drain(&mut t);
    let share = ceil10(2000 / 3);
    assert_eq!(t.money(1), 10_000 - share, "P1 pays the pre-drop share {share}");
    assert_eq!(t.money(2), 10_000, "P2 pays nothing");
    assert_eq!(t.money(3), 10_000 - share, "P3 pays the pre-drop share {share}");
}

// =====================================================================
// G04. multiply x cancel-one: a halved share that then loses one payer
// =====================================================================

// 规则书 (微笑的铁假面): 「其他玩家[分摊][支付][使用者]2000资金」.
// 规则书 (0.5倍速): 「你的所有资金支付与消耗减半」.
// 规则书 (网络链接异常) 1: 「取消其对目标之一的[指定]」.
// Doc note: the design's Repaint / 祥，移动 arrangements do not put a multiply
// and a cancel-one on one shared charge. The 分摊 leg does: P1's share is
// halved by its own 0.5倍速, P2's leg is dropped.
// RULING: does the halving apply before or after a cancel-one, and does the
// share recompute after the drop?
#[ignore = "RULING: halving vs cancel-one order on a 分摊 leg (and whether the share recomputes)"]
#[test]
fn g04_half_share_then_cancel_one_payer() {
    let mut t = Table::new(&["白鹭千圣", "青叶摩卡", "仓田真白", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // P1 halves its own pays.
    t.place_raw(1, "AG:（摩卡）0.5倍速");
    t.set_crystals(1, "AG:（摩卡）0.5倍速", 3);
    t.give(2, &["通用:网络链接异常"]);
    t.give_play(0, "PP:[白鹭千圣]微笑的铁假面").unwrap();
    counteract_cancelling(&mut t, 2, "通用:网络链接异常", 2);
    drain(&mut t);
    let share = ceil10(2000 / 3);
    // The surviving halved payer pays share/2; the cancelled one pays 0.
    assert_eq!(t.money(1), 10_000 - share / 2, "P1 pays {share}/2");
    assert_eq!(t.money(2), 10_000, "P2 pays nothing");
    assert_eq!(t.money(3), 10_000 - share, "P3 pays the full pre-drop share {share}");
}

// =====================================================================
// G05. replace-loss x split-share: 三全音 against a 分摊
// =====================================================================

// 规则书 (三全音): 「任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次
// 失去的资金金额，此卡放置在场上，三回合后…弃置此卡并支付由此卡获得的资金」.
// 规则书 (微笑的铁假面): 「其他玩家[分摊][支付][使用者]2000资金」.
// Doc error: the design's 「P1 gains their share and does not lose it」
// contradicts the sheet (which has no 「改为」 and does not cancel the payment)
// and C18's own 「gains … then pays … Net 0 now」. The sheet reading is: P1
// gains the share and still pays it (net 0), the other legs still pay.
#[test]
fn g05_tritone_against_a_split_share() {
    let mut t = Table::new(&["白鹭千圣", "仓田真白", "花园多惠", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    set_draw_n(&mut t, 1, 3);
    t.give(1, &["Mor:迷茫之蝶们的三全音"]);
    t.give_play(0, "PP:[白鹭千圣]微笑的铁假面").unwrap();
    // P1 counters its 分摊 leg with 三全音.
    counteract_cancelling(&mut t, 1, "Mor:迷茫之蝶们的三全音", 1);
    drain(&mut t);
    let share = ceil10(2000 / 3);
    // P1 gains its share and pays it: net 0 now. The other legs still pay.
    assert_eq!(t.money(1), 10_000, "P1 nets 0 now: {}", t.money(1));
    assert_eq!(t.money(0), 10_000 + 3 * share, "P0 receives all three shares");
    assert_eq!(t.money(2), 10_000 - share);
    assert_eq!(t.money(3), 10_000 - share);
    assert!(
        t.field_ids(1).iter().any(|c| c.contains("三全音")),
        "三全音 sits on the field: {:?}",
        t.field_ids(1)
    );
    assert_eq!(t.crystals(1, "Mor:迷茫之蝶们的三全音"), Some(3));
    // Three of P1's turn ends later it is discarded and P1 pays back.
    for _ in 0..3 {
        until_turn(&mut t, 1);
        pass(&mut t, 1);
    }
    assert!(
        t.discard(1).contains(&"Mor:迷茫之蝶们的三全音".to_string()),
        "discarded after 3 turn ends: {:?}",
        t.discard(1)
    );
    assert_eq!(t.money(1), 10_000 - share, "P1 pays the accumulated amount back");
}

// =====================================================================
// G06. clamp-floor x split-share: 不要背负期待's +100 on a 分摊 leg
// =====================================================================

// 规则书 (不要背负期待 (2)): 「[支付]资金时金额提高100（[收取]资金时金额减少
// 100（最低0））」.
// 规则书 (微笑的铁假面): 「其他玩家[分摊][支付][使用者]2000资金」.
// RULING: each 分摊 leg is its own money pipeline (`play.rs::money`), so the
// flat add at 支付计算时 (`C28`) lands on the leg -- share + 100, not
// (2000+100)/3. Same window ordering as `g02`.
//
// `place_raw` arms the hooks but not the 「生效2次」 stack counter; one stack
// keeps the ruling formula's +100 literal (see `g02`).
#[test]
fn g06_expectations_floor_plus_100_on_a_split_leg() {
    let mut t = Table::new(&["白鹭千圣", "仓田真白", "花园多惠", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(1, "PP:不要背负期待");
    t.set_state(1, "no_expectation_stacks", 1);
    t.give_play(0, "PP:[白鹭千圣]微笑的铁假面").unwrap();
    drain(&mut t);
    let share = ceil10(2000 / 3);
    // P1's leg of the 分摊 is share + 100, floored at 0.
    assert_eq!(t.money(1), 10_000 - (share + 100), "P1 pays {share} + 100");
    assert_eq!(t.money(2), 10_000 - share);
    assert_eq!(t.money(3), 10_000 - share);
}

// =====================================================================
// G07. negator x redirect: EXIST retargeting a card that a negator then drops
// =====================================================================

// 规则书 (EXIST): 「直到自己的下一回合开始，场上及打出的所有对单一玩家生效的
// 手卡…的目标将改为你…若在此期间此卡没有造成影响，抽1张卡」.
// 规则书 (网络链接异常) 1: 「手卡的[手]效果且有[指定]目标则取消其对目标之一的
// [指定]」.
// Doc error: the design picks 通用:安可, which only answers 「[异常移动效果]」 --
// a card landing on a field is not one. 网络链接异常 cancels the (retargeted)
// designation instead.
// RULING: does a negated redirect count as "had an effect"? Assert EXIST still
// draws 1 (it redirected nothing in the end).
#[ignore = "RULING: does a negated EXIST redirect count as 「造成影响」? assert draws 1"]
#[test]
fn g07_exist_redirect_then_negated_designation() {
    let mut t = Table::new(&["花园多惠", "仓田真白", "三角初华", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    set_draw_n(&mut t, 2, 3);
    t.place_raw(2, "RAS:EXIST");
    t.give(2, &["通用:网络链接异常"]);
    t.give(0, &["PP:找回珍妮弗"]);
    t.play(0, "PP:找回珍妮弗").unwrap();
    // If the play asks for a target, aim at P1 -- EXIST retargets it to P2.
    if t.prompt().is_some() && offers_player(&t, 1) {
        answer_player(&mut t, 0, 1);
    }
    // P2 cancels the (retargeted) designation of the landing.
    counteract_cancelling(&mut t, 2, "通用:网络链接异常", 2);
    drain(&mut t);
    assert!(
        t.field_ids(2).iter().all(|c| c != "PP:找回珍妮弗"),
        "the card does not land on P2: {:?}",
        t.field_ids(2)
    );
    assert!(
        t.field_ids(1).iter().all(|c| c != "PP:找回珍妮弗"),
        "the card does not land on P1 either: {:?}",
        t.field_ids(1)
    );
    // EXIST's own check: 「若在此期间此卡没有造成影响，抽1张卡」.
    assert_eq!(
        t.hand(2).len(),
        1,
        "EXIST draws 1 because it redirected nothing: {:?}",
        t.hand(2)
    );
}
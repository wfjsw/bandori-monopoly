//! Status interplay: §4 `S*` of `docs/rulebook/CROSS-TESTS.md`.

mod common;

use common::*;
use game_core::msg::Arg;

const FILL: &str = "R:[衍生] 觉悟";

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

fn rent(t: usize, h: usize) -> i32 {
    let tile = &data().tiles[t];
    tile.rent[(h).min(tile.rent.len().saturating_sub(1))]
}

// =====================================================================
// S1. An exiled player can't be designated
// =====================================================================

// 规则书: 50 -- [除外] 「不能被[指定]」. 登上武道馆 designates all others.
// RULING: whether X counts P1 as alive (it does: 存活 = not bankrupt).
#[test]
fn s01_exiled_cannot_be_designated() {
    let mut t = Table::vanilla(3);
    t.set_state(1, "exile", 1);
    t.give_play(0, "通用:登上武道馆").unwrap();
    drain(&mut t);
    // Only P2 pays. X = ceil10(2000/2) = 1000 if P1 still counts as alive.
    let x = ceil10(2000 / 2);
    assert_eq!(t.money(2), 10_000 - x, "P2 pays {x}");
    assert_eq!(t.money(1), 10_000, "P1 is not designated");
    eprintln!(
        "s01 record (RULING: does X count P1?): P0 = {} (11000 if X = 1000)",
        t.money(0)
    );
}

fn ceil10(n: i32) -> i32 {
    (n + 9) / 10 * 10
}

// =====================================================================
// S2. A stunned player can't pay
// =====================================================================

// 规则书: 49 -- [晕眩] 「无法收付款」.
#[test]
fn s02_stunned_cannot_pay() {
    let mut t = Table::vanilla(3);
    t.set_state(1, "stun", 1);
    t.give_play(0, "通用:登上武道馆").unwrap();
    drain(&mut t);
    let x = ceil10(2000 / 2);
    assert_eq!(t.money(1), 10_000, "P1 pays nothing");
    assert_eq!(t.money(2), 10_000 - x);
}

// =====================================================================
// S3. A stunned player can't receive rent
// =====================================================================

// 规则书: 49 -- [晕眩] 「无法收付款」.
#[test]
fn s03_stunned_cannot_receive() {
    let mut t = Table::vanilla(2);
    t.set_state(1, "stun", 1);
    let t1 = tile("购物中心");
    t.own(1, &[t1]);
    t.set_pos(0, t1 - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), 10_000, "P0 pays nothing");
    assert_eq!(t.money(1), 10_000);
}

// =====================================================================
// S4. 初演大成功 keeps money from falling
// =====================================================================

// 规则书: 初演大成功: 「本回合内你的资金不会下降…回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶」.
// RULING: whether P1 still receives R. Record it.
#[test]
fn s04_debut_success() {
    let mut t = Table::new(&["长崎素世（CRYCHIC）", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.give_play(0, "CRYCHIC:初演大成功").unwrap();
    drain(&mut t);
    let t1 = tile("购物中心");
    t.own(1, &[t1]);
    t.set_pos(0, t1 - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    let r = rent(t1, 0);
    assert_eq!(t.money(0), 10_000, "P0's money does not fall");
    eprintln!(
        "s04 record (RULING: does P1 still receive?): P1 = {} (want 10000+{r}), P0 stun = {}, band crystals = {:?}",
        t.money(1),
        t.state(0, "stun"),
        t.crystals(0, &t.skill_id(0, "美好的往日幻影"))
    );
    pass(&mut t, 0);
    eprintln!(
        "s04 record after turn end: stun = {}, band crystals = {:?}",
        t.state(0, "stun"),
        t.crystals(0, &t.skill_id(0, "美好的往日幻影"))
    );
}

// =====================================================================
// S5. CRYCHIC band (1) at 6 or more hand cards
// =====================================================================

// 规则书: CRYCHIC band (1): 「任何时刻拥有手牌数大于等于6时，你无法获得或失去资金，
// 无法从手中打出任何牌且领取CiRCLE奖励时必须选择抽一张卡」.
// The lock is the skill owner's (`你`) money only: P1 carries the CRYCHIC
// band skill (bound at match start / Returns borrow) and holds 6 cards, so
// P1 cannot lose money (its leg of 登上武道馆 is void) while P2 still pays.
// 「无法从手中打出任何牌」 is a global play gate (needs a `cantPlayHand`
// capability -- BACKLOG SETTLE-02 / B); the `t.play` line records, not asserts.
#[test]
fn s05_crychic_band_six_cards() {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "花园多惠", "青叶摩卡"]);
    t.clean();
    t.begin_turn(0);
    t.set_hand(1, &[FILL, FILL, FILL, FILL, FILL, FILL]);
    t.give_play(0, "通用:登上武道馆").unwrap();
    drain(&mut t);
    let x = ceil10(2000 / 2);
    assert_eq!(t.money(1), 10_000, "P1 neither pays nor receives");
    assert_eq!(t.money(2), 10_000 - x, "P2 pays");
    // P1 can't play a hand card.
    let r = t.play(1, FILL);
    eprintln!("s05 record: play FILL -> {r:?}");
    // On a CiRCLE pass, P1 must take the draw.
    until_turn(&mut t, 1);
    set_draw_n(&mut t, 1, 2);
    t.set_pos(1, 59);
    t.dice(&[2]);
    t.roll(1).unwrap();
    drain(&mut t);
    eprintln!(
        "s05 record: P1 hand = {:?} (must take the draw)",
        t.hand(1)
    );
}

// =====================================================================
// S6. 壱雫空 clears stays and stuns
// =====================================================================

// 规则书: 壱雫空: 「清除场上所有[停留]与[眩晕]效果，所有玩家因本效果每清除一种效果则支付
// 此卡使用者1000资金，若清除了此卡使用者受到的效果则每种效果使用者额外获得1000资金」.
// Ruling 2026-10-06: 「每清除一种效果」 is per effect type, counted separately
// for each affected player. Here P1 停留 (1) + P2 眩晕 (1) + P0 眩晕 (1) = 3.
// Every player pays 3000; the user gains 1000 extra per own type (1).
#[test]
fn s06_izana_clears_statuses() {
    let mut t = Table::vanilla(3);
    t.set_state(1, "stay", 1);
    t.set_state(2, "stun", 1);
    t.set_state(0, "stun", 1);
    t.give_play(0, "MyGO:壱雫空").unwrap();
    drain(&mut t);
    assert_eq!(t.state(1, "stay"), 0, "P1's stay cleared");
    assert_eq!(t.state(2, "stun"), 0, "P2's stun cleared");
    assert_eq!(t.state(0, "stun"), 0, "P0's own stun cleared");
    // count = 3 → every player pays 3000; user also +1000 extra.
    assert_eq!(t.money(1), 7_000, "P1 pays 3000 (count 3)");
    assert_eq!(t.money(2), 7_000, "P2 pays 3000 (count 3)");
    assert_eq!(t.money(0), 10_000 + 3_000 + 3_000 + 1_000, "3000 x 2 + 1000 extra");
}

// =====================================================================
// S7. 椎名立希 (1) counts stays from other players' cards
// =====================================================================

// 规则书: 椎名立希 (1): 「场上每有玩家获得一层[停留]时，你获得一个[火罐]」.
#[test]
fn s07_riki_counts_stays() {
    let mut t = Table::new(&["椎名立希", "花园多惠", "青叶摩卡", "户山香澄"]);
    t.clean();
    // Arrange before begin_turn: the turn-start skill hooks leave a routine
    // pending, and world_mut may not be used while one is.
    t.set_fire(0, 0, 5);
    t.give(1, &["通用:雨啊，快点来吧"]);
    t.dice(&[1, 1]);
    t.begin_turn(1);
    drain(&mut t);
    t.play(1, "通用:雨啊，快点来吧").unwrap();
    // X = 3 designations including the user; pick P2 and P3.
    let mut picks = vec![];
    loop {
        let Some(p) = t.prompt() else { break };
        let is_player = p.options.iter().any(|o| o.key() == "ask.player");
        if is_player {
            let pick = [2i32, 3].into_iter().find(|w| !picks.contains(w)).unwrap_or(2);
            picks.push(pick);
            answer_player(&mut t, 1, pick);
        } else {
            t.decline();
        }
    }
    eprintln!("s07 picks = {picks:?}");
    eprintln!(
        "s07 record: P0 fire = {} (want 2, one per layer), stays = {}, {}",
        t.fire(0),
        t.state(1, "stay"),
        t.state(2, "stay")
    );
    eprintln!(
        "s07 record: fire = {} (want one per layer granted), stays = {}, {}",
        t.fire(0),
        t.state(1, "stay"),
        t.state(2, "stay")
    );
    // 「场上每有玩家获得一层[停留]时，你获得一个[火罐]」 — one fire per layer.
    let total_stay: i32 = [0usize, 1, 2, 3].iter().map(|w| t.state(*w, "stay")).sum();
    assert!(total_stay > 0, "the stays were granted: picks={picks:?}");
    assert_eq!(t.fire(0), total_stay, "one fire per 停留 layer granted");
}

// =====================================================================
// S8. 丰川祥子 (1) absorbs statuses as she passes
// =====================================================================

// 规则书: 丰川祥子 (1): 「经过其他玩家时，可将其所有层数的停留，眩晕转移至自己身上…
// 每获得一层停留，眩晕，你获得1500资金。每次受到停留，眩晕，除外影响…获得一个火罐」.
#[test]
fn s08_sakiko_absorbs_statuses() {
    let mut t = Table::new(&["丰川祥子", "花园多惠", "青叶摩卡"]);
    t.clean();
    t.begin_turn(0);
    t.set_state(0, "skillState", 1);
    t.set_state(1, "stay", 2);
    t.set_pos(1, 5);
    t.set_pos(0, 3);
    t.dice(&[4]);
    t.roll(0).unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        let dump = t.dump_prompt();
        // The absorb ask_yes (`sakiko_life_take`); YesNo option labels are
        // `ask.yes` / `ask.no` (prompt_to_ask), so match the message key.
        if dump.contains("sakiko_life_take") {
            let k = t.option("ask.yes").unwrap_or(0);
            t.answer(0, k).unwrap();
        } else {
            eprintln!("s08 declining: {dump}");
            t.decline();
        }
    }
    eprintln!(
        "s08 record: P1 stay = {}, P0 stay = {}, money0 = {}, fire0 = {}",
        t.state(1, "stay"),
        t.state(0, "stay"),
        t.money(0),
        t.fire(0)
    );
    assert_eq!(t.state(1, "stay"), 0, "P1 = 0");
    assert_eq!(t.state(0, "stay"), 2, "P0 = 2");
    assert_eq!(t.money(0), 10_000 + 3000, "+1500 per layer");
}

// =====================================================================
// S9. Ave Mujica band at 1.5× plus （小白）
// =====================================================================

// 规则书: Ave Mujica band: 「处于状态2时从[收取]与[支付]的资金改为1.5倍（只对自己结算）」.
// （小白）: 「此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金」.
#[test]
fn s09_mujica_band_plus_shiroko() {
    // P0 is 仓田真白 but needs an Ave Mujica band. Use Mujica:（睦/mortis） to copy
    // is complex; arrange the state directly and record.
    let mut t = Table::new(&["仓田真白", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.set_state(0, "skillState", 2);
    t.give(0, &["Mor:（小白）"]);
    let t1 = tile("购物中心");
    t.own(1, &[t1]);
    t.set_pos(0, t1 - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("Mor:（小白）") {
            t.counteract(0, "Mor:（小白）").unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!(
        "s09 record (may be not testable without the Mujica band): P0 = {}, P1 = {}",
        t.money(0),
        t.money(1)
    );
}

// =====================================================================
// S10. 若麦 state 1 raises payments to her
// =====================================================================

// 规则书: 祐天寺若麦 (1): 「每次其他玩家向你支付资金时，你立即获得1火罐…
// 并使那次支付的金额提高X*100，X为你拥有的火罐数」.
// RULING: X = the fire held after the gain, or before. Record which.
#[test]
fn s10_numa_raises_payments() {
    let mut t = Table::new(&["花园多惠", "祐天寺若麦"]);
    t.clean();
    t.begin_turn(0);
    t.set_state(1, "skillState", 1);
    t.set_fire(1, 2, 5);
    let t1 = tile("购物中心");
    t.own(1, &[t1]);
    t.set_pos(0, t1 - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    let r = rent(t1, 0);
    // If X = fire after the gain (3): +300. If before (2): +200.
    eprintln!(
        "s10 record (RULING: X before or after the gain): P1 = {} (R = {r}, +200 or +300), fire = {}",
        t.money(1),
        t.fire(1)
    );
    assert_eq!(t.fire(1), 3, "P1 gains 1 fire");
    let paid = 10_000 - t.money(0);
    assert!(
        paid == r + 200 || paid == r + 300,
        "paid = {paid}, R = {r}"
    );
}
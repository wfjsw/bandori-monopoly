//! G12 status, blocked-ness and immunity gates.
//!
//! Test list: `docs/rulebook/spec/abi/G12-status-and-immunity.md` §9.1 (one
//! test per row). Expectations come from the quoted card / glossary text, never
//! from the engine.
//!
//! Card texts (ids under `docs/rulebook/cards.json`):
//!
//! ```text
//! 通用:@Tsugu ycm：
//! [手]：
//! 投掷3d10并根据结果依次进行以下操作：
//! 1. 结果至少为22则抽1张卡；
//! 2. 结果至少为26则进进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并不[结算]，且可选择购买任意无主的[可购买格子]；
//! 3. 结果至少为28则本回合购买格子时[消耗]资金时降低1500（最低0）；
//! 4. 结果小于26则选择[获得]1000资金或进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并[结算]。
//!
//! Mor:（筑紫）迷茫的庭园：
//! 获得100资金。投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动（选中自己则前进一格，若玩家数量不足6则超出部分重新计算，ex：在五人局roll到6时，视为选中第一位玩家），可以选择是否触发结算。直到下个你的回合开始时，你无法被异常移动
//!
//! [不可阻挡]（glossary 14）：
//! 处于该状态时无视已存在的[停留]，[晕眩]效果也不会获得新的层数；无法获得新的[除外]层数。[停留]，[晕眩]，[除外]在适当时机依旧掉层。处于该状态时可选择受到的[传送]，[强制移动]，[强制停下]效果是否生效。
//!
//! R:选择自己的舞台：
//! [反击] 受到[除外]以外的异常移动效果影响时可打出此卡，选择自己的本次移动（或无法移动的回合结束时）是否触发结算
//!
//! CRYCHIC:想要成为人类 (2)：
//! [自动]若你的回合开始时此卡上拥有两个或以上的奇迹水晶，移除此卡上全部奇迹水晶并使你下次的移动掷骰结果额外增加20-X；若该次移动过程中受到异常移动效果影响，为此卡添加两个奇迹水晶。
//!
//! PP:再次闪耀 [特]：
//! 此卡不受除拥有此卡的玩家以外的玩家的效果影响。
//!
//! RAS:（MASKING）CRUSH ON THE DRUM!!! [特]：
//! 打出此卡的回合任何[使用者]使用的卡包含添加骰效果的卡不受任何其他效果影响。
//! ```

use game_core::engine::SettleAnswer;
use rules_spec::{forall, frame, Table};

const TSUGU: &str = "通用:@Tsugu ycm";
const GARDEN: &str = "Mor:（筑紫）迷茫的庭园";
const OWN_STAGE: &str = "R:选择自己的舞台";
const HUMAN: &str = "CRYCHIC:想要成为人类";
const GRAB: &str = "CRYCHIC:想要抓住...";
const SHINE: &str = "PP:再次闪耀";
const CRUSH: &str = "RAS:（MASKING）CRUSH ON THE DRUM!!!";
const NET_ERROR: &str = "通用:网络链接异常";
const ECHO: &str = "TEST:echo";

const UNSTOPPABLE: &str = bandori_std::state_key::UNSTOPPABLE;
const STAY: &str = bandori_std::state_key::STAY;

const BANDORI: usize = 35; // 「bandori车站」 / board name 「Bandori车站」
const FUJIMI: usize = 11; // 富士见坂, a cheap purchasable

/// Skip every open prompt.
fn skip_all(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// End `who`'s turn without a main move, then drain to the next seat.
fn pass_turn(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    skip_all(t);
}

/// Did the abnormal refusal hook fire after `mark`?
fn refused(t: &Table, mark: i32) -> bool {
    t.keys_since(mark)
        .iter()
        .any(|k| k.contains("abnormalRefused"))
}

/// Player 0 holds [不可阻挡]; hand the turn to player 1, who will deliver.
fn unstoppable_victim() -> Table {
    let mut t = Table::vanilla(2);
    t.set_state(0, UNSTOPPABLE, 1);
    t.set_pos(0, 5);
    t.set_pos(1, 10);
    pass_turn(&mut t, 0);
    t
}

// =====================================================================
// Item 1 -- the replay-stable move gate
// =====================================================================

/// C-G12-tsugu-02: 「结果至少为26则…[传送]到“bandori车站”…且可选择购买
/// 任意无主的[可购买格子]」 -- the buy answer replays the body; the teleport
/// its own activation drove is not "already moved".
///
/// Source: `docs/rulebook/cards.json`, id `通用:@Tsugu ycm`. Design: G12 §6.1,
/// §9.1 (the §1.1 failing run as a pinned test).
#[test]
fn g12_tsugu_replays_its_own_teleport() {
    forall(6, 0x6120_0001, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money);
        t.set_draw(0, &["通用:GREAT"]);
        t.give(0, &[TSUGU]);
        t.dice(&[9, 9, 8]); // 3d10 sum = 26
        let condition = t.snapshot_world();
        t.play(0, TSUGU)
            .map_err(|e| format!("play rejected: {e}"))?;
        if t.pos(0) != BANDORI {
            return Err(format!("pos {}, expected Bandori车站 {BANDORI}", t.pos(0)));
        }
        // 「可选择购买任意无主的[可购买格子]」 -- the offer must be open.
        let p = t.expect_prompt();
        if p.options.len() != 2 {
            return Err(format!("no optional-buy offer: {}", t.dump_prompt()));
        }
        // The buy answer replays the body. The replay must keep the landing.
        t.answer_one(0).map_err(|e| format!("buy yes: {e}"))?;
        if t.pos(0) != BANDORI {
            return Err(format!(
                "the replay dropped the teleport: pos {} (events {:?})",
                t.pos(0),
                t.recent_keys(20)
            ));
        }
        let p = t.expect_prompt();
        if p.kind != "tile" {
            return Err(format!(
                "no tile pick after the buy yes: {}",
                t.dump_prompt()
            ));
        }
        t.answer_tile(0, FUJIMI)
            .map_err(|e| format!("buy tile: {e}"))?;
        skip_all(&mut t);
        if t.owner(FUJIMI) != Some(0) {
            return Err(format!("the purchase did not land: {:?}", t.owner(FUJIMI)));
        }
        if t.money(0) != money - 600 {
            return Err(format!(
                "money {}, expected {} (the 600 purchase charged)",
                t.money(0),
                money - 600
            ));
        }
        frame(
            &condition,
            t.world(),
            &[
                "pos:P0",
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "owner:T11",
                "turn.abnormal",
            ],
        )
    })
    .expect("C-G12-tsugu-02");
}

/// C-G12-tsugu-04: 「结果小于26则…」 -- with the move latch closed the
/// teleport is skipped; the draw still lands. The (2) purchase does not
/// depend on the move and stays offered.
///
/// Source: `docs/rulebook/cards.json`, id `通用:@Tsugu ycm` (glossary 13,
/// [无法移动]). Design: G12 §5.4, §9.1.
#[test]
fn g12_tsugu_stay_latch_still_skips() {
    forall(4, 0x6120_0002, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_state(0, STAY, 1);
        t.m.world_mut().st.skip_move = true; // the latch the stay state derives
        t.set_draw(0, &["通用:GREAT"]);
        t.give(0, &[TSUGU]);
        t.dice(&[9, 9, 8]); // sum 26, but the move gate answers 3
        let condition = t.snapshot_world();
        let pos = t.pos(0);
        t.play(0, TSUGU)
            .map_err(|e| format!("play rejected: {e}"))?;
        if t.pos(0) != pos {
            return Err(format!(
                "teleported from {pos} to {} under the stay latch",
                t.pos(0)
            ));
        }
        skip_all(&mut t);
        if !t.hand(0).iter().any(|c| c == "通用:GREAT") {
            return Err(format!("the (1) draw did not land: {:?}", t.hand(0)));
        }
        if t.owner(BANDORI) == Some(0) {
            return Err("the blocked teleport bought Bandori车站".into());
        }
        frame(
            &condition,
            t.world(),
            &["hand:P0", "draw:P0", "discard:P0", "turn.abnormal"],
        )
    })
    .expect("C-G12-tsugu-04");
}

/// C-G12-garden-02: 「传送到行动条上对应玩家前一格并视为主要移动」 -- the
/// settle answer replays the body; the move its own activation drove is not
/// dropped and the answered settle stands.
///
/// Source: `docs/rulebook/cards.json`, id `Mor:（筑紫）迷茫的庭园`. Design: G12
/// §6.3, §9.1.
#[test]
fn g12_tsukushi_garden_replays_its_own_move() {
    forall(4, 0x6120_0003, |_rng, _i| {
        let mut t = Table::new(&["二叶筑紫", "户山香澄", "花园多惠", "牛込里美", "山吹沙绫"]);
        t.clean();
        t.begin_turn(0);
        t.set_pos(1, 20);
        t.own(1, &[21]); // the destination is owned: a yes settles and pays rent
        t.give(0, &[GARDEN]);
        t.dice(&[2]); // 1d6 = 2 -> p1 -> one ahead of them
        let condition = t.snapshot_world();
        let mark = t.mark();
        t.play(0, GARDEN)
            .map_err(|e| format!("play rejected: {e}"))?;
        // 交给班长吧（1） fires on the card's own 「获得100资金」; decline it.
        t.decline();
        let p = t.expect_prompt();
        if p.options.len() != 2 {
            return Err(format!("no settle offer: {}", t.dump_prompt()));
        }
        if t.pos(0) != 21 {
            return Err(format!("pos {}, expected 21 (one ahead of p1)", t.pos(0)));
        }
        t.answer_one(0).map_err(|e| format!("settle yes: {e}"))?;
        skip_all(&mut t);
        if t.pos(0) != 21 {
            return Err(format!(
                "the replay dropped the move: pos {} (events {:?})",
                t.pos(0),
                t.recent_keys(20)
            ));
        }
        if !t
            .keys_since(mark)
            .iter()
            .any(|k| k.contains("tsukushi_garden_will_settle"))
        {
            return Err(format!("no will_settle line: {:?}", t.recent_keys(20)));
        }
        if t.money(0) != 10_000 + 100 - 240 {
            return Err(format!("money {}, expected the 240 rent paid", t.money(0)));
        }
        if t.money(1) != 10_000 + 240 {
            return Err(format!("money(1) {}, expected the 240 rent", t.money(1)));
        }
        frame(
            &condition,
            t.world(),
            &[
                "money:P0",
                "money:P1",
                "pos:P0",
                "hand:P0",
                "discard:P0",
                "state:P0:unstoppable",
                "field:P0:",
                "fprop:",
                "draw:P0",
                "turn.abnormal",
                "turn.paid_in_settle",
            ],
        )
    })
    .expect("C-G12-garden-02");
}

// =====================================================================
// Item 2 -- [不可阻挡] and blocked-ness
// =====================================================================

/// C-G12-unstoppable-04: 「处于该状态时可选择受到的[传送]…效果是否生效」 --
/// a received teleport does not land and the refusal is reported.
///
/// Source: `docs/rulebook/cards.json`, glossary 14 ([不可阻挡]). Design: G12
/// §5.1, §9.1.
#[test]
fn g12_unstoppable_refuses_a_received_teleport() {
    forall(4, 0x6120_0004, |_rng, _i| {
        let mut t = unstoppable_victim();
        t.give(1, &["TEST:teleporter"]);
        let condition = t.snapshot_world();
        let mark = t.mark();
        t.play(1, "TEST:teleporter")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        if t.pos(0) != 5 {
            return Err(format!("teleport landed: pos {}", t.pos(0)));
        }
        if !refused(&t, mark) {
            return Err(format!("no abnormalRefused: {:?}", t.recent_keys(20)));
        }
        frame(
            &condition,
            t.world(),
            &["pos:P0", "hand:P1", "discard:P1", "draw:P1"],
        )
    })
    .expect("C-G12-unstoppable-04 (teleport)");
}

/// C-G12-unstoppable-04: 「可选择受到的…[强制移动]…效果是否生效」.
///
/// Source: `docs/rulebook/cards.json`, glossary 14 ([不可阻挡]). Design: G12
/// §5.1, §9.1.
#[test]
fn g12_unstoppable_refuses_a_received_forced_move() {
    forall(4, 0x6120_0005, |_rng, _i| {
        let mut t = unstoppable_victim();
        t.give(1, &["TEST:forcer"]);
        let condition = t.snapshot_world();
        let mark = t.mark();
        t.play(1, "TEST:forcer")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        if t.pos(0) != 5 {
            return Err(format!("forced move landed: pos {}", t.pos(0)));
        }
        if !refused(&t, mark) {
            return Err(format!("no abnormalRefused: {:?}", t.recent_keys(20)));
        }
        frame(
            &condition,
            t.world(),
            &["pos:P0", "hand:P1", "discard:P1", "draw:P1"],
        )
    })
    .expect("C-G12-unstoppable-04 (forced)");
}

/// C-G12-unstoppable-04: 「可选择受到的…[强制停下]…效果是否生效」.
///
/// Source: `docs/rulebook/cards.json`, glossary 14 ([不可阻挡]). Design: G12
/// §5.1, §9.1.
#[test]
fn g12_unstoppable_refuses_a_received_stop() {
    forall(4, 0x6120_0006, |_rng, _i| {
        let mut t = unstoppable_victim();
        t.give(1, &["TEST:stopper"]);
        let condition = t.snapshot_world();
        let mark = t.mark();
        t.play(1, "TEST:stopper")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        if t.state(0, "TEST:stopped") != 0 {
            return Err(format!(
                "stop landed: marker {}",
                t.state(0, "TEST:stopped")
            ));
        }
        if !refused(&t, mark) {
            return Err(format!("no abnormalRefused: {:?}", t.recent_keys(20)));
        }
        frame(
            &condition,
            t.world(),
            &[
                "state:P0:",
                "hand:P1",
                "discard:P1",
                "draw:P1",
                "turn.abnormal",
            ],
        )
    })
    .expect("C-G12-unstoppable-04 (stop)");
}

/// C-G12-unstoppable-04: 「可选择受到的…」 -- the abnormal reverse rides the
/// same gate.
///
/// Source: `docs/rulebook/cards.json`, glossary 14 ([不可阻挡]). Design: G12
/// §5.1, §9.1.
#[test]
fn g12_unstoppable_refuses_a_received_reverse() {
    forall(4, 0x6120_0007, |_rng, _i| {
        let mut t = unstoppable_victim();
        t.give(1, &["TEST:reverser"]);
        let condition = t.snapshot_world();
        let mark = t.mark();
        t.play(1, "TEST:reverser")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        if t.state(0, "TEST:reversed") != 0 {
            return Err(format!(
                "reverse landed: marker {}",
                t.state(0, "TEST:reversed")
            ));
        }
        if !refused(&t, mark) {
            return Err(format!("no abnormalRefused: {:?}", t.recent_keys(20)));
        }
        frame(
            &condition,
            t.world(),
            &[
                "state:P0:",
                "hand:P1",
                "discard:P1",
                "draw:P1",
                "turn.abnormal",
            ],
        )
    })
    .expect("C-G12-unstoppable-04 (reverse)");
}

/// C-G12-unstoppable-04: 「[停留]…在适当时机依旧掉层」 -- a stay still lands
/// under [不可阻挡] (the refusal names only kinds 4-7) and still decays at
/// its owner's turn end.
///
/// Source: `docs/rulebook/cards.json`, glossary 14 ([不可阻挡]). Design: G12
/// §5.1, §6.2, §9.1.
#[test]
fn g12_unstoppable_still_lands_a_stay_layer() {
    forall(4, 0x6120_0008, |_rng, _i| {
        let mut t = unstoppable_victim();
        t.give(1, &["TEST:stayer"]);
        let mark = t.mark();
        t.play(1, "TEST:stayer")
            .map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        if t.state(0, STAY) != 1 {
            return Err(format!("stay did not land: {}", t.state(0, STAY)));
        }
        if refused(&t, mark) {
            return Err(format!("stay refused: {:?}", t.recent_keys(20)));
        }
        // The layer is TurnEnd-ticked on its owner: P1's own end leaves it.
        pass_turn(&mut t, 1);
        if t.state(0, STAY) != 1 {
            return Err(format!("stay ticked at p1's end: {}", t.state(0, STAY)));
        }
        let condition = t.snapshot_world();
        pass_turn(&mut t, 0);
        if t.state(0, STAY) != 0 {
            return Err(format!(
                "stay did not decay at its tick: {}",
                t.state(0, STAY)
            ));
        }
        frame(
            &condition,
            t.world(),
            &["state:P0:stay", "hand:P1", "draw:P1"],
        )
    })
    .expect("C-G12-unstoppable-04 (stay lands)");
}

/// C-G12-unstoppable-04: an existing layer still wears off while the holder
/// is [不可阻挡] (「在适当时机依旧掉层」).
///
/// Source: `docs/rulebook/cards.json`, glossary 14 ([不可阻挡]). Design: G12
/// §5.1, §9.1.
#[test]
fn g12_unstoppable_keeps_an_existing_layer_decaying() {
    forall(4, 0x6120_0009, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_state(0, STAY, 1);
        t.set_state(0, UNSTOPPABLE, 1);
        t.begin_turn(0);
        skip_all(&mut t);
        if t.state(0, STAY) != 1 {
            return Err(format!("stay lost on the way in: {}", t.state(0, STAY)));
        }
        let condition = t.snapshot_world();
        pass_turn(&mut t, 0);
        if t.state(0, STAY) != 0 {
            return Err(format!("stay did not decay: {}", t.state(0, STAY)));
        }
        frame(
            &condition,
            t.world(),
            &["state:P0:stay", "hand:P1", "draw:P1"],
        )
    })
    .expect("C-G12-unstoppable-04 (existing layer)");
}

/// C-G12-own-stage-02: 「选择自己的本次移动（或无法移动的回合结束时）是否
/// 触发结算」 -- the branch is the landing's blocked-ness, not its kind: a stay
/// that lands under [不可阻挡] does not hold the move, so the move answer is
/// written.
///
/// Source: `docs/rulebook/cards.json`, id `R:选择自己的舞台`. Design: G12
/// §6.4, §9.1, §9.2.
#[test]
fn g12_own_stage_takes_the_move_branch_under_unstoppable() {
    forall(4, 0x6120_000a, |_rng, _i| {
        let mut t = unstoppable_victim();
        t.set_state(0, game_core::state::key::MOVE_EXEMPT, 1);
        t.give(0, &[OWN_STAGE]);
        t.give(1, &["TEST:stayer"]);
        let condition = t.snapshot_world();
        t.play(1, "TEST:stayer")
            .map_err(|e| format!("play rejected: {e}"))?;
        if !t.counteract_offered(OWN_STAGE) {
            return Err(format!(
                "the landed stay must offer the card: {}",
                t.dump_prompt()
            ));
        }
        t.counteract(0, OWN_STAGE)
            .map_err(|e| format!("declare: {e}"))?;
        // The body asks 「是否触发结算」 -- answer yes.
        t.answer_one(0).map_err(|e| format!("settle yes: {e}"))?;
        skip_all(&mut t);
        if t.state(0, "resolveOverride") != 1 {
            return Err(format!(
                "move settle answer {}, expected 1 (the move branch)",
                t.state(0, "resolveOverride")
            ));
        }
        if t.world().blocked_settle_answer(0) != SettleAnswer::Unanswered {
            return Err(format!(
                "turn-end answer {:?}, expected Unanswered",
                t.world().blocked_settle_answer(0)
            ));
        }
        frame(
            &condition,
            t.world(),
            &[
                "state:P0:",
                "hand:P0",
                "discard:P0",
                "hand:P1",
                "discard:P1",
                "draw:P0",
                "draw:P1",
                "turn.abnormal",
            ],
        )
    })
    .expect("C-G12-own-stage-02");
}

// =====================================================================
// Item 3 -- the move span
// =====================================================================

/// Play [`HUMAN`], declare X = 1, and arm the next move's boost from two
/// crystals, stopping at the fresh turn's 运营.
fn armed_boost(t: &mut Table) -> Result<(), String> {
    t.give(0, &[HUMAN]);
    t.play(0, HUMAN).map_err(|e| format!("play: {e}"))?;
    let p = t.expect_prompt();
    let k = p
        .options
        .iter()
        .position(|o| format!("{o:?}").contains("I(1)"))
        .unwrap_or(0) as i32;
    t.answer(0, k).map_err(|e| format!("declare X: {e}"))?;
    skip_all(t);
    t.set_crystals(0, HUMAN, 2);
    t.m.world_mut().st.skip_move = true;
    t.end(0).map_err(|e| format!("end p0: {e}"))?;
    skip_all(t);
    t.begin_turn(1);
    skip_all(t);
    t.begin_turn(0);
    skip_all(t);
    if t.crystals(0, HUMAN) != Some(0) {
        return Err(format!(
            "the turn start did not consume the two crystals: {:?}",
            t.crystals(0, HUMAN)
        ));
    }
    Ok(())
}

/// C-G12-human-02: 「若该次移动过程中受到异常移动效果影响，为此卡添加两个
/// 奇迹水晶」 -- a landing before the roll is inside the move's span.
///
/// Source: `docs/rulebook/cards.json`, id `CRYCHIC:想要成为人类`. Design: G12
/// §5.5, §6.5, §9.1.
#[test]
fn g12_want_human_counts_an_abnormal_before_the_roll() {
    forall(4, 0x6120_000b, |_rng, _i| {
        let mut t = Table::vanilla(2);
        armed_boost(&mut t)?;
        // The field card stays the actor at the plan pass (before the dice).
        t.place_raw(0, "TEST:planStay");
        t.dice(&[9]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        skip_all(&mut t);
        t.end(0).map_err(|e| format!("end p0: {e}"))?;
        skip_all(&mut t);
        if t.crystals(0, HUMAN) != Some(2) {
            return Err(format!(
                "crystals {:?}, expected 2 (「该次移动过程中」 includes the plan pass)",
                t.crystals(0, HUMAN)
            ));
        }
        Ok(())
    })
    .expect("C-G12-human-02 (before the roll)");
}

/// C-G12-human-02: a landing **after** the walk is outside the move's span, so
/// the turn end adds nothing.
///
/// Source: `docs/rulebook/cards.json`, id `CRYCHIC:想要成为人类`. Design: G12
/// §5.5, §6.5, §9.1.
#[test]
fn g12_want_human_ignores_one_after_the_walk() {
    forall(4, 0x6120_000c, |_rng, _i| {
        let mut t = Table::vanilla(2);
        armed_boost(&mut t)?;
        // 「获得一层[停留]」 at the turn end -- after the walk's frame closed.
        t.place_raw(0, "TEST:endStay");
        t.dice(&[9]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        skip_all(&mut t);
        let mark = t.mark();
        t.end(0).map_err(|e| format!("end p0: {e}"))?;
        skip_all(&mut t);
        let keys = t.keys_since(mark);
        if !keys.iter().any(|k| k.ends_with("end_stay_done"))
            || !keys.iter().any(|k| k.ends_with("outcome.stay"))
        {
            return Err(format!("the end-of-turn stay did not land: {keys:?}"));
        }
        if t.crystals(0, HUMAN) != Some(0) {
            return Err(format!(
                "crystals {:?}, expected 0 (the landing is after the walk)",
                t.crystals(0, HUMAN)
            ));
        }
        Ok(())
    })
    .expect("C-G12-human-02 (after the walk)");
}

// =====================================================================
// Item 4 -- causer-aware card immunity
// =====================================================================

/// C-G12-shine-01: 「此卡不受除拥有此卡的玩家以外的玩家的效果影响」 -- an
/// effect from another player is skipped.
///
/// Source: `docs/rulebook/cards.json`, id `PP:再次闪耀`. Design: G12 §5.7,
/// §6.7, §9.1.
#[test]
fn g12_shine_again_ignores_another_players_effect() {
    forall(4, 0x6120_000d, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.give(0, &[SHINE]);
        t.play(0, SHINE).map_err(|e| format!("play shine: {e}"))?;
        t.give(1, &["TEST:immuneTouch"]);
        pass_turn(&mut t, 0);
        let condition = t.snapshot_world();
        t.play(1, "TEST:immuneTouch")
            .map_err(|e| format!("play touch: {e}"))?;
        skip_all(&mut t);
        if t.crystals(0, SHINE) != Some(0) {
            return Err(format!(
                "another player's effect landed: crystals {:?}",
                t.crystals(0, SHINE)
            ));
        }
        frame(&condition, t.world(), &["hand:P1", "discard:P1", "draw:P1"])
    })
    .expect("C-G12-shine-01 (another player)");
}

/// C-G12-shine-01: the owner's own effect still lands (「除拥有此卡的玩家
/// 以外」).
///
/// Source: `docs/rulebook/cards.json`, id `PP:再次闪耀`. Design: G12 §5.7,
/// §6.7, §9.1.
#[test]
fn g12_shine_again_accepts_its_owners() {
    forall(4, 0x6120_000e, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.give(0, &[SHINE]);
        t.play(0, SHINE).map_err(|e| format!("play shine: {e}"))?;
        t.give(0, &["TEST:immuneTouch"]);
        let condition = t.snapshot_world();
        t.play(0, "TEST:immuneTouch")
            .map_err(|e| format!("play touch: {e}"))?;
        skip_all(&mut t);
        if t.crystals(0, SHINE) != Some(1) {
            return Err(format!(
                "the owner's effect was skipped: crystals {:?}",
                t.crystals(0, SHINE)
            ));
        }
        frame(
            &condition,
            t.world(),
            &["hand:P0", "discard:P0", "draw:P0", "field:P0:"],
        )
    })
    .expect("C-G12-shine-01 (owner)");
}

// =====================================================================
// Item 5 -- dice immunity
// =====================================================================

/// C-G12-crush-01: 「打出此卡的回合任何[使用者]使用的卡包含添加骰效果的卡
/// 不受任何其他效果影响」 -- the card's own window is skipped, the body runs.
///
/// Source: `docs/rulebook/cards.json`, id
/// `RAS:（MASKING）CRUSH ON THE DRUM!!!`. Design: G12 §5.8, §6.8, §9.1.
#[test]
fn g12_crush_drum_immune_turn_skips_the_card_window() {
    forall(4, 0x6120_000f, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "佐藤益木"); // the card is 佐藤益木's
        t.set_discard(0, &["通用:GREAT"]);
        t.give(0, &[CRUSH]);
        t.give(1, &[NET_ERROR]);
        let condition = t.snapshot_world();
        let mark = t.mark();
        t.play(0, CRUSH).map_err(|e| format!("play: {e}"))?;
        if t.prompt().is_some() {
            return Err(format!("a card window opened: {}", t.dump_prompt()));
        }
        let keys = t.keys_since(mark);
        if !keys.iter().any(|k| k.contains("play_immune")) {
            return Err(format!("no immunity line: {keys:?}"));
        }
        if !keys.iter().any(|k| k.contains("crush_drum_roll")) {
            return Err(format!("the body did not run: {keys:?}"));
        }
        frame(
            &condition,
            t.world(),
            &["hand:P0", "discard:P0", "draw:P0", "field:P0:"],
        )
    })
    .expect("C-G12-crush-01 (window skipped)");
}

/// C-G12-crush-01: the flag is turn-scoped, so the next turn offers the
/// counteraction again.
///
/// Source: `docs/rulebook/cards.json`, id
/// `RAS:（MASKING）CRUSH ON THE DRUM!!!`. Design: G12 §5.8, §9.1.
#[test]
fn g12_crush_drum_immunity_does_not_outlive_the_turn() {
    forall(4, 0x6120_0010, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "佐藤益木"); // the card is 佐藤益木's
        t.give(0, &[CRUSH, NET_ERROR]);
        t.play(0, CRUSH).map_err(|e| format!("play: {e}"))?;
        skip_all(&mut t);
        pass_turn(&mut t, 0);
        t.give(1, &[ECHO]);
        t.play(1, ECHO).map_err(|e| format!("play echo: {e}"))?;
        if !t.counteract_offered(NET_ERROR) {
            return Err(format!(
                "the next turn must offer the counteraction: {}",
                t.dump_prompt()
            ));
        }
        skip_all(&mut t);
        Ok(())
    })
    .expect("C-G12-crush-01 (turn scope)");
}

/// C-G12-crush-01: a plain (non dice-adding) card still opens its window.
///
/// Source: `docs/rulebook/cards.json`, id
/// `RAS:（MASKING）CRUSH ON THE DRUM!!!`. Design: G12 §5.8, §9.1.
#[test]
fn g12_a_plain_card_still_opens_its_window() {
    forall(4, 0x6120_0011, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.give(0, &[ECHO]);
        t.give(1, &[NET_ERROR]);
        t.play(0, ECHO).map_err(|e| format!("play: {e}"))?;
        if !t.counteract_offered(NET_ERROR) {
            return Err(format!(
                "a plain play must offer the counteraction: {}",
                t.dump_prompt()
            ));
        }
        skip_all(&mut t);
        Ok(())
    })
    .expect("C-G12-crush-01 (plain card)");
}

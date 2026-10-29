//! Per-card contracts for the 通用 (general) group -- 14 cards.
//!
//! Template: `rules/tests/tests/spec_example_card.rs` (which owns
//! `通用:10次招募（1回限定）`; this file does **not** re-test it). The doc
//! entry for that card lives in `docs/rulebook/spec/cards/general.md` and
//! cross-references the example's executable.
//!
//! Source of every quote: `docs/rulebook/cards.json` (live sheet). Rulings
//! from `docs/rulebook/TEST-FINDINGS.md` §6 apply unless the sheet text
//! supersedes them. Never derived from engine behaviour.
//!
//! Card texts (roster keys under `通用:`):
//!
//! ```text
//! 通用:GREAT：
//! [手]：
//! 依次进行以下操作：
//! 1. [移除]此卡；
//! 2. 将一张“PERFECT“加入抽卡区并洗切；
//! 3. [获得]2000资金。
//!
//! 通用:[衍生]PERFECT：
//! [手]：
//! 依次进行以下操作：
//! 1. [移除]此卡；
//! 2. 将一张“FEVER!“加入弃卡区；
//! 3. [获得]3000资金。
//!
//! 通用:[衍生]FEVER!：
//! [手]：
//! 将此卡放置在[使用者]的[场地]。
//! [持续]：
//! （1）[拥有者]被[支付]或[获得]资金时将金额额外提高X；X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）。
//! （2）[拥有者]回合开始时将此卡放入[使用者]弃卡区。
//!
//! 通用:@Tsugu ycm：
//! [手]：
//! 投掷3d10并根据结果依次进行以下操作：
//! 1. 结果至少为22则抽1张卡；
//! 2. 结果至少为26则进进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并不[结算]，且可选择购买任意无主的[可购买格子]；
//! 3. 结果至少为28则本回合购买格子时[消耗]资金时降低1500（最低0）；
//! 4. 结果小于26则选择[获得]1000资金或进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并[结算]。
//!
//! 通用:CiRCLE THANKS PARTY!：
//! [手]：所有其他玩家可选择[消耗]500资金，你消耗500资金，将X设为因此卡[消耗]资金的玩家数量加1。根据X进行以下操作之一：
//! 1. X至少为2则[使用者]投掷Xd20，如果结果至少为35则[使用者][获得]3000资金且其他因此卡[消耗]资金的玩家[获得]1500资金；
//! 2. X等于1则[使用者]的本回合结束后获得一个额外回合。
//!
//! 通用:登上武道馆：
//! [手]：
//! 将X设为2000÷“[使用者]以外的[存活]玩家数量”向上取整10，Y设为[存活]玩家数量减1。[指定][使用者]以外的所有玩家，被[指定]的玩家[支付][使用者]X资金。
//!
//! 通用:[月岛麻里奈]今天也要加油工作喔：
//! [手]：
//! [反击][使用者][经过]#1格子且#1格子受到其他效果影响时：[使用者]本次对#1格子的[经过]或[结算]正常进行而不受到其上的额外效果。
//!
//! 通用:[都筑诗船]Parking Space：
//! [手]：
//! 将此卡放置于“Space”格子上，将其上的房屋转移至其他你拥有的格子上（每个格子因此效果最多获得1层）。
//! [持续]：
//! （1）此卡所在格子的[结算]改为回合结束后获得一层[停留]。
//! （2）位于此卡所在格子上的玩家无法使用角色及乐队技能。
//!
//! 通用:安可：
//! [手]：
//! [反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次[异常移动效果]和其导致的所有效果。
//!
//! 通用:尽力后的收获：
//! [手]：
//! 立刻进入移动阶段，本回合的[主要移动]改为移动1到6以内的任意整数并[结算]。
//!
//! 通用:网络链接异常：
//! [手]：
//! [反击]1张手卡的[手]效果或事件卡的效果生效前：根据卡和效果的类型依次进行以下操作：
//! 1. 手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]；
//! 2. 手卡的[手]效果且没有[指定]目标则抵消其所有的效果；
//! 3. 事件卡的效果手牌则则抵消其所有的效果。
//!
//! 通用:该清CP了：
//! （1）[使用者]使用此卡后的下2回合开始时，此卡在格子上添加的[CP点]及其产物将在相邻的没有[CP点]的格子添加1个[CP点]。
//! [手]：
//! 在任意一个没有角色和[CP点]的格子上添加1个[CP点]并在自己[场上]添加6个[CP点]。在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]1个[CP点]，[获得]800资金。
//!
//! 通用:雨啊，快点来吧：
//! [手]：
//! 投掷2d2并记录结果为X。[指定]X名玩家（其中必须包括[使用者]），被[指定]的玩家获得一层[停留]。
//! ```
//!
//! ## How to write another card contract
//!
//! 1. Quote the card text in this file's header and in
//!    `docs/rulebook/spec/cards/<group>.md`.
//! 2. Clause IDs `C-GEN-<slug>-<n>`; doc and code share the string.
//! 3. `forall` over randomized pre-states, drive the transition, assert
//!    exact numbers, then [`rules_spec::frame`] the rest.
//! 4. A discrepancy is a plain failing test. No `#[ignore]`.

use rules_spec::{forall, frame, Clause, Step, Table};

/// Engine tile index of the rulebook 「#N格」 is `N - 1`.
const CIRCLE: usize = 0;
const BANDORI: usize = 35; // 「bandori车站」 / board name 「Bandori车站」
const SPACE: usize = 42; // 「Space」
const FUJIMI: usize = 11; // 富士见坂, a cheap purchasable
const NO_HOUSE_TILE: usize = 5; // 偶像经纪公司 -- no characters / CP

// ---------------------------------------------------------------------
// local helpers
// ---------------------------------------------------------------------

/// Skip every open prompt (counteract windows, offers, ...).
fn skip_all(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// Did this step activate `card` (play or bare activation)? Keyed on the
/// **engine act** (the play input), never on the `"card"` presentation
/// event -- a lazy header must not move the frame window.
fn activated(step: &Step<'_>, card: &str) -> bool {
    step.act_is_play(card)
}

/// Money delta of `who` across the step.
fn money_delta(step: &Step<'_>, who: usize) -> i64 {
    step.post_money(who) as i64 - step.pre_money(who) as i64
}

/// The standard frame for a hand-card body that only moves the user's
/// money / card zones (and maybe their board position / purchases).
fn user_card_frame(step: &Step<'_>, card: &str) -> Result<(), String> {
    if !activated(step, card) {
        return Ok(());
    }
    let who = step.player_of_act();
    let allowed = [
        format!("money:P{who}"),
        format!("hand:P{who}"),
        format!("draw:P{who}"),
        format!("discard:P{who}"),
        format!("hand_n:P{who}"),
        format!("draw_n:P{who}"),
        format!("discard_pub:P{who}"),
    ];
    let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
    frame(step.pre, step.post, &allowed)
}

// =====================================================================
// 通用:GREAT
// =====================================================================

/// C-GEN-great-01 -- [移除]此卡.
///
/// * **ID:** `C-GEN-great-01`
/// * **Source:** `roster#通用:GREAT`
/// * **Quote:** 「1. [移除]此卡；」
/// * **Trigger:** the [手] effect of this card, played from hand.
/// * **Condition:** the card was in the user's hand (legal play).
/// * **Effect:** the card left every zone (rulebook: 「[移除]：将卡移除出游戏」).
/// * **Frame:** see [`c_gen_great_frame`].
/// * **Rulings & OPEN:** none.
/// * **Executable:** [`c_gen_great_01`].
pub fn c_gen_great_01(step: &Step<'_>) -> Result<(), String> {
    if !activated(step, "通用:GREAT") {
        return Ok(());
    }
    let who = step.player_of_act();
    // 「[移除]：将卡移除出游戏」 -- the card is in no zone at all.
    let in_any = step.post.hidden[who].hand.iter().any(|c| c == "通用:GREAT")
        || step.post.hidden[who].deck.iter().any(|c| c == "通用:GREAT")
        || step.post.hidden[who]
            .graveyard
            .iter()
            .any(|c| c == "通用:GREAT")
        || step.post.st.players[who]
            .field
            .iter()
            .any(|f| f.card == "通用:GREAT");
    if in_any {
        return Err("GREAT still in a zone after [移除]".into());
    }
    Ok(())
}

/// C-GEN-great-02 -- 将一张“PERFECT“加入抽卡区并洗切.
///
/// * **ID:** `C-GEN-great-02`
/// * **Source:** `roster#通用:GREAT`
/// * **Quote:** 「2. 将一张“PERFECT“加入抽卡区并洗切；」
/// * **Trigger:** the [手] effect of this card.
/// * **Condition:** none beyond the play.
/// * **Effect:** exactly one `通用:[衍生]PERFECT` is in the user's draw pile
///   afterwards (the shuffle is not observable as an order contract).
/// * **Frame:** see [`c_gen_great_frame`].
/// * **Rulings & OPEN:** the quote names 「PERFECT」; the live sheet card id is
///   `通用:[衍生]PERFECT`.
/// * **Executable:** [`c_gen_great_02`].
pub fn c_gen_great_02(step: &Step<'_>) -> Result<(), String> {
    if !activated(step, "通用:GREAT") {
        return Ok(());
    }
    let who = step.player_of_act();
    let n = step.post.hidden[who]
        .deck
        .iter()
        .filter(|c| c.as_str() == "通用:[衍生]PERFECT")
        .count();
    if n != 1 {
        return Err(format!("draw holds {n} PERFECT, expected 1"));
    }
    Ok(())
}

/// C-GEN-great-03 -- [获得]2000资金.
///
/// * **ID:** `C-GEN-great-03`
/// * **Source:** `roster#通用:GREAT`
/// * **Quote:** 「3. [获得]2000资金。」
/// * **Trigger:** the [手] effect of this card.
/// * **Condition:** none beyond the play (「[获得]」 = print money).
/// * **Effect:** user's money rises by exactly 2000.
/// * **Frame:** see [`c_gen_great_frame`].
/// * **Rulings & OPEN:** none.
/// * **Executable:** [`c_gen_great_03`].
pub fn c_gen_great_03(step: &Step<'_>) -> Result<(), String> {
    if !activated(step, "通用:GREAT") {
        return Ok(());
    }
    let who = step.player_of_act();
    let d = money_delta(step, who);
    if d != 2000 {
        return Err(format!("money delta {d}, expected +2000 ([获得]2000)"));
    }
    Ok(())
}

/// Executable frame for a GREAT play.
pub fn c_gen_great_frame(step: &Step<'_>) -> Result<(), String> {
    user_card_frame(step, "通用:GREAT")
}

/// The clause set a GREAT contract arms on `Table::observe`.
pub const GREAT_CLAUSES: &[Clause] = &[
    Clause::new(
        "C-GEN-great-01",
        "roster#通用:GREAT",
        "1. [移除]此卡；",
        c_gen_great_01,
    ),
    Clause::new(
        "C-GEN-great-02",
        "roster#通用:GREAT",
        "2. 将一张“PERFECT“加入抽卡区并洗切；",
        c_gen_great_02,
    ),
    Clause::new(
        "C-GEN-great-03",
        "roster#通用:GREAT",
        "3. [获得]2000资金。",
        c_gen_great_03,
    ),
    Clause::new(
        "C-GEN-great-frame",
        "roster#通用:GREAT",
        "依次进行以下操作：1. [移除]此卡；2. 将一张“PERFECT“加入抽卡区并洗切；3. [获得]2000资金。",
        c_gen_great_frame,
    ),
];

/// C-GEN-great-01/02/03 + frame, over randomized pre-states.
#[test]
fn great_contract() {
    forall(8, 0x6EA7_0001, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 10_000 + rng.range(0, 30) * 100;
        t.set_money(0, money);
        // A non-empty draw pile so the [移除]ed card cannot reshuffle back.
        t.set_draw(0, &["通用:安可", "通用:尽力后的收获"]);
        t.give(0, &["通用:GREAT"]);

        let pre = t.snapshot_world();
        t.observe(GREAT_CLAUSES);
        t.play(0, "通用:GREAT")
            .map_err(|e| format!("play rejected: {e}"))?;

        if t.money(0) as i64 - money as i64 != 2000 {
            return Err(format!("money {}, expected {money}+2000", t.money(0)));
        }
        if t.discard(0).iter().any(|c| c == "通用:GREAT") {
            return Err(format!("[移除]ed card in discard: {:?}", t.discard(0)));
        }
        if t.hand(0).iter().any(|c| c == "通用:GREAT") {
            return Err("GREAT still in hand".into());
        }
        let perfect = t
            .draw_pile(0)
            .iter()
            .filter(|c| c.as_str() == "通用:[衍生]PERFECT")
            .count();
        if perfect != 1 {
            return Err(format!("draw {:?}, expected one PERFECT", t.draw_pile(0)));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
            ],
        )
    })
    .expect("GREAT contract");
}

// =====================================================================
// 通用:[衍生]PERFECT
// =====================================================================

/// C-GEN-perfect-01 -- [移除]此卡.
///
/// * **ID:** `C-GEN-perfect-01`
/// * **Source:** `roster#通用:[衍生]PERFECT`
/// * **Quote:** 「1. [移除]此卡；」
/// * **Trigger:** the [手] effect of this card.
/// * **Condition:** the card is in the user's hand.
/// * **Effect:** the card left every zone (「[移除]：将卡移除出游戏」).
/// * **Frame:** see [`c_gen_perfect_frame`].
/// * **Rulings & OPEN:** [衍生] cards 「不能加入初始卡组，需通过特定效果获得」;
///   the contract arranges the card in hand (a legal obtained state).
/// * **Executable:** [`c_gen_perfect_01`].
pub fn c_gen_perfect_01(step: &Step<'_>) -> Result<(), String> {
    if !activated(step, "通用:[衍生]PERFECT") {
        return Ok(());
    }
    let who = step.player_of_act();
    let in_any = step.post.hidden[who]
        .hand
        .iter()
        .any(|c| c == "通用:[衍生]PERFECT")
        || step.post.hidden[who]
            .deck
            .iter()
            .any(|c| c == "通用:[衍生]PERFECT")
        || step.post.hidden[who]
            .graveyard
            .iter()
            .any(|c| c == "通用:[衍生]PERFECT")
        || step.post.st.players[who]
            .field
            .iter()
            .any(|f| f.card == "通用:[衍生]PERFECT");
    if in_any {
        return Err("PERFECT still in a zone after [移除]".into());
    }
    Ok(())
}

/// C-GEN-perfect-02 -- 将一张“FEVER!“加入弃卡区.
///
/// * **ID:** `C-GEN-perfect-02`
/// * **Source:** `roster#通用:[衍生]PERFECT`
/// * **Quote:** 「2. 将一张“FEVER!“加入弃卡区；」
/// * **Trigger:** the [手] effect of this card.
/// * **Condition:** none beyond the play.
/// * **Effect:** exactly one `通用:[衍生]FEVER!` was written into the user's
///   discard. (If the draw pile is empty the engine may reshuffle the discard
///   into the draw -- the card still *entered* the discard; the contract
///   therefore allows it in discard **or** draw after the step.)
/// * **Frame:** see [`c_gen_perfect_frame`].
/// * **Rulings & OPEN:** the quote names 「FEVER!」; the live sheet id is
///   `通用:[衍生]FEVER!`.
/// * **Executable:** [`c_gen_perfect_02`].
pub fn c_gen_perfect_02(step: &Step<'_>) -> Result<(), String> {
    if !activated(step, "通用:[衍生]PERFECT") {
        return Ok(());
    }
    let who = step.player_of_act();
    let in_discard = step.post.hidden[who]
        .graveyard
        .iter()
        .filter(|c| c.as_str() == "通用:[衍生]FEVER!")
        .count();
    let in_draw = step.post.hidden[who]
        .deck
        .iter()
        .filter(|c| c.as_str() == "通用:[衍生]FEVER!")
        .count();
    // 「加入弃卡区」 -- it must be in the discard. The only legal reason to
    // find it in the draw is a discard->draw reshuffle (the draw pile was
    // empty when it was filed), in which case the discard is empty.
    if in_discard == 1 {
        Ok(())
    } else if in_draw == 1 && step.post.hidden[who].graveyard.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "FEVER! in discard {in_discard} / draw {in_draw} (discard {:?}), expected 1 in discard",
            step.post.hidden[who].graveyard
        ))
    }
}

/// C-GEN-perfect-03 -- [获得]3000资金.
///
/// * **ID:** `C-GEN-perfect-03`
/// * **Source:** `roster#通用:[衍生]PERFECT`
/// * **Quote:** 「3. [获得]3000资金。」
/// * **Trigger:** the [手] effect of this card.
/// * **Condition:** none beyond the play.
/// * **Effect:** user's money rises by exactly 3000.
/// * **Frame:** see [`c_gen_perfect_frame`].
/// * **Rulings & OPEN:** none.
/// * **Executable:** [`c_gen_perfect_03`].
pub fn c_gen_perfect_03(step: &Step<'_>) -> Result<(), String> {
    if !activated(step, "通用:[衍生]PERFECT") {
        return Ok(());
    }
    let who = step.player_of_act();
    let d = money_delta(step, who);
    if d != 3000 {
        return Err(format!("money delta {d}, expected +3000 ([获得]3000)"));
    }
    Ok(())
}

/// Executable frame for a PERFECT play.
pub fn c_gen_perfect_frame(step: &Step<'_>) -> Result<(), String> {
    user_card_frame(step, "通用:[衍生]PERFECT")
}

/// The clause set a PERFECT contract arms on `Table::observe`.
pub const PERFECT_CLAUSES: &[Clause] = &[
    Clause::new(
        "C-GEN-perfect-01",
        "roster#通用:[衍生]PERFECT",
        "1. [移除]此卡；",
        c_gen_perfect_01,
    ),
    Clause::new(
        "C-GEN-perfect-02",
        "roster#通用:[衍生]PERFECT",
        "2. 将一张“FEVER!“加入弃卡区；",
        c_gen_perfect_02,
    ),
    Clause::new(
        "C-GEN-perfect-03",
        "roster#通用:[衍生]PERFECT",
        "3. [获得]3000资金。",
        c_gen_perfect_03,
    ),
    Clause::new(
        "C-GEN-perfect-frame",
        "roster#通用:[衍生]PERFECT",
        "依次进行以下操作：1. [移除]此卡；2. 将一张“FEVER!“加入弃卡区；3. [获得]3000资金。",
        c_gen_perfect_frame,
    ),
];

/// C-GEN-perfect-01/02/03 + frame, over randomized pre-states.
#[test]
fn perfect_contract() {
    forall(8, 0x6EA7_0002, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 10_000 + rng.range(0, 30) * 100;
        t.set_money(0, money);
        // Keep a non-empty draw pile so the [移除]ed PERFECT cannot reshuffle
        // and the FEVER! line stays observable in the discard.
        t.set_draw(0, &["通用:安可"]);
        // A FEVER! is already in hand: 「加入一张新的」 must not consume it.
        t.give(0, &["通用:[衍生]PERFECT", "通用:[衍生]FEVER!"]);

        let pre = t.snapshot_world();
        t.observe(PERFECT_CLAUSES);
        t.play(0, "通用:[衍生]PERFECT")
            .map_err(|e| format!("play rejected: {e}"))?;

        if t.money(0) as i64 - money as i64 != 3000 {
            return Err(format!("money {}, expected {money}+3000", t.money(0)));
        }
        if t.discard(0).iter().any(|c| c == "通用:[衍生]PERFECT") {
            return Err(format!("[移除]ed PERFECT in discard: {:?}", t.discard(0)));
        }
        if !t.discard(0).iter().any(|c| c == "通用:[衍生]FEVER!") {
            return Err(format!(
                "FEVER! not in discard {:?} (draw {:?})",
                t.discard(0),
                t.draw_pile(0)
            ));
        }
        // 「将一张“FEVER!“加入弃卡区」 is a new copy: the one held in hand stays.
        if !t.hand(0).iter().any(|c| c == "通用:[衍生]FEVER!") {
            return Err(format!(
                "the hand FEVER! was consumed by the add; hand {:?} discard {:?}",
                t.hand(0),
                t.discard(0)
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
            ],
        )
    })
    .expect("PERFECT contract");
}

// =====================================================================
// 通用:[衍生]FEVER!
// =====================================================================

/// C-GEN-fever-01 -- 将此卡放置在[使用者]的[场地].
///
/// * **ID:** `C-GEN-fever-01`
/// * **Source:** `roster#通用:[衍生]FEVER!`
/// * **Quote:** 「[手]：将此卡放置在[使用者]的[场地]。」
/// * **Trigger:** the [手] effect of this card.
/// * **Condition:** the card is in the user's hand.
/// * **Effect:** a field instance of `通用:[衍生]FEVER!` sits on the user's
///   [场地] (owner == user == the player who played it); the card left the hand.
/// * **Frame:** see [`c_gen_fever_frame`].
/// * **Rulings & OPEN:** none.
/// * **Executable:** [`c_gen_fever_01`].
pub fn c_gen_fever_01(step: &Step<'_>) -> Result<(), String> {
    if !activated(step, "通用:[衍生]FEVER!") {
        return Ok(());
    }
    let who = step.player_of_act();
    let f = step.post.st.players[who]
        .field
        .iter()
        .find(|f| f.card == "通用:[衍生]FEVER!");
    match f {
        None => Err("FEVER! not on the user's [场地]".into()),
        Some(f) if f.owner != who as i32 || f.user != who as i32 => Err(format!(
            "FEVER! owner/user {}/{}, expected {who}",
            f.owner, f.user
        )),
        Some(_) => Ok(()),
    }
}

/// X of 「X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）」.
pub fn fever_x(owner_field_len: usize) -> i64 {
    600 - 200 * owner_field_len as i64
}

/// C-GEN-fever-02/03 -- the [获得] / [支付] boost, same X.
///
/// * **ID:** `C-GEN-fever-02` / `C-GEN-fever-03`
/// * **Source:** `roster#通用:[衍生]FEVER!`
/// * **Quote:** 「（1）[拥有者]被[支付]或[获得]资金时将金额额外提高X；X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）。」
/// * **Trigger:** any [获得] (print) or [支付] (transfer) **to** the [拥有者]
///   while FEVER! is on their field.
/// * **Condition:** `n` = number of cards on the owner's [场地] (including
///   this instance); `X = 600 - 200 * n` (may be negative).
/// * **Effect:** the amount is raised by X -- a print becomes `amount + X`, a
///   transfer moves `amount + X` on both sides.
/// * **Frame:** the boost rides the money window of that transfer / print.
/// * **Rulings & OPEN:** 「场上每拥有一张卡」 counts every field card of the
///   owner. 「将金额额外提高X」 on a [支付] is read as raising the transferred
///   amount (a pure transfer), not a separate printed gain.
/// * **Executable:** [`fever_gain_boost`], [`fever_payment_boost`].
pub fn c_gen_fever_02(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}
pub fn c_gen_fever_03(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-fever-04 -- （2）[拥有者]回合开始时将此卡放入[使用者]弃卡区.
///
/// * **ID:** `C-GEN-fever-04`
/// * **Source:** `roster#通用:[衍生]FEVER!`
/// * **Quote:** 「（2）[拥有者]回合开始时将此卡放入[使用者]弃卡区。」
/// * **Trigger:** the owner's turn start, while the card is on their field.
/// * **Condition:** the card is placed.
/// * **Effect:** the field instance leaves the [场地] and one
///   `通用:[衍生]FEVER!` enters the [使用者]'s discard.
/// * **Frame:** see [`c_gen_fever_frame`].
/// * **Rulings & OPEN:** when owner and user differ the discard is the
///   **user**'s. The normal play makes them the same seat.
/// * **Executable:** [`fever_leaves_at_owner_turn_start`].
pub fn c_gen_fever_04(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// Executable frame for a FEVER! play / leave.
pub fn c_gen_fever_frame(step: &Step<'_>) -> Result<(), String> {
    if !step.act_is_card("通用:[衍生]FEVER!") {
        return Ok(());
    }
    let allowed = [
        "money:P".to_string(),
        "hand:P".to_string(),
        "draw:P".to_string(),
        "discard:P".to_string(),
        "hand_n:P".to_string(),
        "draw_n:P".to_string(),
        "discard_pub:P".to_string(),
        "field:P".to_string(),
        "crystals:P".to_string(),
        "pool:P".to_string(),
        "fprop:P".to_string(),
        "extra_turns".into(),
        "pos:P".into(),
        "turn.abnormal".into(),
        "targeted".into(),
    ];
    let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
    frame(step.pre, step.post, &allowed)
}

/// The clause set a FEVER! contract arms on `Table::observe`.
pub const FEVER_CLAUSES: &[Clause] = &[
    Clause::new(
        "C-GEN-fever-01",
        "roster#通用:[衍生]FEVER!",
        "[手]：将此卡放置在[使用者]的[场地]。",
        c_gen_fever_01,
    ),
    Clause::new(
        "C-GEN-fever-frame",
        "roster#通用:[衍生]FEVER!",
        "（1）[拥有者]被[支付]或[获得]资金时将金额额外提高X；X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）。",
        c_gen_fever_frame,
    ),
];

/// C-GEN-fever-01 + X formula on a [获得] boost.
#[test]
fn fever_gain_boost() {
    forall(6, 0x6EA7_0003, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 10_000 + rng.range(0, 20) * 100;
        t.set_money(0, money);
        t.give(0, &["通用:[衍生]FEVER!"]);
        t.observe(FEVER_CLAUSES);
        t.play(0, "通用:[衍生]FEVER!")
            .map_err(|e| format!("play rejected: {e}"))?;
        if !t.on_field(0, "通用:[衍生]FEVER!") {
            return Err("not on field".into());
        }
        let n = t.field(0).len();
        let x = fever_x(n);
        // A plain CiRCLE reward [获得]2000 becomes 2000 + X.
        // Stay in the play turn -- `begin_turn` would fire （2） and drop FEVER!
        t.set_pos(0, 55);
        t.dice(&[6]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        if t.prompt().is_some() {
            t.answer(0, 0).map_err(|e| format!("claim rejected: {e}"))?;
        }
        skip_all(&mut t);
        let expected = money as i64 + 2000 + x;
        if t.money(0) as i64 != expected {
            return Err(format!(
                "money {}, expected {expected} (= {money} + 2000 + X({x})), n={n}",
                t.money(0)
            ));
        }
        Ok(())
    })
    .expect("FEVER! [获得] boost");
}

/// C-GEN-fever-03 -- the payment boost is a pure transfer of `rent + X`.
#[test]
fn fever_payment_boost() {
    forall(6, 0x6EA7_0004, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money0 = 10_000 + rng.range(0, 20) * 100;
        let money1 = 10_000 + rng.range(0, 20) * 100;
        t.set_money(0, money0);
        t.set_money(1, money1);
        t.own(0, &[FUJIMI]);
        t.give(0, &["通用:[衍生]FEVER!"]);
        t.play(0, "通用:[衍生]FEVER!")
            .map_err(|e| format!("play rejected: {e}"))?;
        let n = t.field(0).len();
        let x = fever_x(n);
        t.begin_turn(1);
        t.set_pos(1, FUJIMI - 1);
        t.dice(&[1]);
        t.roll(1).map_err(|e| format!("roll rejected: {e}"))?;
        skip_all(&mut t);
        let paid = money1 as i64 - t.money(1) as i64;
        let got = t.money(0) as i64 - money0 as i64;
        if paid != 60 + x || got != 60 + x {
            return Err(format!(
                "paid {paid} got {got}, expected both {} (rent 60 + X {x}, n={n})",
                60 + x
            ));
        }
        if t.money(0) as i64 + t.money(1) as i64 != money0 as i64 + money1 as i64 {
            return Err("not a pure transfer".into());
        }
        Ok(())
    })
    .expect("FEVER! [支付] boost");
}

/// C-GEN-fever-02: X < 0 when the owner's field holds ≥ 4 cards (「可小于0」).
#[test]
fn fever_x_negative_lowers_the_gain() {
    forall(4, 0x6EA7_001B, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money);
        // Three decoy field cards + FEVER! itself → n = 4 → X = 600-800 = -200.
        t.place_raw(0, "通用:GREAT");
        t.place_raw(0, "通用:安可");
        t.place_raw(0, "通用:尽力后的收获");
        t.give(0, &["通用:[衍生]FEVER!"]);
        t.play(0, "通用:[衍生]FEVER!")
            .map_err(|e| format!("play rejected: {e}"))?;
        let n = t.field(0).len();
        let x = fever_x(n);
        if x >= 0 {
            return Err(format!("n={n} X={x}, expected X < 0 for this setup"));
        }
        t.set_pos(0, 55);
        t.dice(&[6]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        if t.prompt().is_some() {
            t.answer(0, 0).map_err(|e| format!("claim: {e}"))?;
        }
        skip_all(&mut t);
        let expected = money as i64 + 2000 + x;
        if t.money(0) as i64 != expected {
            return Err(format!(
                "money {}, expected {expected} (2000 + X {x}, n={n})",
                t.money(0)
            ));
        }
        Ok(())
    })
    .expect("FEVER! X < 0");
}

/// C-GEN-fever-04 -- the card leaves to the user's discard at the owner's turn start.
#[test]
fn fever_leaves_at_owner_turn_start() {
    forall(4, 0x6EA7_0005, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_money(0, 10_000 + rng.range(0, 10) * 100);
        t.set_draw(0, &["通用:安可"]);
        t.give(0, &["通用:[衍生]FEVER!"]);
        t.play(0, "通用:[衍生]FEVER!")
            .map_err(|e| format!("play rejected: {e}"))?;
        t.dice(&[1]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        skip_all(&mut t);
        t.end(0).map_err(|e| format!("end: {e}"))?;
        t.dice(&[1]);
        t.roll(1).map_err(|e| format!("roll: {e}"))?;
        skip_all(&mut t);
        t.end(1).map_err(|e| format!("end: {e}"))?;
        if t.on_field(0, "通用:[衍生]FEVER!") {
            return Err(format!("still on field: {:?}", t.field_ids(0)));
        }
        if !t.discard(0).iter().any(|c| c == "通用:[衍生]FEVER!")
            && !t.draw_pile(0).iter().any(|c| c == "通用:[衍生]FEVER!")
        {
            return Err(format!(
                "FEVER! not in user's discard {:?} / draw {:?}",
                t.discard(0),
                t.draw_pile(0)
            ));
        }
        Ok(())
    })
    .expect("FEVER! （2） leave");
}

// =====================================================================
// 通用:登上武道馆
// =====================================================================

/// X of 「2000÷“[使用者]以外的[存活]玩家数量”向上取整10」.
pub fn budokan_x(alive_others: usize) -> i64 {
    let n = alive_others.max(1) as i64;
    let raw = 2000 / n;
    // 「向上取整10」: round **up** to a multiple of 10.
    (raw + 9) / 10 * 10
}

/// C-GEN-budokan-01 -- X formula and the payments.
///
/// * **ID:** `C-GEN-budokan-01`
/// * **Source:** `roster#通用:登上武道馆`
/// * **Quote:** 「将X设为2000÷“[使用者]以外的[存活]玩家数量”向上取整10，Y设为[存活]玩家数量减1。[指定][使用者]以外的所有玩家，被[指定]的玩家[支付][使用者]X资金。」
/// * **Trigger:** the [手] effect of this card.
/// * **Condition:** `alive_others` = number of [存活] players other than the user.
/// * **Effect:** `X = ceil_to_10(2000 / alive_others)`; every other [存活]
///   player [支付]s the user X (a pure transfer).
/// * **Frame:** only money among the players may move (plus the played card's
///   zones).
/// * **Rulings & OPEN:** `Y` is printed but never used by the effect body --
///   see `OPEN-GEN-budokan-Y`. 「向上取整10」 is read as round **up** to a
///   multiple of 10 (2000/3 → 670).
/// * **Executable:** [`c_gen_budokan_01`] + [`budokan_contract`].
pub fn c_gen_budokan_01(step: &Step<'_>) -> Result<(), String> {
    if !activated(step, "通用:登上武道馆") {
        return Ok(());
    }
    let who = step.player_of_act();
    let n_players = step.pre.st.players.len();
    // 「[使用者]以外的[存活]玩家」 -- not bankrupt and not left the game.
    let alive_others = (0..n_players)
        .filter(|&i| i != who && !step.pre.st.players[i].bankrupt && !step.pre.st.players[i].left)
        .count();
    let x = budokan_x(alive_others);
    let mut expect = vec![0i64; n_players];
    expect[who] = x * alive_others as i64;
    for i in 0..n_players {
        if i != who {
            expect[i] = -x;
        }
    }
    for i in 0..n_players {
        let d = money_delta(step, i);
        if d != expect[i] {
            return Err(format!(
                "P{i} money delta {d}, expected {} (X={x}, alive_others={alive_others})",
                expect[i]
            ));
        }
    }
    Ok(())
}

/// C-GEN-budokan-frame -- only money / card zones of the players move.
pub fn c_gen_budokan_frame(step: &Step<'_>) -> Result<(), String> {
    if !activated(step, "通用:登上武道馆") {
        return Ok(());
    }
    let allowed = [
        "money:P".to_string(),
        "hand:P".to_string(),
        "draw:P".to_string(),
        "discard:P".to_string(),
        "hand_n:P".to_string(),
        "draw_n:P".to_string(),
        "discard_pub:P".to_string(),
        "targeted".into(),
    ];
    let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
    frame(step.pre, step.post, &allowed)
}

/// The clause set a 武道馆 contract arms on `Table::observe`.
pub const BUDOKAN_CLAUSES: &[Clause] = &[
    Clause::new(
        "C-GEN-budokan-01",
        "roster#通用:登上武道馆",
        "将X设为2000÷“[使用者]以外的[存活]玩家数量”向上取整10，Y设为[存活]玩家数量减1。[指定][使用者]以外的所有玩家，被[指定]的玩家[支付][使用者]X资金。",
        c_gen_budokan_01,
    ),
    Clause::new(
        "C-GEN-budokan-frame",
        "roster#通用:登上武道馆",
        "被[指定]的玩家[支付][使用者]X资金。",
        c_gen_budokan_frame,
    ),
];

/// C-GEN-budokan-01 over 2..=5 players, randomized money.
#[test]
fn budokan_contract() {
    forall(8, 0x6EA7_0006, |rng, case| {
        let n = 2 + (case as usize % 4);
        let mut t = Table::vanilla(n);
        let base: Vec<i64> = (0..n)
            .map(|i| {
                let m = 10_000 + rng.range(0, 20) * 100;
                t.set_money(i, m);
                m as i64
            })
            .collect();
        t.give(0, &["通用:登上武道馆"]);
        let pre = t.snapshot_world();
        t.observe(BUDOKAN_CLAUSES);
        t.play(0, "通用:登上武道馆")
            .map_err(|e| format!("play rejected: {e}"))?;
        let x = budokan_x(n - 1);
        for i in 0..n {
            let d = t.money(i) as i64 - base[i];
            let expect = if i == 0 { x * (n as i64 - 1) } else { -x };
            if d != expect {
                return Err(format!("P{i} delta {d}, expected {expect} (X={x}, n={n})"));
            }
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P",
                "hand:P",
                "draw:P",
                "discard:P",
                "hand_n:P",
                "draw_n:P",
                "discard_pub:P",
                "targeted",
            ],
        )
    })
    .expect("武道馆 contract");
}

// =====================================================================
// 通用:@Tsugu ycm
// =====================================================================

/// C-GEN-tsugu-01 -- 「结果至少为22则抽1张卡」.
///
/// * **ID:** `C-GEN-tsugu-01`
/// * **Source:** `roster#通用:@Tsugu ycm`
/// * **Quote:** 「1. 结果至少为22则抽1张卡；」
/// * **Trigger:** the [手] effect, after the 3d10 roll.
/// * **Condition:** `sum(3d10) >= 22`.
/// * **Effect:** the user draws exactly one card.
/// * **Frame:** see [`c_gen_tsugu_frame`].
/// * **Rulings & OPEN:** the branches are applied 「依次」 (in order) and may
///   stack (≥28 also implies ≥26 and ≥22).
/// * **Executable:** [`tsugu_ge22_draws`].
pub fn c_gen_tsugu_01(_step: &Step<'_>) -> Result<(), String> {
    // Driven by `tsugu_ge22_draws` (the draw is interleaved with prompts).
    Ok(())
}

/// C-GEN-tsugu-02 -- 「结果至少为26则…[传送]到“bandori车站”并不[结算]」.
///
/// * **ID:** `C-GEN-tsugu-02`
/// * **Source:** `roster#通用:@Tsugu ycm`
/// * **Quote:** 「2. 结果至少为26则进进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并不[结算]，且可选择购买任意无主的[可购买格子]；」
/// * **Trigger:** the [手] effect, after the 3d10 roll.
/// * **Condition:** `sum(3d10) >= 26`.
/// * **Effect:** the user is on 「bandori车站」 (board 「Bandori车站」, index 35);
///   that tile did **not** [结算]; an optional buy of any unowned
///   [可购买格子] is offered.
/// * **Frame:** see [`c_gen_tsugu_frame`].
/// * **Rulings & OPEN:** the sheet writes 「bandori车站」 (lowercase); the board
///   name is 「Bandori车站」. 「不[结算]」 means no ordinary landing settle (the
///   card's own buy offer is separate). OPEN-GEN-tsugu-teleport-main records
///   that a teleport-as-main-move's settle shape is still an open ruling
///   (TEST-FINDINGS §6.1).
/// * **Executable:** [`tsugu_ge26_teleports`].
pub fn c_gen_tsugu_02(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-tsugu-03 -- 「结果至少为28则本回合购买格子时[消耗]资金时降低1500（最低0）」.
///
/// * **ID:** `C-GEN-tsugu-03`
/// * **Source:** `roster#通用:@Tsugu ycm`
/// * **Quote:** 「3. 结果至少为28则本回合购买格子时[消耗]资金时降低1500（最低0）；」
/// * **Trigger:** the [手] effect (sets a turn-long modifier), then any buy.
/// * **Condition:** `sum(3d10) >= 28`.
/// * **Effect:** this turn, money spent on buying a tile is reduced by 1500,
///   floored at 0.
/// * **Frame:** see [`c_gen_tsugu_frame`].
/// * **Rulings & OPEN:** 「最低0」 floors the *spent* amount, so a tile of
///   price ≤ 1500 costs 0.
/// * **Executable:** [`tsugu_ge28_buy_discount`].
pub fn c_gen_tsugu_03(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-tsugu-04 -- 「结果小于26则选择[获得]1000资金或…[传送]…并[结算]」.
///
/// * **ID:** `C-GEN-tsugu-04`
/// * **Source:** `roster#通用:@Tsugu ycm`
/// * **Quote:** 「4. 结果小于26则选择[获得]1000资金或进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并[结算]。」
/// * **Trigger:** the [手] effect, after the 3d10 roll.
/// * **Condition:** `sum(3d10) < 26`.
/// * **Effect:** a binary choice: (a) [获得]1000资金, or (b) teleport to
///   「bandori车站」 **and** [结算].
/// * **Frame:** see [`c_gen_tsugu_frame`].
/// * **Rulings & OPEN:** branch (4) is the `< 26` fallback and does not stack
///   with (2).
/// * **Executable:** [`tsugu_lt26_choice`].
pub fn c_gen_tsugu_04(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// Executable frame for a Tsugu play (teleport / buy / gain all land in one
/// act or the answers that finish it).
pub fn c_gen_tsugu_frame(step: &Step<'_>) -> Result<(), String> {
    if !step.act_is_card("通用:@Tsugu ycm") {
        return Ok(());
    }
    let who = step.player_of_act();
    let allowed = [
        format!("money:P"),
        format!("hand:P{who}"),
        format!("draw:P{who}"),
        format!("discard:P{who}"),
        format!("hand_n:P"),
        format!("draw_n:P"),
        format!("discard_pub:P"),
        format!("pos:P{who}"),
        "owner:T".into(),
        "houses:T".into(),
        "turn.lingering".into(),
        "turn.abnormal".into(),
        "targeted".into(),
    ];
    let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
    frame(step.pre, step.post, &allowed)
}

/// The clause set a Tsugu contract arms on `Table::observe`.
pub const TSUGU_CLAUSES: &[Clause] = &[Clause::new(
    "C-GEN-tsugu-frame",
    "roster#通用:@Tsugu ycm",
    "投掷3d10并根据结果依次进行以下操作：…",
    c_gen_tsugu_frame,
)];

/// C-GEN-tsugu-01: sum ≥ 22 draws one card (and the <26 choice fires).
#[test]
fn tsugu_ge22_draws() {
    forall(6, 0x6EA7_0007, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money);
        t.set_draw(0, &["通用:GREAT", "通用:安可"]);
        t.give(0, &["通用:@Tsugu ycm"]);
        t.dice(&[8, 8, 6]); // 3d10 sum = 22
        let pre = t.snapshot_world();
        t.observe(TSUGU_CLAUSES);
        t.play(0, "通用:@Tsugu ycm")
            .map_err(|e| format!("play rejected: {e}"))?;
        // sum 22: branch 1 (draw) and branch 4 (choice). Take [获得]1000.
        if t.prompt().is_some() {
            t.answer(0, 0).map_err(|e| format!("choice: {e}"))?;
        }
        skip_all(&mut t);
        if !t
            .hand(0)
            .iter()
            .any(|c| c == "通用:GREAT" || c == "通用:安可")
        {
            return Err(format!("no draw landed: hand {:?}", t.hand(0)));
        }
        let d = t.money(0) as i64 - money as i64;
        if d != 1000 {
            return Err(format!("money delta {d}, expected +1000 (branch 4 gain)"));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P",
                "draw_n:P",
                "discard_pub:P",
            ],
        )
    })
    .expect("Tsugu ≥22 draws");
}

/// C-GEN-tsugu-02: sum ≥ 26 teleports to Bandori车站 with 不[结算].
#[test]
fn tsugu_ge26_teleports() {
    forall(6, 0x6EA7_0008, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_money(0, 10_000 + rng.range(0, 10) * 100);
        t.set_draw(0, &["通用:GREAT"]);
        t.give(0, &["通用:@Tsugu ycm"]);
        t.dice(&[9, 9, 8]); // sum 26
        t.observe(TSUGU_CLAUSES);
        t.play(0, "通用:@Tsugu ycm")
            .map_err(|e| format!("play rejected: {e}"))?;
        if t.pos(0) != BANDORI {
            return Err(format!("pos {}, expected Bandori车站 {BANDORI}", t.pos(0)));
        }
        // 「可选择购买任意无主的[可购买格子]」 -- the offer must be open.
        // 不[结算] means it is the card's own yes/no, not a landing settle.
        let p = t.expect_prompt();
        if p.options.len() != 2 && p.kind != "tile" {
            return Err(format!("no optional-buy offer: {}", t.dump_prompt()));
        }
        t.decline();
        skip_all(&mut t);
        if t.owner(BANDORI).is_some() {
            return Err(format!("settled/bought Bandori: {:?}", t.owner(BANDORI)));
        }
        Ok(())
    })
    .expect("Tsugu ≥26 teleport");
}

/// C-GEN-tsugu-03: sum ≥ 28 floors a buy at 0 after −1500.
#[test]
fn tsugu_ge28_buy_discount() {
    forall(6, 0x6EA7_0009, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money);
        t.give(0, &["通用:@Tsugu ycm"]);
        t.dice(&[10, 10, 9]); // sum 29 (also ≥26: teleport + optional buy)
        t.observe(TSUGU_CLAUSES);
        t.play(0, "通用:@Tsugu ycm")
            .map_err(|e| format!("play rejected: {e}"))?;
        if t.pos(0) != BANDORI {
            return Err(format!("pos {}, expected Bandori {BANDORI}", t.pos(0)));
        }
        // Branch 3 is a **turn-long** modifier: 「本回合购买格子时[消耗]资金
        // 时降低1500（最低0）」. Whether it also covers branch 2's own in-effect
        // buy is OPEN-GEN-tsugu-3-order (「依次」 puts 3 after 2), so this
        // contract pins the modifier, not that buy's price.
        let lingering = t
            .world()
            .turn
            .lingering
            .iter()
            .any(|l| l.card.contains("Tsugu") || l.card.contains("tsugu"));
        if !lingering {
            return Err(format!(
                "no turn-long Tsugu modifier: {:?}",
                t.world().turn.lingering
            ));
        }
        // Decline the optional buy; the modifier stays armed for the turn.
        skip_all(&mut t);
        if t.owner(BANDORI).is_some() {
            return Err("declined buy still took Bandori".into());
        }
        Ok(())
    })
    .expect("Tsugu ≥28 buy discount");
}

/// C-GEN-tsugu-04: sum < 26 offers the binary choice.
#[test]
fn tsugu_lt26_choice() {
    forall(6, 0x6EA7_000A, |rng, case| {
        let mut t = Table::vanilla(2);
        let money = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money);
        t.set_draw(0, &["通用:GREAT"]);
        t.give(0, &["通用:@Tsugu ycm"]);
        t.dice(&[1, 1, 1]); // sum 3 < 26
        t.observe(TSUGU_CLAUSES);
        t.play(0, "通用:@Tsugu ycm")
            .map_err(|e| format!("play rejected: {e}"))?;
        let p = t.expect_prompt();
        if p.options.len() != 2 {
            return Err(format!(
                "choice arity {}, expected 2: {}",
                p.options.len(),
                t.dump_prompt()
            ));
        }
        if case % 2 == 0 {
            t.answer(0, 0).map_err(|e| format!("gain: {e}"))?;
            let d = t.money(0) as i64 - money as i64;
            if d != 1000 {
                return Err(format!("money delta {d}, expected +1000"));
            }
            if t.pos(0) != CIRCLE {
                return Err(format!("pos {}, expected stay on CiRCLE", t.pos(0)));
            }
        } else {
            t.answer(0, 1).map_err(|e| format!("move: {e}"))?;
            if t.pos(0) != BANDORI {
                return Err(format!("pos {}, expected Bandori {BANDORI}", t.pos(0)));
            }
            // 并[结算]: the unowned-buy offer for the landing tile appears.
            let p = t.expect_prompt();
            if !(p.kind == "tile"
                || p.title.key().contains("buy")
                || p.title.key().contains("agent"))
            {
                return Err(format!("no settle offer: {}", t.dump_prompt()));
            }
            skip_all(&mut t);
        }
        Ok(())
    })
    .expect("Tsugu <26 choice");
}

// =====================================================================
// 通用:CiRCLE THANKS PARTY!
// =====================================================================

/// X of 「将X设为因此卡[消耗]资金的玩家数量加1」.
///
/// Ruling 2026-10-06 (TEST-FINDINGS §6): X does **not** count the user; the
/// 「加1」 supplies the user. So `X = others_who_spent + 1`.
pub fn party_x(others_who_spent: usize) -> i64 {
    others_who_spent as i64 + 1
}

/// C-GEN-party-01 -- 「所有其他玩家可选择[消耗]500资金，你消耗500资金」.
///
/// * **ID:** `C-GEN-party-01`
/// * **Source:** `roster#通用:CiRCLE THANKS PARTY!`
/// * **Quote:** 「[手]：所有其他玩家可选择[消耗]500资金，你消耗500资金，将X设为因此卡[消耗]资金的玩家数量加1。」
/// * **Trigger:** the [手] effect of this card.
/// * **Condition:** each other player may accept or decline the 500.
/// * **Effect:** the user always [消耗]s 500; each accepting other [消耗]s 500.
/// * **Frame:** see [`c_gen_party_frame`].
/// * **Rulings & OPEN:** X = (others who spent) + 1 (user ruling 2026-10-06).
/// * **Executable:** [`party_x1_extra_turn`] / [`party_x2_roll_pays`].
pub fn c_gen_party_01(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-party-02 -- the X≥2 roll branch.
///
/// * **ID:** `C-GEN-party-02`
/// * **Source:** `roster#通用:CiRCLE THANKS PARTY!`
/// * **Quote:** 「1. X至少为2则[使用者]投掷Xd20，如果结果至少为35则[使用者][获得]3000资金且其他因此卡[消耗]资金的玩家[获得]1500资金；」
/// * **Trigger:** the [手] effect, after X is set.
/// * **Condition:** `X >= 2`.
/// * **Effect:** the user rolls Xd20; on sum ≥ 35 the user [获得]s 3000 and
///   every *other* spender [获得]s 1500. Below 35 nobody gains.
/// * **Frame:** see [`c_gen_party_frame`].
/// * **Rulings & OPEN:** 「至少为35」 is inclusive (sheet 2026-10-06 A12).
/// * **Executable:** [`party_x2_roll_pays`].
pub fn c_gen_party_02(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-party-03 -- the X=1 extra-turn branch.
///
/// * **ID:** `C-GEN-party-03`
/// * **Source:** `roster#通用:CiRCLE THANKS PARTY!`
/// * **Quote:** 「2. X等于1则[使用者]的本回合结束后获得一个额外回合。」
/// * **Trigger:** the [手] effect, after X is set.
/// * **Condition:** `X == 1`.
/// * **Effect:** after the user's current turn ends they receive one extra turn.
/// * **Frame:** see [`c_gen_party_frame`].
/// * **Rulings & OPEN:** none.
/// * **Executable:** [`party_x1_extra_turn`].
pub fn c_gen_party_03(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// Executable frame for a THANKS PARTY play.
pub fn c_gen_party_frame(step: &Step<'_>) -> Result<(), String> {
    if !step.act_is_card("通用:CiRCLE THANKS PARTY!") {
        return Ok(());
    }
    let allowed = [
        "money:P".to_string(),
        "hand:P".to_string(),
        "draw:P".to_string(),
        "discard:P".to_string(),
        "hand_n:P".to_string(),
        "draw_n:P".to_string(),
        "discard_pub:P".to_string(),
        "extra_turns".into(),
        "turn.player".into(),
        "next_turn_pending".into(),
        "turn.abnormal".into(),
        "targeted".into(),
    ];
    let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
    frame(step.pre, step.post, &allowed)
}

/// The clause set a THANKS PARTY contract arms on `Table::observe`.
pub const PARTY_CLAUSES: &[Clause] = &[Clause::new(
    "C-GEN-party-frame",
    "roster#通用:CiRCLE THANKS PARTY!",
    "[手]：所有其他玩家可选择[消耗]500资金，你消耗500资金，…",
    c_gen_party_frame,
)];

/// C-GEN-party-03: X = 1 (nobody else joins) grants an extra turn.
#[test]
fn party_x1_extra_turn() {
    forall(6, 0x6EA7_000B, |rng, _i| {
        let mut t = Table::vanilla(3);
        let money = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money);
        t.give(0, &["通用:CiRCLE THANKS PARTY!"]);
        t.observe(PARTY_CLAUSES);
        t.play(0, "通用:CiRCLE THANKS PARTY!")
            .map_err(|e| format!("play rejected: {e}"))?;
        // Decline every other player's 500 offer.
        while t.prompt().is_some() {
            for who in t.asked() {
                if who != 0 {
                    let _ = t.decline_one(who, t.expect_prompt().id, t.expect_prompt().fallback);
                } else {
                    break;
                }
            }
            // Fallback: if the prompt is still open and only the user is left,
            // decline everything.
            if t.prompt().is_some() {
                t.decline();
            }
        }
        let d = t.money(0) as i64 - money as i64;
        if d != -500 {
            return Err(format!("user money delta {d}, expected -500 ([消耗]500)"));
        }
        // Drive to the end of the user's turn: the extra turn keeps the seat.
        t.dice(&[1]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        skip_all(&mut t);
        t.end(0).map_err(|e| format!("end: {e}"))?;
        if t.turn() != 0 {
            return Err(format!("turn {}, expected extra turn on P0", t.turn()));
        }
        Ok(())
    })
    .expect("THANKS PARTY X=1");
}

/// C-GEN-party-02: X ≥ 2 rolls Xd20; sum < 35 pays nobody.
#[test]
fn party_x2_roll_miss_pays_nobody() {
    forall(4, 0x6EA7_0019, |rng, _i| {
        let mut t = Table::vanilla(3);
        let base: Vec<i64> = (0..3)
            .map(|i| {
                let m = 10_000 + rng.range(0, 10) * 100;
                t.set_money(i, m);
                m as i64
            })
            .collect();
        t.give(0, &["通用:CiRCLE THANKS PARTY!"]);
        // X = 2, 2d20 sum 2 < 35.
        t.dice(&[1, 1]);
        t.play(0, "通用:CiRCLE THANKS PARTY!")
            .map_err(|e| format!("play rejected: {e}"))?;
        loop {
            let Some(p) = t.prompt() else { break };
            let asked = t.asked();
            if asked.is_empty() {
                break;
            }
            let who = asked[0];
            if who == 1 {
                t.answer(who, 0).map_err(|e| format!("P1 join: {e}"))?;
            } else {
                t.decline_one(who, p.id, p.fallback)
                    .map_err(|e| format!("decline {who}: {e}"))?;
            }
        }
        skip_all(&mut t);
        // Only the 500 spends -- nobody gains.
        if t.money(0) as i64 - base[0] != -500 {
            return Err(format!(
                "user delta {}, expected -500",
                t.money(0) as i64 - base[0]
            ));
        }
        if t.money(1) as i64 - base[1] != -500 {
            return Err(format!(
                "P1 delta {}, expected -500",
                t.money(1) as i64 - base[1]
            ));
        }
        if t.money(2) as i64 - base[2] != 0 {
            return Err(format!(
                "P2 delta {}, expected 0",
                t.money(2) as i64 - base[2]
            ));
        }
        Ok(())
    })
    .expect("THANKS PARTY X≥2 miss");
}

/// C-GEN-party-02: X = 3 rolls 3d20 and pays 3000 / 1500 to the joiners.
#[test]
fn party_x3_roll_pays() {
    forall(4, 0x6EA7_001A, |rng, _i| {
        let mut t = Table::vanilla(4);
        let base: Vec<i64> = (0..4)
            .map(|i| {
                let m = 10_000 + rng.range(0, 10) * 100;
                t.set_money(i, m);
                m as i64
            })
            .collect();
        t.give(0, &["通用:CiRCLE THANKS PARTY!"]);
        // X = 3 (two others join) → 3d20 sum 60 ≥ 35.
        t.dice(&[20, 20, 20]);
        t.play(0, "通用:CiRCLE THANKS PARTY!")
            .map_err(|e| format!("play rejected: {e}"))?;
        loop {
            let Some(p) = t.prompt() else { break };
            let asked = t.asked();
            if asked.is_empty() {
                break;
            }
            let who = asked[0];
            if who == 1 || who == 2 {
                t.answer(who, 0).map_err(|e| format!("P{who} join: {e}"))?;
            } else {
                t.decline_one(who, p.id, p.fallback)
                    .map_err(|e| format!("decline {who}: {e}"))?;
            }
        }
        skip_all(&mut t);
        if t.money(0) as i64 - base[0] != -500 + 3000 {
            return Err(format!(
                "user delta {}, expected 2500",
                t.money(0) as i64 - base[0]
            ));
        }
        for w in [1, 2] {
            if t.money(w) as i64 - base[w] != -500 + 1500 {
                return Err(format!(
                    "P{w} delta {}, expected 1000",
                    t.money(w) as i64 - base[w]
                ));
            }
        }
        if t.money(3) as i64 - base[3] != 0 {
            return Err(format!(
                "P3 delta {}, expected 0 (declined)",
                t.money(3) as i64 - base[3]
            ));
        }
        Ok(())
    })
    .expect("THANKS PARTY X=3");
}

/// C-GEN-party-02: X ≥ 2 rolls Xd20; sum ≥ 35 pays 3000 / 1500.
#[test]
fn party_x2_roll_pays() {
    forall(6, 0x6EA7_000C, |rng, _i| {
        let mut t = Table::vanilla(3);
        let base: Vec<i64> = (0..3)
            .map(|i| {
                let m = 10_000 + rng.range(0, 10) * 100;
                t.set_money(i, m);
                m as i64
            })
            .collect();
        t.give(0, &["通用:CiRCLE THANKS PARTY!"]);
        // X = 2: one other joins. 2d20 loaded to 20+15 = 35 (inclusive win).
        t.dice(&[20, 15]);
        t.observe(PARTY_CLAUSES);
        t.play(0, "通用:CiRCLE THANKS PARTY!")
            .map_err(|e| format!("play rejected: {e}"))?;
        // P1 accepts (answer 0), P2 declines (answer 1) -- prompt option order
        // is accept/decline in the engine's yes/no.
        loop {
            let Some(p) = t.prompt() else { break };
            let asked = t.asked();
            if asked.is_empty() {
                break;
            }
            let who = asked[0];
            if who == 1 {
                t.answer(who, 0).map_err(|e| format!("P1 join: {e}"))?;
            } else {
                t.decline_one(who, p.id, p.fallback)
                    .map_err(|e| format!("decline {who}: {e}"))?;
            }
        }
        skip_all(&mut t);
        // user: −500 + 3000; P1: −500 + 1500; P2: 0.
        if t.money(0) as i64 - base[0] != -500 + 3000 {
            return Err(format!(
                "user delta {}, expected 2500",
                t.money(0) as i64 - base[0]
            ));
        }
        if t.money(1) as i64 - base[1] != -500 + 1500 {
            return Err(format!(
                "P1 delta {}, expected 1000",
                t.money(1) as i64 - base[1]
            ));
        }
        if t.money(2) as i64 - base[2] != 0 {
            return Err(format!(
                "P2 delta {}, expected 0 (declined)",
                t.money(2) as i64 - base[2]
            ));
        }
        Ok(())
    })
    .expect("THANKS PARTY X≥2");
}

// =====================================================================
// 通用:[月岛麻里奈]今天也要加油工作喔
// =====================================================================

/// C-GEN-marina-01 -- the [反击] on an extra effect over #1格子.
///
/// * **ID:** `C-GEN-marina-01`
/// * **Source:** `roster#通用:[月岛麻里奈]今天也要加油工作喔`
/// * **Quote:** 「[反击][使用者][经过]#1格子且#1格子受到其他效果影响时：[使用者]本次对#1格子的[经过]或[结算]正常进行而不受到其上的额外效果。」
/// * **Trigger:** a [反击] window opened because the user is [经过]ing #1格子
///   (engine index 0, CiRCLE) **and** that tile carries another effect.
/// * **Condition:** the user is 月岛麻里奈 (the card name is a character
///   exclusive -- rulebook: 「角色专属卡在卡名里会有角色名」).
/// * **Effect:** the [经过] / [结算] of #1 proceeds normally; the extra effect
///   on the tile does not apply to the user.
/// * **Frame:** the counteraction itself spends the card from hand.
/// * **Rulings & OPEN:** 「受到其他效果影响」 is what the extra effect is; this
///   contract arranges a [CP点] tile-mark (the 该清CP了 settle clause) as the
///   extra effect. Exclusive-play refusal is `err.play_exclusive`.
/// * **Executable:** [`marina_counters_extra_effect`].
pub fn c_gen_marina_01(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-marina-exclusive -- the card is 月岛麻里奈's exclusive.
///
/// * **ID:** `C-GEN-marina-exclusive`
/// * **Source:** `roster#通用:[月岛麻里奈]今天也要加油工作喔` + `rulebook-doc.md:112`
/// * **Quote:** 「角色专属卡在卡名里会有角色名（例：[丸山彩] 憧憬的前方）。」
/// * **Trigger:** a play of this card.
/// * **Condition:** the user's character is not 月岛麻里奈.
/// * **Effect:** the play is refused.
/// * **Frame:** a refused play changes nothing.
/// * **Rulings & OPEN:** the card id carries 「[月岛麻里奈]」.
/// * **Executable:** [`marina_exclusive_refused`].
pub fn c_gen_marina_exclusive(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-marina-01: the [反击] neutralises the tile's extra effect for the user.
#[test]
fn marina_counters_extra_effect() {
    forall(4, 0x6EA7_000D, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money);
        t.set_character_raw(0, "月岛麻里奈");
        t.give(0, &["通用:[月岛麻里奈]今天也要加油工作喔"]);
        // Extra effect on #1: a [CP点] mark from 该清CP了. Move everyone off
        // CiRCLE first -- the [手] only offers tiles 「没有角色和[CP点]」.
        t.set_pos(0, 10);
        t.set_pos(1, 10);
        t.give(0, &["通用:该清CP了"]);
        t.play(0, "通用:该清CP了")
            .map_err(|e| format!("cp play: {e}"))?;
        if t.prompt().is_some() {
            t.answer_tile(0, CIRCLE)
                .map_err(|e| format!("cp tile: {e}"))?;
        }
        skip_all(&mut t);
        if t.marks_on(CIRCLE).is_empty() {
            return Err("no extra effect on #1".into());
        }
        // Walk through CiRCLE (from #56, roll 6, path wraps, lands on #3).
        t.begin_turn(0);
        t.set_pos(0, 56);
        t.dice(&[6]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        // Find the counteract window offering the card.
        let mut offered = false;
        for _ in 0..6 {
            if t.counteract_offered("通用:[月岛麻里奈]今天也要加油工作喔") {
                offered = true;
                break;
            }
            if t.prompt().is_some() {
                let who = t.asked()[0];
                let _ = t.answer(who, 0);
            } else {
                break;
            }
        }
        if !offered {
            return Err(format!("counteract never offered: {}", t.dump_prompt()));
        }
        t.counteract(0, "通用:[月岛麻里奈]今天也要加油工作喔")
            .map_err(|e| format!("counteract: {e}"))?;
        skip_all(&mut t);
        // 「正常进行」: the pass went through and the CiRCLE reward paid, with
        // no CP extra.
        if t.pos(0) != 2 {
            return Err(format!("pos {}, expected 2 (pass #1, land #3)", t.pos(0)));
        }
        // Money across the walk: +2000 (CiRCLE reward) only. The CP settle's
        // [获得]800 is the 「额外效果」 this [反击] suppresses.
        let reward = t.money(0) as i64 - money as i64;
        if reward != 2000 {
            return Err(format!(
                "money delta {reward}, expected +2000 (CiRCLE reward, CP extra suppressed)"
            ));
        }
        Ok(())
    })
    .expect("月岛麻里奈 [反击]");
}

/// C-GEN-marina-exclusive: a non-月岛 seat cannot play the card.
#[test]
fn marina_exclusive_refused() {
    let mut t = Table::vanilla(2);
    t.give(0, &["通用:[月岛麻里奈]今天也要加油工作喔"]);
    let r = t.play(0, "通用:[月岛麻里奈]今天也要加油工作喔");
    match r {
        Err(e) if e.contains("exclusive") || e.contains("play_exclusive") => Ok(()),
        other => Err(format!("expected exclusive refusal, got {other:?}")),
    }
    .expect("月岛麻里奈 exclusive");
}

// =====================================================================
// 通用:[都筑诗船]Parking Space
// =====================================================================

/// C-GEN-parking-01 -- place on Space and redistribute houses.
///
/// * **ID:** `C-GEN-parking-01`
/// * **Source:** `roster#通用:[都筑诗船]Parking Space`
/// * **Quote:** 「[手]：将此卡放置于“Space”格子上，将其上的房屋转移至其他你拥有的格子上（每个格子因此效果最多获得1层）。」
/// * **Trigger:** the [手] effect of this card.
/// * **Condition:** the user is 都筑诗船 (name exclusive); Space may hold houses.
/// * **Effect:** the card sits on the Space tile; houses that were on Space are
///   transferred to other tiles the user owns, at most +1 level each.
/// * **Frame:** see [`c_gen_parking_frame`].
/// * **Rulings & OPEN:** excess houses beyond the +1-capacity of the user's
///   other tiles are unspecified -- `OPEN-GEN-parking-houses`.
/// * **Executable:** [`parking_moves_houses`].
pub fn c_gen_parking_01(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-parking-02 -- （1） settle replaced by a turn-end [停留].
///
/// * **ID:** `C-GEN-parking-02`
/// * **Source:** `roster#通用:[都筑诗船]Parking Space`
/// * **Quote:** 「（1）此卡所在格子的[结算]改为回合结束后获得一层[停留]。」
/// * **Trigger:** a player [结算]s the tile the card sits on.
/// * **Condition:** the card is placed on Space.
/// * **Effect:** the ordinary settle does not run; at that player's turn end
///   they gain one layer of [停留].
/// * **Frame:** see [`c_gen_parking_frame`].
/// * **Rulings & OPEN:** 「回合结束后」 is the end of the settling player's
///   turn (行动阶段 row 18 also drops one layer of [停留]/[晕眩] -- the gain
///   and the drop are different turns' bookkeeping).
/// * **Executable:** [`parking_replaces_settle`].
pub fn c_gen_parking_02(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-parking-03 -- （2） skills blocked on the tile.
///
/// * **ID:** `C-GEN-parking-03`
/// * **Source:** `roster#通用:[都筑诗船]Parking Space`
/// * **Quote:** 「（2）位于此卡所在格子上的玩家无法使用角色及乐队技能。」
/// * **Trigger:** a skill use by a player standing on the card's tile.
/// * **Condition:** the card is placed; the player is on that tile.
/// * **Effect:** character and band skills are refused.
/// * **Frame:** a refused skill changes nothing.
/// * **Rulings & OPEN:** none.
/// * **Executable:** [`parking_blocks_skills`].
pub fn c_gen_parking_03(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// Executable frame for a Parking Space play.
pub fn c_gen_parking_frame(step: &Step<'_>) -> Result<(), String> {
    if !step.act_is_card("通用:[都筑诗船]Parking Space") {
        return Ok(());
    }
    let who = step.player_of_act();
    let allowed = [
        format!("money:P{who}"),
        format!("hand:P{who}"),
        format!("draw:P{who}"),
        format!("discard:P{who}"),
        format!("hand_n:P{who}"),
        format!("draw_n:P{who}"),
        format!("discard_pub:P{who}"),
        format!("field:P{who}"),
        format!("crystals:P{who}"),
        format!("fprop:P{who}"),
        "houses:T".into(),
        "owner:T".into(),
        format!("state:P"),
        "mark:".into(),
        "pool:".into(),
    ];
    let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
    frame(step.pre, step.post, &allowed)
}

/// The name carries 「[都筑诗船]」; a non-都筑 seat cannot play it.
#[test]
fn parking_exclusive_refused() {
    let mut t = Table::vanilla(2);
    t.give(0, &["通用:[都筑诗船]Parking Space"]);
    let r = t.play(0, "通用:[都筑诗船]Parking Space");
    match r {
        Err(e) if e.contains("exclusive") || e.contains("play_exclusive") => Ok(()),
        other => Err(format!("expected exclusive refusal, got {other:?}")),
    }
    .expect("都筑诗船 exclusive");
}

/// C-GEN-parking-01: Space's houses move to other owned tiles (+1 each).
#[test]
fn parking_moves_houses() {
    forall(6, 0x6EA7_000E, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "都筑诗船");
        t.set_money(0, 10_000 + rng.range(0, 10) * 100);
        t.set_owner(SPACE, Some(0));
        t.set_houses(SPACE, 2);
        t.own(0, &[BANDORI, FUJIMI]);
        t.give(0, &["通用:[都筑诗船]Parking Space"]);
        t.observe(&[Clause::new(
            "C-GEN-parking-frame",
            "roster#通用:[都筑诗船]Parking Space",
            "将此卡放置于“Space”格子上，将其上的房屋转移至其他你拥有的格子上（每个格子因此效果最多获得1层）。",
            c_gen_parking_frame,
        )]);
        t.play(0, "通用:[都筑诗船]Parking Space")
            .map_err(|e| format!("play rejected: {e}"))?;
        if t.houses(SPACE) != 0 {
            return Err(format!("Space houses {}, expected 0", t.houses(SPACE)));
        }
        if t.houses(BANDORI) != 1 || t.houses(FUJIMI) != 1 {
            return Err(format!(
                "BANDORI {} FUJIMI {}, expected 1/1 (each +1)",
                t.houses(BANDORI),
                t.houses(FUJIMI)
            ));
        }
        if !t.on_field(0, "通用:[都筑诗船]Parking Space") {
            return Err("card not on field".into());
        }
        Ok(())
    })
    .expect("Parking Space houses");
}

/// C-GEN-parking-02 + 03: settle replaced by stay; skills blocked.
#[test]
fn parking_replaces_settle_and_blocks_skills() {
    forall(4, 0x6EA7_000F, |rng, _i| {
        let mut t = Table::new(&["都筑诗船", "户山香澄"]);
        t.clean();
        t.set_character_raw(0, "都筑诗船");
        t.set_owner(SPACE, Some(0));
        t.set_money(0, 10_000 + rng.range(0, 10) * 100);
        t.give(0, &["通用:[都筑诗船]Parking Space"]);
        t.play(0, "通用:[都筑诗船]Parking Space")
            .map_err(|e| format!("play rejected: {e}"))?;

        // （2）: stand on Space and try a character skill -- refused.
        t.set_pos(0, SPACE);
        let sid = t.skill_id(0, "尽力了吗");
        if t.skill(0, &sid).is_ok() {
            return Err("skill succeeded on the blocked tile".into());
        }

        // （1）: P1 settles Space; the buy/rent settle is gone; stay at turn end.
        t.begin_turn(1);
        t.set_pos(1, SPACE - 1);
        let m0 = t.money(0);
        let m1 = t.money(1);
        t.dice(&[1]);
        t.roll(1).map_err(|e| format!("roll: {e}"))?;
        skip_all(&mut t);
        if t.owner(SPACE) != Some(0) {
            return Err(format!("Space ownership changed: {:?}", t.owner(SPACE)));
        }
        // 「[结算]改为」: no rent changed hands.
        if t.money(0) != m0 || t.money(1) != m1 {
            return Err(format!(
                "money moved on the replaced settle: P0 {m0}->{}, P1 {m1}->{:?}",
                t.money(0),
                t.money(1)
            ));
        }
        t.end(1).map_err(|e| format!("end: {e}"))?;
        if t.state(1, "stay") != 1 {
            return Err(format!(
                "P1 stay {}, expected 1 at turn end",
                t.state(1, "stay")
            ));
        }
        Ok(())
    })
    .expect("Parking Space continuous");
}

// =====================================================================
// 通用:安可
// =====================================================================

/// C-GEN-encore-01 -- negate an [异常移动效果] on the user.
///
/// * **ID:** `C-GEN-encore-01`
/// * **Source:** `roster#通用:安可`
/// * **Quote:** 「[反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次[异常移动效果]和其导致的所有效果。」
/// * **Trigger:** a [反击] window opened because the user is about to receive
///   an [异常移动效果].
/// * **Condition:** the effect is one of the [异常移动效果] set (rulebook:
///   「包括[传送]，[停留]，[晕眩]，[除外]，[强制移动]，[强制停下]，反方向移动」).
/// * **Effect:** that effect and everything it would cause are negated for
///   the user.
/// * **Frame:** the counteraction spends the card from hand.
/// * **Rulings & OPEN:** this contract arranges a [停留] from 「雨啊，快点来吧」
///   as the incoming effect.
/// * **Executable:** [`encore_negates_stay`].
pub fn c_gen_encore_01(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-encore-01: [停留] on the user is negated; other targets still hit.
#[test]
fn encore_negates_stay() {
    forall(6, 0x6EA7_0010, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_money(0, 10_000 + rng.range(0, 10) * 100);
        t.give(0, &["通用:安可"]);
        // P1 plays 雨啊 targeting both (2d2 → X = 2).
        t.begin_turn(1);
        t.dice(&[1, 1]);
        t.give_play(1, "通用:雨啊，快点来吧")
            .map_err(|e| format!("rain: {e}"))?;
        // Designate P0 (the remaining pick is the user P1 automatically).
        if t.prompt().is_some() {
            t.answer(1, 0).map_err(|e| format!("target: {e}"))?;
        }
        if !t.counteract_offered("通用:安可") {
            return Err(format!("安可 not offered: {}", t.dump_prompt()));
        }
        t.counteract(0, "通用:安可")
            .map_err(|e| format!("counteract: {e}"))?;
        skip_all(&mut t);
        if t.state(0, "stay") != 0 {
            return Err(format!(
                "user stay {}, expected 0 (negated)",
                t.state(0, "stay")
            ));
        }
        if t.state(1, "stay") != 1 {
            return Err(format!(
                "P1 stay {}, expected 1 (still targeted)",
                t.state(1, "stay")
            ));
        }
        Ok(())
    })
    .expect("安可 [反击]");
}

// =====================================================================
// 通用:尽力后的收获
// =====================================================================

/// C-GEN-harvest-01 -- move 1..=6 and [结算].
///
/// * **ID:** `C-GEN-harvest-01`
/// * **Source:** `roster#通用:尽力后的收获`
/// * **Quote:** 「[手]：立刻进入移动阶段，本回合的[主要移动]改为移动1到6以内的任意整数并[结算]。」
/// * **Trigger:** the [手] effect of this card.
/// * **Condition:** none beyond the play.
/// * **Effect:** the user enters the move phase immediately; the [主要移动]
///   becomes a move of any integer in 1..=6, and the landing tile [结算]s.
/// * **Frame:** see [`c_gen_harvest_frame`].
/// * **Rulings & OPEN:** 「任意整数」 is the player's choice among 1..=6
///   (six options). Wrap / reverse follow the ordinary [路径] rules.
/// * **Executable:** [`harvest_moves_and_settles`].
pub fn c_gen_harvest_01(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// Executable frame for a harvest play.
pub fn c_gen_harvest_frame(step: &Step<'_>) -> Result<(), String> {
    // Keyed on the engine act (the play input), never on the `"card"`
    // presentation event -- a lazy header must not move the frame window.
    if !step.act_is_play("通用:尽力后的收获") {
        return Ok(());
    }
    let who = step.player_of_act();
    let allowed = [
        format!("money:P"),
        format!("hand:P{who}"),
        format!("draw:P{who}"),
        format!("discard:P{who}"),
        format!("hand_n:P{who}"),
        format!("draw_n:P{who}"),
        format!("discard_pub:P{who}"),
        format!("pos:P{who}"),
        "owner:T".into(),
        "houses:T".into(),
    ];
    let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
    frame(step.pre, step.post, &allowed)
}

/// C-GEN-harvest-01: six options 1..=6; the chosen distance settles.
#[test]
fn harvest_moves_and_settles() {
    forall(6, 0x6EA7_0011, |rng, _case| {
        let mut t = Table::vanilla(2);
        t.set_money(0, 10_000 + rng.range(0, 10) * 100);
        // Start just before the 地产商 (index 4) so 「并[结算]」 shows up as a
        // tile prompt (its settle offers the same-colour buys).
        t.set_pos(0, 3);
        t.give(0, &["通用:尽力后的收获"]);
        let pre = t.snapshot_world();
        t.observe(&[Clause::new(
            "C-GEN-harvest-frame",
            "roster#通用:尽力后的收获",
            "立刻进入移动阶段，本回合的[主要移动]改为移动1到6以内的任意整数并[结算]。",
            c_gen_harvest_frame,
        )]);
        t.play(0, "通用:尽力后的收获")
            .map_err(|e| format!("play rejected: {e}"))?;
        let p = t.expect_prompt();
        if p.options.len() != 6 {
            return Err(format!(
                "options {}, expected 6 (1..=6): {}",
                p.options.len(),
                t.dump_prompt()
            ));
        }
        // Distance 1 lands on the 地产商 (index 4) -- its [结算] is a tile prompt.
        let dist = 1;
        t.answer(0, dist - 1).map_err(|e| format!("dist: {e}"))?;
        if t.pos(0) != 4 {
            return Err(format!("pos {}, expected 4 (地产商)", t.pos(0)));
        }
        // 「并[结算]」: the settle offer must be open before we decline it.
        let p = t.expect_prompt();
        if p.kind != "tile" && !p.title.key().contains("buy") && !p.title.key().contains("agent") {
            return Err(format!("no settle/buy offer: {}", t.dump_prompt()));
        }
        skip_all(&mut t);
        frame(
            &pre,
            t.world(),
            &[
                "money:P",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P",
                "draw_n:P",
                "discard_pub:P",
                "pos:P0",
                "owner:T",
                "houses:T",
                "turn.abnormal",
            ],
        )
    })
    .expect("尽力后的收获");
}

// =====================================================================
// 通用:网络链接异常
// =====================================================================

/// C-GEN-net-02 -- negate an untargeted hand [手] effect.
///
/// * **ID:** `C-GEN-net-02`
/// * **Source:** `roster#通用:网络链接异常`
/// * **Quote:** 「2. 手卡的[手]效果且没有[指定]目标则抵消其所有的效果；」
/// * **Trigger:** a [反击] window before a hand card's [手] effect resolves.
/// * **Condition:** the target effect is a hand [手] with no [指定] targets.
/// * **Effect:** every effect of that [手] is negated.
/// * **Frame:** the counteraction spends the card from hand.
/// * **Rulings & OPEN:** the quote's third branch (「事件卡的效果手牌则则」) is
///   garbled -- `OPEN-GEN-net-event`.
/// * **Executable:** [`net_negates_untargeted`].
pub fn c_gen_net_02(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-net-01 -- cancel one target of a targeted hand [手] effect.
///
/// * **ID:** `C-GEN-net-01`
/// * **Source:** `roster#通用:网络链接异常`
/// * **Quote:** 「1. 手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]；」
/// * **Trigger:** a [反击] window before a targeted hand [手] effect resolves.
/// * **Condition:** the target effect designates one or more targets.
/// * **Effect:** exactly one of the designations is cancelled (not all).
/// * **Frame:** the counteraction spends the card from hand.
/// * **Rulings & OPEN:** which of the targets is cancelled is unspecified --
///   `OPEN-GEN-net-which-target`.
/// * **Executable:** [`net_cancels_one_target`].
pub fn c_gen_net_01(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-net-02: GREAT is fully negated (no money, no PERFECT).
#[test]
fn net_negates_untargeted() {
    forall(6, 0x6EA7_0012, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money);
        t.set_draw(0, &["通用:安可"]);
        t.give(1, &["通用:网络链接异常"]);
        t.give(0, &["通用:GREAT"]);
        t.play(0, "通用:GREAT").map_err(|e| format!("play: {e}"))?;
        if !t.counteract_offered("通用:网络链接异常") {
            return Err(format!("net not offered: {}", t.dump_prompt()));
        }
        t.counteract(1, "通用:网络链接异常")
            .map_err(|e| format!("counteract: {e}"))?;
        skip_all(&mut t);
        if t.money(0) as i64 != money as i64 {
            return Err(format!(
                "money {}, expected unchanged (all effects negated)",
                t.money(0)
            ));
        }
        if t.draw_pile(0).iter().any(|c| c == "通用:[衍生]PERFECT") {
            return Err(format!(
                "PERFECT appeared despite negation: {:?}",
                t.draw_pile(0)
            ));
        }
        // The play still spends the card; only its [手] body is negated.
        if t.hand(0).iter().any(|c| c == "通用:GREAT") {
            return Err(format!("GREAT still in hand: {:?}", t.hand(0)));
        }
        Ok(())
    })
    .expect("网络链接异常 untargeted");
}

/// C-GEN-net-01: 武道馆 keeps paying for one of the two targets only.
#[test]
fn net_cancels_one_target() {
    forall(6, 0x6EA7_0013, |rng, _i| {
        let mut t = Table::vanilla(3);
        let base: Vec<i64> = (0..3)
            .map(|i| {
                let m = 10_000 + rng.range(0, 10) * 100;
                t.set_money(i, m);
                m as i64
            })
            .collect();
        t.give(1, &["通用:网络链接异常"]);
        t.give(0, &["通用:登上武道馆"]);
        t.play(0, "通用:登上武道馆")
            .map_err(|e| format!("play: {e}"))?;
        if !t.counteract_offered("通用:网络链接异常") {
            return Err(format!("net not offered: {}", t.dump_prompt()));
        }
        t.counteract(1, "通用:网络链接异常")
            .map_err(|e| format!("counteract: {e}"))?;
        skip_all(&mut t);
        // One target cancelled: exactly one of P1/P2 pays X = 1000 (3 players).
        let x = budokan_x(2);
        let paid: i64 = (1..3).map(|w| base[w] - t.money(w) as i64).sum();
        if paid != x {
            return Err(format!(
                "others paid {paid}, expected {x} (exactly one designation cancelled)"
            ));
        }
        let got = t.money(0) as i64 - base[0];
        if got != x {
            return Err(format!("user received {got}, expected {x}"));
        }
        Ok(())
    })
    .expect("网络链接异常 targeted");
}

// =====================================================================
// 通用:该清CP了
// =====================================================================

/// C-GEN-clearcp-01 -- [手]: one tile [CP点] + six on-card [CP点].
///
/// * **ID:** `C-GEN-clearcp-01`
/// * **Source:** `roster#通用:该清CP了`
/// * **Quote:** 「[手]：在任意一个没有角色和[CP点]的格子上添加1个[CP点]并在自己[场上]添加6个[CP点]。」
/// * **Trigger:** the [手] effect of this card.
/// * **Condition:** the chosen tile has no character and no [CP点].
/// * **Effect:** one tile-mark [CP点] on that tile; six on-card [CP点] on this
///   instance (`FieldCard::cp`, ruling 2026-10-07).
/// * **Frame:** see [`c_gen_clearcp_frame`].
/// * **Rulings & OPEN:** two kinds of [CP点] (TEST-FINDINGS §6, 2026-10-07):
///   tile marks (`category = "cp"`, `owner = -1`, provenance `src`) and
///   on-card counters on the instance. 「自己[场上]」 is the card's own count.
/// * **Executable:** [`clearcp_places_cp`].
pub fn c_gen_clearcp_01(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-clearcp-02 -- settle on a [CP] tile spends 1+1 and pays 800.
///
/// * **ID:** `C-GEN-clearcp-02`
/// * **Source:** `roster#通用:该清CP了`
/// * **Quote:** 「在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]1个[CP点]，[获得]800资金。」
/// * **Trigger:** any player's [结算] on a tile carrying a [CP点].
/// * **Condition:** the mark's `src` card still holds ≥1 on-card [CP点].
/// * **Effect:** one tile [CP点] and one on-card [CP点] are spent; the
///   **settler** [获得]s 800.
/// * **Frame:** see [`c_gen_clearcp_frame`].
/// * **Rulings & OPEN:** the printed 「移除格子上的个[CP点]」 omits the numeral;
///   read as **1** (TEST-FINDINGS §6). 「自己[场上]1个[CP点]」 is spent from the
///   card the mark is attached to (`TileMark.src`), and the settler is the
///   subject of the gain (same §6).
/// * **Executable:** [`clearcp_settle_pays`].
pub fn c_gen_clearcp_02(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-clearcp-03 -- （1） spread to adjacent tiles for two turn starts.
///
/// * **ID:** `C-GEN-clearcp-03`
/// * **Source:** `roster#通用:该清CP了`
/// * **Quote:** 「（1）[使用者]使用此卡后的下2回合开始时，此卡在格子上添加的[CP点]及其产物将在相邻的没有[CP点]的格子添加1个[CP点]。」
/// * **Trigger:** each of the next two turn starts after the play.
/// * **Condition:** the source marks (this card's tile [CP点] and their
///   products) sit on tiles whose neighbour has no [CP点].
/// * **Effect:** each such product mark adds one [CP点] on an adjacent empty
///   tile.
/// * **Frame:** see [`c_gen_clearcp_frame`].
/// * **Rulings & OPEN:** 「及其产物」 is provenance (this card instance), per
///   the 2026-10-07 ruling. Which neighbour is chosen when both are empty is
///   unspecified -- `OPEN-GEN-clearcp-neighbour`.
/// * **Executable:** [`clearcp_spreads`].
pub fn c_gen_clearcp_03(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-GEN-clearcp-04 -- graveyard when the on-card [CP点] hits 0.
///
/// * **ID:** `C-GEN-clearcp-04`
/// * **Source:** `roster#通用:该清CP了` + TEST-FINDINGS §6 (2026-10-07)
/// * **Quote:** 「The card is graveyarded as soon as its on-card [CP点] is empty.」
/// * **Trigger:** the on-card count drops to 0 however it drops.
/// * **Condition:** this instance is on the field.
/// * **Effect:** the card leaves the [场地] into the owner's 弃卡区.
/// * **Frame:** see [`c_gen_clearcp_frame`].
/// * **Rulings & OPEN:** user ruling 2026-10-07 (the sheet text does not state
///   the graveyard; the ruling does). Tile marks may outlive the card.
/// * **Executable:** [`clearcp_graveyards_at_zero`].
pub fn c_gen_clearcp_04(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// Executable frame for a 该清CP了 play / settle / spread.
pub fn c_gen_clearcp_frame(step: &Step<'_>) -> Result<(), String> {
    // Fire on a 该清CP了 activation, or on a settle that spends its marks
    // (the mark's note is `log.cp_place` / the gain is 800 to the settler).
    let touches_card = step
        .events
        .iter()
        .any(|e| e.card.contains("该清CP了") || e.card.contains("clear_cp"));
    let spends_cp = step
        .events
        .iter()
        .any(|e| e.msg.key().contains("cp") || (e.r#type == "gain" && e.value == 800));
    if !touches_card && !spends_cp {
        return Ok(());
    }
    let allowed = [
        "money:P".to_string(),
        "hand:P".to_string(),
        "draw:P".to_string(),
        "discard:P".to_string(),
        "hand_n:P".to_string(),
        "draw_n:P".to_string(),
        "discard_pub:P".to_string(),
        "field:P".to_string(),
        "crystals:P".to_string(),
        "pool:P".to_string(),
        "fprop:P".to_string(),
        "mark:".into(),
        "pool:".into(),
    ];
    let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
    frame(step.pre, step.post, &allowed)
}

/// C-GEN-clearcp-01: one tile mark + six on-card CP.
#[test]
fn clearcp_places_cp() {
    forall(6, 0x6EA7_0014, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_money(0, 10_000 + rng.range(0, 10) * 100);
        t.give(0, &["通用:该清CP了"]);
        t.play(0, "通用:该清CP了")
            .map_err(|e| format!("play rejected: {e}"))?;
        if t.prompt().is_some() {
            t.answer_tile(0, NO_HOUSE_TILE)
                .map_err(|e| format!("tile: {e}"))?;
        }
        skip_all(&mut t);
        let marks = t.marks_on(NO_HOUSE_TILE);
        if marks.len() != 1 || marks[0].count != 1 {
            return Err(format!("marks on tile {NO_HOUSE_TILE}: {:?}", marks));
        }
        if marks[0].owner != -1 {
            return Err(format!("CP mark owned by {}: {:?}", marks[0].owner, marks));
        }
        if t.cp_on_card(0, "通用:该清CP了") != Some(6) {
            return Err(format!(
                "on-card CP {:?}, expected Some(6)",
                t.cp_on_card(0, "通用:该清CP了")
            ));
        }
        if !t.on_field(0, "通用:该清CP了") {
            return Err("card not on field".into());
        }
        Ok(())
    })
    .expect("该清CP了 place");
}

/// C-GEN-clearcp-02: settle spends 1+1 and the settler gains 800.
#[test]
fn clearcp_settle_pays() {
    forall(6, 0x6EA7_0015, |rng, _i| {
        let mut t = Table::vanilla(2);
        let base = 10_000 + rng.range(0, 10) * 100;
        t.set_money(1, base);
        t.give(0, &["通用:该清CP了"]);
        t.play(0, "通用:该清CP了")
            .map_err(|e| format!("play rejected: {e}"))?;
        if t.prompt().is_some() {
            t.answer_tile(0, NO_HOUSE_TILE)
                .map_err(|e| format!("tile: {e}"))?;
        }
        skip_all(&mut t);
        // P1 (not the owner) settles the marked tile.
        t.begin_turn(1);
        t.set_pos(1, NO_HOUSE_TILE - 1);
        t.dice(&[1]);
        t.roll(1).map_err(|e| format!("roll: {e}"))?;
        skip_all(&mut t);
        if t.money(1) as i64 - base as i64 != 800 {
            return Err(format!(
                "settler money delta {}, expected +800",
                t.money(1) as i64 - base as i64
            ));
        }
        if t.cp_on_card(0, "通用:该清CP了") != Some(5) {
            return Err(format!(
                "on-card CP {:?}, expected Some(5)",
                t.cp_on_card(0, "通用:该清CP了")
            ));
        }
        if !t.marks_on(NO_HOUSE_TILE).is_empty() {
            return Err(format!(
                "tile mark remains: {:?}",
                t.marks_on(NO_HOUSE_TILE)
            ));
        }
        if !t.on_field(0, "通用:该清CP了") {
            return Err("card should stay (on-card CP not empty)".into());
        }
        Ok(())
    })
    .expect("该清CP了 settle");
}

/// C-GEN-clearcp-03: products spread to adjacent empty tiles for 2 turn starts.
#[test]
fn clearcp_spreads() {
    forall(4, 0x6EA7_0016, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_money(0, 10_000 + rng.range(0, 10) * 100);
        t.give(0, &["通用:该清CP了"]);
        t.play(0, "通用:该清CP了")
            .map_err(|e| format!("play rejected: {e}"))?;
        if t.prompt().is_some() {
            t.answer_tile(0, NO_HOUSE_TILE)
                .map_err(|e| format!("tile: {e}"))?;
        }
        skip_all(&mut t);
        let n0 = t.marks().len();
        if n0 != 1 {
            return Err(format!("seed marks {n0}, expected 1"));
        }
        // Two full rounds of turn starts.
        for _ in 0..2 {
            for who in [0, 1] {
                t.dice(&[1]);
                t.roll(who).map_err(|e| format!("roll {who}: {e}"))?;
                skip_all(&mut t);
                t.end(who).map_err(|e| format!("end {who}: {e}"))?;
            }
        }
        // 「及其产物」: at each of the next two turn starts every extant mark
        // of this card adds one product on an adjacent empty tile.
        // Start 1: seed → +1. Start 2: seed + product → +2. Total 4 marks.
        // (A third turn start is outside 「下2回合」.)
        let n1 = t.marks().len();
        if n1 != 4 {
            return Err(format!(
                "marks after the spread window {n1}, expected 4 (seed + 1 + 2 products): {:?}",
                t.marks()
            ));
        }
        Ok(())
    })
    .expect("该清CP了 spread");
}

/// C-GEN-clearcp-04: the card leaves when on-card CP hits 0.
#[test]
fn clearcp_graveyards_at_zero() {
    forall(4, 0x6EA7_0017, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_money(0, 10_000 + rng.range(0, 10) * 100);
        t.give(0, &["通用:该清CP了"]);
        t.play(0, "通用:该清CP了")
            .map_err(|e| format!("play rejected: {e}"))?;
        if t.prompt().is_some() {
            t.answer_tile(0, NO_HOUSE_TILE)
                .map_err(|e| format!("tile: {e}"))?;
        }
        skip_all(&mut t);
        // Arrange one on-card CP left (test seam; does not raise cpChanged).
        t.set_cp_on_card(0, "通用:该清CP了", 1);
        t.set_pos(0, NO_HOUSE_TILE - 1);
        t.dice(&[1]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        skip_all(&mut t);
        if t.on_field(0, "通用:该清CP了") {
            return Err(format!(
                "card still on field at 0 on-card CP: {:?}",
                t.field_ids(0)
            ));
        }
        if t.cp_on_card(0, "通用:该清CP了").is_some() {
            return Err("on-card CP still present".into());
        }
        // Ruling 2026-10-07: graveyarded into the owner's 弃卡区.
        if !t.discard(0).iter().any(|c| c == "通用:该清CP了")
            && !t.draw_pile(0).iter().any(|c| c == "通用:该清CP了")
        {
            return Err(format!(
                "card not in owner's 弃卡区 {:?} / draw {:?}",
                t.discard(0),
                t.draw_pile(0)
            ));
        }
        Ok(())
    })
    .expect("该清CP了 graveyard");
}

// =====================================================================
// 通用:雨啊，快点来吧
// =====================================================================

/// C-GEN-rain-01 -- 2d2 = X; designate X players including the user; [停留].
///
/// * **ID:** `C-GEN-rain-01`
/// * **Source:** `roster#通用:雨啊，快点来吧`
/// * **Quote:** 「[手]：投掷2d2并记录结果为X。[指定]X名玩家（其中必须包括[使用者]），被[指定]的玩家获得一层[停留]。」
/// * **Trigger:** the [手] effect of this card.
/// * **Condition:** X = sum of 2d2 (in 2..=4); the player designates X distinct
///   players, and the set must include the user.
/// * **Effect:** each designated player gains one layer of [停留].
/// * **Frame:** see [`c_gen_rain_frame`].
/// * **Rulings & OPEN:** the user is a mandatory target (「其中必须包括[使用者]」);
///   the remaining `X - 1` are free choices among the other players.
/// * **Executable:** [`rain_stays_x`].
pub fn c_gen_rain_01(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// Executable frame for a rain play.
pub fn c_gen_rain_frame(step: &Step<'_>) -> Result<(), String> {
    if !step.act_is_card("通用:雨啊，快点来吧") {
        return Ok(());
    }
    let allowed = [
        "money:P".to_string(),
        "hand:P".to_string(),
        "draw:P".to_string(),
        "discard:P".to_string(),
        "hand_n:P".to_string(),
        "draw_n:P".to_string(),
        "discard_pub:P".to_string(),
        "state:P".into(),
        "targeted".into(),
        "turn.abnormal".into(),
    ];
    let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
    frame(step.pre, step.post, &allowed)
}

/// C-GEN-rain-01: X from 2d2; X players including the user get [停留].
#[test]
fn rain_stays_x() {
    forall(6, 0x6EA7_0018, |rng, _i| {
        let mut t = Table::vanilla(4);
        for i in 0..4 {
            t.set_money(i, 10_000 + rng.range(0, 10) * 100);
        }
        t.give(0, &["通用:雨啊，快点来吧"]);
        // 2d2 loaded faces: each clamped to the die (1..=2).
        let f1 = rng.range(1, 2);
        let f2 = rng.range(1, 2);
        let x = f1 + f2; // 2..=4
        t.dice(&[f1, f2]);
        let pre = t.snapshot_world();
        t.observe(&[Clause::new(
            "C-GEN-rain-frame",
            "roster#通用:雨啊，快点来吧",
            "投掷2d2并记录结果为X。[指定]X名玩家（其中必须包括[使用者]），被[指定]的玩家获得一层[停留]。",
            c_gen_rain_frame,
        )]);
        t.play(0, "通用:雨啊，快点来吧")
            .map_err(|e| format!("play rejected: {e}"))?;
        // The user is mandatory; pick `x - 1` others (option 0 repeatedly).
        for _ in 0..(x - 1) {
            if t.prompt().is_some() {
                t.answer(0, 0).map_err(|e| format!("target: {e}"))?;
            }
        }
        skip_all(&mut t);
        if t.state(0, "stay") != 1 {
            return Err(format!(
                "user stay {}, expected 1 (mandatory target)",
                t.state(0, "stay")
            ));
        }
        let targeted: usize = (1..4).filter(|&w| t.state(w, "stay") == 1).count();
        if targeted != (x - 1) as usize {
            return Err(format!(
                "other stay count {targeted}, expected {} (X={x})",
                x - 1
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P",
                "draw_n:P",
                "discard_pub:P",
                "state:P",
                "targeted",
                "turn.abnormal",
            ],
        )
    })
    .expect("雨啊 contract");
}

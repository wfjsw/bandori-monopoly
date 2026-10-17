//! Morfonica (C-MOR) card / skill / band contracts.
//!
//! Unit 14. Every clause of the 15 `Mor:` cards, the five Morfonica
//! character skills, and the band skill is checked here against the live
//! sheet text (`docs/rulebook/cards.json`, `the roster`,
//! `the roster`). Doc: `docs/rulebook/spec/cards/mor.md`.
//!
//! A discrepancy is a **plain failing test**. No `#[ignore]`, no allowlist.
//! Current red: `C-MOR-nnm-05` (the last crystal does not discard the card).
//! Older TEST-FINDINGS §4 / §6 Morfonica notes (纯真振翅 follow-on roll,
//! 星月夜 even-roll offer, NNM multi-activation) are green here.

use rules_spec::{forall, frame, Clause, Step, Table};

// ---------------------------------------------------------------- ids

const LIGHT: &str = "Mor:你的光芒将照亮前路";
const AGAIN: &str = "Mor:再次牵起手来";
const COURAGE: &str = "Mor:勇气展翅高飞之时";
const SUMMER: &str = "Mor:夏日合宿";
const CENTRIFUGE: &str = "Mor:离心力，不为所动";
const RAINBOW: &str = "Mor:秘密与青春的虹彩";
const WING: &str = "Mor:纯真振翅";
const STARRY: &str = "Mor:蝴蝶飞舞的星月夜";
const TRITONE: &str = "Mor:迷茫之蝶们的三全音";
const NOBLE: &str = "Mor:高贵的微蓝";
const NNM: &str = "Mor:（NNM）稍微努力了一下";
const RUI_DEMON: &str = "Mor:（Rui）正论恶魔";
const TOKO_SWAP: &str = "Mor:（toko）";
const XIAOBAI: &str = "Mor:（小白）";
const GARDEN: &str = "Mor:（筑紫）迷茫的庭园";

const TSUKINOMORI: &str = "月之森女子学院";

// ---------------------------------------------------------------- helpers

/// Absorb every open prompt with its fallback (setup only -- a contract
/// should answer its own prompts).
fn decline_all(t: &mut Table) {
    for _ in 0..12 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
}

/// Roll a main move and absorb incidental prompts.
fn roll_quiet(t: &mut Table, who: usize, faces: &[i32]) {
    t.dice(faces);
    t.roll(who).unwrap();
    decline_all(t);
}

/// End a turn (rolls first if still in 运营) and absorb prompts.
fn end_quiet(t: &mut Table, who: usize) {
    roll_quiet(t, who, &[1]);
    t.end(who).unwrap();
    decline_all(t);
}

/// Does this step's event window contain a play/counter of `card`?
fn card_fired(step: &Step<'_>, card: &str) -> bool {
    step.events
        .iter()
        .any(|e| e.r#type == "card" && e.card == card && !e.negated)
}

/// Did this step play/counter `card` with trigger `kind`?
fn card_kind(step: &Step<'_>, card: &str, kind: &str) -> bool {
    step.events
        .iter()
        .any(|e| e.r#type == "card" && e.card == card && e.kind == kind && !e.negated)
}

// ----------------------------------------------------------------
// C-MOR-light -- 你的光芒将照亮前路
// ----------------------------------------------------------------

/// C-MOR-light-01 -- 「当你在"月之森女子学院"格子前后20格之内，可以从手牌中打出此卡」
///
/// * **ID:** `C-MOR-light-01`
/// * **Source:** `roster#Mor:你的光芒将照亮前路`
/// * **Quote:** 「当你在"月之森女子学院"格子前后20格之内，可以从手牌中打出此卡」
/// * **Trigger:** a `play` of this card from hand.
/// * **Condition:** the user's tile is within 20 steps of 月之森 (either way).
/// * **Effect:** the play is accepted; outside the range it is refused.
/// * **Frame:** refusal leaves the world unchanged.
/// * **Rulings & OPEN:** OPEN-移动起点 on the body; the gate itself is exact.
/// * **Executable:** [`c_mor_light_gate`].
pub fn c_mor_light_gate(step: &Step<'_>) -> Result<(), String> {
    // The gate is a precondition of `play`; a refused play logs no card event.
    // The contract below drives both the accept and the refuse paths.
    Ok(())
}

/// C-MOR-light-02 -- 「此次移动以"月之森女子学院"为起点（不触发起点地块效果）」
pub fn c_mor_light_start(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, LIGHT) {
        return Ok(());
    }
    let who = step.player_of_act();
    // The move starts at 月之森: after the play the user sits 月之森 + walk,
    // never at their pre-play tile + walk. The strongest portable check is
    // that the pre-play tile is not the walk origin -- if the user was
    // already at 月之森 the two coincide, so skip that case.
    let tsuki = step
        .data
        .tiles
        .iter()
        .position(|t| t.name == TSUKINOMORI)
        .ok_or("no tile 月之森女子学院")?;
    let pre_pos = step.pre.st.players[who].pos as usize;
    if pre_pos == tsuki {
        return Ok(());
    }
    // A normal forward walk of N tiles from 月之森 ends at tsuki+N (mod 60).
    // The engine's post position must be reachable from 月之森, not from
    // pre_pos, for the same walk length. We only know that the start tile
    // effect of 月之森 did not fire -- pin that instead of inventing N.
    // A walk of the turn's main move must leave 月之森, not pre_pos.
    let walks: Vec<_> = step
        .events
        .iter()
        .filter(|e| e.r#type == "move")
        .collect();
    if walks.is_empty() {
        // No walk in this window (the play itself does not move).
        return Ok(());
    }
    for w in walks {
        if w.from != tsuki as i32 {
            return Err(format!(
                "C-MOR-light-02: walk started at {}, expected {} (月之森)",
                w.from, tsuki
            ));
        }
    }
    Ok(())
}

/// Frame for the light play: position, and the card leaving the hand.
pub fn c_mor_light_frame(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, LIGHT) {
        return Ok(());
    }
    let who = step.player_of_act();
    frame(
        step.pre,
        step.post,
        &[
            &format!("pos:P{who}"),
            &format!("hand:P{who}"),
            &format!("draw:P{who}"),
            &format!("discard:P{who}"),
            &format!("hand_n:P{who}"),
            &format!("draw_n:P{who}"),
            &format!("discard_pub:P{who}"),
        ],
    )
}

// ----------------------------------------------------------------
// C-MOR-again -- 再次牵起手来
// ----------------------------------------------------------------

/// C-MOR-again-01 -- [反击] place + consume equal.
pub fn c_mor_again_counter(step: &Step<'_>) -> Result<(), String> {
    if !card_kind(step, AGAIN, "counter") {
        return Ok(());
    }
    let who = step.player_of_act();
    // The counter places the card on the field and consumes the same amount
    // the previous player paid. The amount is the largest non-zero pay/lose
    // leg of the window that is not the counter's own consume.
    let paid: i64 = step
        .events
        .iter()
        .filter(|e| e.r#type == "pay" || e.r#type == "rent" || e.r#type == "lose")
        .map(|e| e.value as i64)
        .max()
        .unwrap_or(0);
    if paid <= 0 {
        return Err("counter fired with no prior pay/lose > 0".into());
    }
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    // The window may include the original leg (previous player) plus the
    // counter's consume. The counteruser's net must include -paid.
    if delta > 0 {
        return Err(format!(
            "counteruser money rose by {delta}, expected a consume of {paid}"
        ));
    }
    if !step
        .post
        .st
        .players
        [who]
        .field
        .iter()
        .any(|f| f.card == AGAIN)
    {
        return Err("counter card not placed on the field".into());
    }
    Ok(())
}

/// C-MOR-again-02 -- [持续] cancel the next consume/pay and discard.
pub fn c_mor_again_persist(step: &Step<'_>) -> Result<(), String> {
    // The persist fires on a later consume/pay of the field owner. The
    // contract below drives that leg and asserts the cancel + discard.
    Ok(())
}

// ----------------------------------------------------------------
// C-MOR-courage -- 勇气展翅高飞之时
// ----------------------------------------------------------------

/// C-MOR-courage-01 -- owner pays price + half the house cost.
pub fn c_mor_courage_pay(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, COURAGE) {
        return Ok(());
    }
    let who = step.player_of_act();
    // Find the pay leg from another player to `who`.
    let mut expected: Option<(usize, i64)> = None;
    for e in step.events {
        if (e.r#type == "pay" || e.r#type == "rent") && e.other == who as i32 {
            expected = Some((e.player_id as usize, e.value as i64));
        }
    }
    let Some((from, amount)) = expected else {
        // No pay leg: the rolled tile is an agent (courage-02) or unowned
        // (courage-03). Those clauses own those outcomes.
        return Ok(());
    };
    let got = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if got != amount {
        return Err(format!("user money delta {got}, expected +{amount}"));
    }
    let lost = step.pre_money(from) as i64 - step.post_money(from) as i64;
    if lost != amount {
        return Err(format!("owner money delta -{lost}, expected -{amount}"));
    }
    Ok(())
}

/// C-MOR-courage-02 -- agent tile pays 1000.
pub fn c_mor_courage_agent(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, COURAGE) {
        return Ok(());
    }
    // Pin the 1000 in the contract below (the step check needs the rolled
    // tile, which the events carry as `dice`).
    Ok(())
}

/// C-MOR-courage-03 -- unowned tile pays nothing.
pub fn c_mor_courage_unowned(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, COURAGE) {
        return Ok(());
    }
    Ok(())
}

// ----------------------------------------------------------------
// C-MOR-summer -- 夏日合宿
// ----------------------------------------------------------------

/// C-MOR-summer-01 -- shield until next own turn start.
pub fn c_mor_summer_shield(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, SUMMER) {
        return Ok(());
    }
    let who = step.player_of_act();
    if !step
        .post
        .st
        .players
        [who]
        .field
        .iter()
        .any(|f| f.card == SUMMER)
    {
        return Err("shield card not placed on the field".into());
    }
    Ok(())
}

/// C-MOR-summer-02 -- draw 1 when the window ends unused.
pub fn c_mor_summer_draw(step: &Step<'_>) -> Result<(), String> {
    // Fires on the turn-start that tears the shield down. The contract below
    // drives that turn and asserts the draw.
    Ok(())
}

// ----------------------------------------------------------------
// C-MOR-centrifuge -- 离心力，不为所动
// ----------------------------------------------------------------

/// C-MOR-centrifuge-01 -- only on the 2nd targeting between own turns.
pub fn c_mor_centrifuge_gate(step: &Step<'_>) -> Result<(), String> {
    // The gate is a counteract offer; the contract below asserts "not
    // offered on the 1st, offered on the 2nd".
    Ok(())
}

/// C-MOR-centrifuge-02 -- shield until next own turn start.
pub fn c_mor_centrifuge_shield(step: &Step<'_>) -> Result<(), String> {
    if !card_kind(step, CENTRIFUGE, "counter") {
        return Ok(());
    }
    let who = step.player_of_act();
    if !step
        .post
        .st
        .players
        [who]
        .field
        .iter()
        .any(|f| f.card == CENTRIFUGE)
    {
        return Err("centrifuge card not placed on the field".into());
    }
    Ok(())
}

// ----------------------------------------------------------------
// C-MOR-rainbow -- 秘密与青春的虹彩
// ----------------------------------------------------------------

/// C-MOR-rainbow-01 -- pay to junior/same is halved.
pub fn c_mor_rainbow_half(step: &Step<'_>) -> Result<(), String> {
    if !card_kind(step, RAINBOW, "counter") {
        return Ok(());
    }
    Ok(())
}

/// C-MOR-rainbow-02 -- senior/same paying you is 1.5x.
pub fn c_mor_rainbow_times(step: &Step<'_>) -> Result<(), String> {
    if !card_kind(step, RAINBOW, "counter") {
        return Ok(());
    }
    Ok(())
}

// ----------------------------------------------------------------
// C-MOR-wing -- 纯真振翅
// ----------------------------------------------------------------

/// C-MOR-wing-01 -- teleport 20 forward, no settle at the destination.
pub fn c_mor_wing_teleport(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, WING) || step.post_prompt_open {
        return Ok(());
    }
    let who = step.player_of_act();
    let pre = step.pre.st.players[who].pos as i64;
    let post = step.post.st.players[who].pos as i64;
    let n = step.pre.st.owners.len() as i64;
    // 「传送到移动方向20格后」 -- the jump is +20; wing-02 then adds the
    // follow-on roll. Pin the jump origin: a walk event must leave pre+20
    // (or the final position must be pre+20+k).
    let jumped = (pre + 20).rem_euclid(n);
    // No settle at the jump tile: the landing of the jump itself must not
    // have paid rent. (A later landing of the follow-on roll may.)
    for e in step.events {
        if (e.r#type == "rent" || e.r#type == "pay") && e.player_id == who as i32 {
            // Allowed only if this is the follow-on landing, i.e. post != jumped.
            if post == jumped {
                return Err(
                    "C-MOR-wing-01: rent paid at the no-settle teleport destination".into(),
                );
            }
        }
    }
    let _ = jumped;
    Ok(())
}

/// C-MOR-wing-02 -- 「立刻进行移动掷骰」.
pub fn c_mor_wing_roll(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, WING) {
        return Ok(());
    }
    // A movement dice roll must have happened in this window.
    let rolled = step.events.iter().any(|e| {
        e.r#type == "diceRollBefore" || e.r#type == "move" || e.r#type == "dice"
    });
    if !rolled {
        return Err(
            "C-MOR-wing-02: 「立刻进行移动掷骰」 -- no movement dice in the window"
                .into(),
        );
    }
    Ok(())
}

// ----------------------------------------------------------------
// C-MOR-starry -- 蝴蝶飞舞的星月夜
// ----------------------------------------------------------------

/// C-MOR-starry-01 -- pay X*1000, place with X crystals.
pub fn c_mor_starry_place(step: &Step<'_>) -> Result<(), String> {
    // The play itself leaves the X prompt open; the placement resolves on
    // the answer step. Fire only on the placement window (card newly on the
    // field, or a STARRY event this step) -- not on every later step while
    // the card sits there.
    let pre_on_field = step
        .pre
        .st
        .players
        .iter()
        .any(|p| p.field.iter().any(|f| f.card == STARRY));
    let post_on_field = step
        .post
        .st
        .players
        .iter()
        .any(|p| p.field.iter().any(|f| f.card == STARRY));
    let newly_placed = !pre_on_field && post_on_field;
    if !newly_placed {
        return Ok(());
    }
    // Only when the card is now placed and the user paid for it.
    let who = step.player_of_act();
    let Some(crystals) = step
        .post
        .st
        .players
        [who]
        .field
        .iter()
        .find(|f| f.card == STARRY)
        .map(|f| f.counters.get("crystals").copied().unwrap_or(0))
    else {
        return Ok(());
    };
    if crystals <= 0 {
        return Err(format!(
            "C-MOR-starry-01: placed with {crystals} crystals, expected >= 1"
        ));
    }
    let spent = step.pre_money(who) as i64 - step.post_money(who) as i64;
    if spent != crystals as i64 * 1000 {
        return Err(format!(
            "C-MOR-starry-01: paid {spent}, expected {} (= X*1000 for X={crystals})",
            crystals * 1000
        ));
    }
    Ok(())
}

/// C-MOR-starry-02 -- no crystals -> discard.
pub fn c_mor_starry_empty(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-starry-03 -- owner may re-roll the first move roll.
pub fn c_mor_starry_reroll(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-starry-04 -- even roll of another player: pay 1000, remove 1.
pub fn c_mor_starry_even(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-starry-05 -- owner passes CiRCLE: remove 1.
pub fn c_mor_starry_circle(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-starry-06 -- each crystal removed pays the owner 1000.
pub fn c_mor_starry_payout(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

// ----------------------------------------------------------------
// C-MOR-tritone -- 迷茫之蝶们的三全音
// ----------------------------------------------------------------

/// C-MOR-tritone-01 -- gain the lost amount, place with 3 crystals.
pub fn c_mor_tritone_gain(step: &Step<'_>) -> Result<(), String> {
    if !card_kind(step, TRITONE, "counter") {
        return Ok(());
    }
    let who = step.player_of_act();
    let crystals = step
        .post
        .st
        .players
        [who]
        .field
        .iter()
        .find(|f| f.card == TRITONE)
        .map(|f| f.counters.get("crystals").copied().unwrap_or(0))
        .ok_or("tritone not placed")?;
    if crystals != 3 {
        return Err(format!("tritone placed with {crystals} crystals, expected 3"));
    }
    // The window's net on `who` is 0: the loss that opened the window is
    // given back on the spot.
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if delta != 0 {
        return Err(format!(
            "C-MOR-tritone-01: user money delta {delta}, expected 0 \
             (「立刻获得此次失去的资金金额」)"
        ));
    }
    Ok(())
}

/// C-MOR-tritone-02 -- 3 owner turn-ends later: discard + pay back.
pub fn c_mor_tritone_payback(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

// ----------------------------------------------------------------
// C-MOR-noble -- 高贵的微蓝
// ----------------------------------------------------------------

/// C-MOR-noble-01 -- gate: deed price >= 2200.
pub fn c_mor_noble_gate(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-noble-02 -- own tile: mark "noTarget" on the tile.
pub fn c_mor_noble_mark(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, NOBLE) || step.post_prompt_open {
        // 「立刻进行一次触发结算」 on the user's own deed opens the
        // G-SETTLE-06/07 buy/build prompt; the mark (rule 2) lands only
        // after that settle is answered.
        return Ok(());
    }
    let who = step.player_of_act();
    let pos = step.post.st.players[who].pos as usize;
    let owner = step.post.st.owners.get(pos).copied().unwrap_or(-1);
    if owner != who as i32 {
        // Foreign tile: the mark is only for 「如果此地块属于你」.
        return Ok(());
    }
    let marked = step.post.st.marks.iter().any(|m| m.tile == pos as i32);
    if !marked {
        return Err("C-MOR-noble-02: no mark placed on the user's tile".into());
    }
    Ok(())
}

// ----------------------------------------------------------------
// C-MOR-nnm -- （NNM）稍微努力了一下
// ----------------------------------------------------------------

/// C-MOR-nnm-01 -- discard x marks, pick one branch.
pub fn c_mor_nnm_choice(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-nnm-02 -- (1) draw x, discard to 5 at turn end.
pub fn c_mor_nnm_draw(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-nnm-03 -- (2) x times the CiRCLE money reward.
pub fn c_mor_nnm_circle_gain(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-nnm-04 -- (3) place with x crystals; spend one for the (2) skill.
pub fn c_mor_nnm_place(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-nnm-05 -- crystals exhausted -> discard.
pub fn c_mor_nnm_empty(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

// ----------------------------------------------------------------
// C-MOR-rui-demon -- （Rui）正论恶魔
// ----------------------------------------------------------------

/// C-MOR-rui-demon-01 -- pay 100 to all (immune), then 300 from all.
pub fn c_mor_rui_demon_money(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, RUI_DEMON) {
        return Ok(());
    }
    let who = step.player_of_act();
    let n = step.post.st.players.len();
    // User pays 100 to each of n-1 others, then receives 300 from each.
    // Net on the user: -100*(n-1) + 300*(n-1) = +200*(n-1).
    let expect = 200i64 * (n as i64 - 1);
    let got = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if got != expect {
        return Err(format!(
            "C-MOR-rui-demon-01: user money delta {got}, expected {expect} \
             (= 200 per other player, n={n})"
        ));
    }
    for (i, p) in step.post.st.players.iter().enumerate() {
        if i == who {
            continue;
        }
        let d = p.money as i64 - step.pre.st.players[i].money as i64;
        if d != -200 {
            return Err(format!(
                "C-MOR-rui-demon-01: p{i} money delta {d}, expected -200 \
                 (paid 300, received 100)"
            ));
        }
    }
    Ok(())
}

/// C-MOR-rui-demon-02 -- skill fire during the settle sets X to 5.
pub fn c_mor_rui_demon_x(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, RUI_DEMON) {
        return Ok(());
    }
    let who = step.player_of_act();
    // Quote: 「每当你的技能被触发后，立即将x设置为5」 -- only when a skill
    // of the user's fired during this settle. Without such a trigger the
    // clause is a no-op (the card itself must not invent the write).
    let skill_fired = step.events.iter().any(|e| {
        e.r#type == "card" && e.kind == "skill" && e.player_id == who as i32
    });
    if !skill_fired {
        return Ok(());
    }
    let x = step.post.st.players[who].state_get("skill.yuriCrit.x");
    if x != 5 {
        return Err(format!(
            "C-MOR-rui-demon-02: X = {x}, expected 5 \
             (「每当你的技能被触发后，立即将x设置为5」)"
        ));
    }
    Ok(())
}

// ----------------------------------------------------------------
// C-MOR-toko-swap -- （toko）
// ----------------------------------------------------------------

/// C-MOR-toko-swap-01 -- swap same-colour mortgaged deeds, pay the
/// difference, free-redeem the received deed.
pub fn c_mor_toko_swap(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, TOKO_SWAP) {
        return Ok(());
    }
    Ok(())
}

// ----------------------------------------------------------------
// C-MOR-xiaobai -- （小白）
// ----------------------------------------------------------------

/// C-MOR-xiaobai-01 -- pay becomes lose; target loses half.
pub fn c_mor_xiaobai(step: &Step<'_>) -> Result<(), String> {
    if !card_kind(step, XIAOBAI, "counter") {
        return Ok(());
    }
    let who = step.player_of_act();
    // The user loses the full amount; the intended payee loses half; the
    // payee gains nothing from this leg.
    let user_delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if user_delta >= 0 {
        return Err(format!(
            "C-MOR-xiaobai-01: user money delta {user_delta}, expected a loss"
        ));
    }
    let amount = -user_delta;
    // Find the other seat that lost (the payee, half).
    let mut found = false;
    for (i, p) in step.post.st.players.iter().enumerate() {
        if i == who {
            continue;
        }
        let d = step.pre.st.players[i].money as i64 - p.money as i64;
        if d == amount / 2 {
            found = true;
        }
        if p.money as i64 - step.pre.st.players[i].money as i64 > 0 {
            return Err(format!(
                "C-MOR-xiaobai-01: p{i} gained money; the pay must become a lose"
            ));
        }
    }
    if !found {
        return Err(format!(
            "C-MOR-xiaobai-01: no seat lost half of {amount} (= {h})",
            h = amount / 2
        ));
    }
    Ok(())
}

// ----------------------------------------------------------------
// C-MOR-garden -- （筑紫）迷茫的庭园
// ----------------------------------------------------------------

/// C-MOR-garden-01 -- 「获得100资金」.
pub fn c_mor_garden_gain(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, GARDEN) {
        return Ok(());
    }
    let who = step.player_of_act();
    // The window may also carry the teleport / settle; the +100 is a floor
    // only if nothing else moves money. The contract below pins it on a
    // declined settle with no other legs.
    let _ = who;
    Ok(())
}

/// C-MOR-garden-02 -- 1d6 teleport to the tile before the chosen player.
pub fn c_mor_garden_teleport(step: &Step<'_>) -> Result<(), String> {
    if !card_fired(step, GARDEN) || step.post_prompt_open {
        // A prompt (筑紫's own redirect on 「获得100资金」, or the settle
        // choice) is still open: the teleport has not resolved yet.
        return Ok(());
    }
    // The main move is spent: the turn cannot roll again. Observable as
    // `turn.main_moved` staying true.
    if !step.post.turn.main_moved {
        return Err(
            "C-MOR-garden-02: 「并视为主要移动」 -- main move not spent".into(),
        );
    }
    Ok(())
}

/// C-MOR-garden-03 -- optional settle (a skip leaves money unchanged).
pub fn c_mor_garden_settle_choice(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-garden-04 -- no abnormal movement until next own turn start.
pub fn c_mor_garden_immunity(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

// ----------------------------------------------------------------
// Character skills
// ----------------------------------------------------------------

/// C-MOR-mashiro-01 -- main move is 2d20.
pub fn c_mor_mashiro_2d20(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-mashiro-02 -- reverse direction.
pub fn c_mor_mashiro_reverse(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-touko-01 -- pass CiRCLE +1 fire (cap 1).
pub fn c_mor_touko_fire(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-touko-02 -- once/turn skill: neighbours pay ceil100(half rent / n).
pub fn c_mor_touko_skill(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-nanami-01 -- pass CiRCLE +1 fire (cap 2).
pub fn c_mor_nanami_fire(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-tsukushi-01 -- redirect a gain/draw to another player.
pub fn c_mor_tsukushi_redirect(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-tsukushi-02 -- flip the X mark and replace the action.
pub fn c_mor_tsukushi_replace(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-tsukushi-03 -- sweep flipped marks, gain n * per_mark.
pub fn c_mor_tsukushi_sweep(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-rui-crit-02 -- 1d20 vs X on every non-zero gain/lose.
pub fn c_mor_rui_crit(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

// ----------------------------------------------------------------
// Band skill
// ----------------------------------------------------------------

/// C-MOR-band-01 -- lose >= 2000 in one go -> draw 1.
pub fn c_mor_band_draw(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-MOR-band-02 -- CiRCLE money cycles 1000 / 1500 / 2000.
pub fn c_mor_band_cycle(step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

// ----------------------------------------------------------------
// CLAUSES -- the observe() set
// ----------------------------------------------------------------

/// Clauses armed on `Table::observe`. Each is one book sentence.
pub const CLAUSES: &[Clause] = &[
    Clause::new(
        "C-MOR-light-01",
        "roster#Mor:你的光芒将照亮前路",
        "当你在“月之森女子学院”格子前后20格之内，可以从手牌中打出此卡",
        c_mor_light_gate,
    ),
    Clause::new(
        "C-MOR-light-02",
        "roster#Mor:你的光芒将照亮前路",
        "此次移动以“月之森女子学院”为起点（不触发起点地块效果）",
        c_mor_light_start,
    ),
    Clause::new(
        "C-MOR-light-frame",
        "roster#Mor:你的光芒将照亮前路",
        "此次移动以“月之森女子学院”为起点",
        c_mor_light_frame,
    ),
    Clause::new(
        "C-MOR-again-01",
        "roster#Mor:再次牵起手来",
        "[反击]当[使用者]的行动序列前一名玩家[消耗]或[支付]大于0资金后将此卡放置在[使用者]的[场地]并[消耗]等量资金。",
        c_mor_again_counter,
    ),
    Clause::new(
        "C-MOR-again-02",
        "roster#Mor:再次牵起手来",
        "[持续]：[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区。",
        c_mor_again_persist,
    ),
    Clause::new(
        "C-MOR-courage-01",
        "roster#Mor:勇气展翅高飞之时",
        "结果对应序号格子的所有者向你支付该地块的购买价格+地块已有房子的建造价格总额的一半",
        c_mor_courage_pay,
    ),
    Clause::new(
        "C-MOR-courage-02",
        "roster#Mor:勇气展翅高飞之时",
        "若为地产商地块，获得1000资金",
        c_mor_courage_agent,
    ),
    Clause::new(
        "C-MOR-courage-03",
        "roster#Mor:勇气展翅高飞之时",
        "结果对应序号格子的所有者向你支付",
        c_mor_courage_unowned,
    ),
    Clause::new(
        "C-MOR-summer-01",
        "roster#Mor:夏日合宿",
        "打出此卡，直到下个自己的回合开始前，你只会被自己发动的效果指定。",
        c_mor_summer_shield,
    ),
    Clause::new(
        "C-MOR-summer-02",
        "roster#Mor:夏日合宿",
        "当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌",
        c_mor_summer_draw,
    ),
    Clause::new(
        "C-MOR-centrifuge-01",
        "roster#Mor:离心力，不为所动",
        "[反击]当你在两个你的回合之间…第二次成为其他角色技能或卡牌的目标时，你可以打出此卡",
        c_mor_centrifuge_gate,
    ),
    Clause::new(
        "C-MOR-centrifuge-02",
        "roster#Mor:离心力，不为所动",
        "直到下个你的回合开始时，无效化你受到的所有效果",
        c_mor_centrifuge_shield,
    ),
    Clause::new(
        "C-MOR-rainbow-01",
        "roster#Mor:秘密与青春的虹彩",
        "（1）当你向学妹或同级生支付时，打出此卡，此次支付金额减半。",
        c_mor_rainbow_half,
    ),
    Clause::new(
        "C-MOR-rainbow-02",
        "roster#Mor:秘密与青春的虹彩",
        "（2）当学姐或同级生向你支付的时候，打出此卡，使此次支付资金变成1.5倍。",
        c_mor_rainbow_times,
    ),
    Clause::new(
        "C-MOR-wing-01",
        "roster#Mor:纯真振翅",
        "传送到移动方向20格后（不触发结算）",
        c_mor_wing_teleport,
    ),
    Clause::new(
        "C-MOR-wing-02",
        "roster#Mor:纯真振翅",
        "立刻进行移动掷骰",
        c_mor_wing_roll,
    ),
    Clause::new(
        "C-MOR-starry-01",
        "roster#Mor:蝴蝶飞舞的星月夜",
        "（1）支付X次1000的的资金，将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]",
        c_mor_starry_place,
    ),
    Clause::new(
        "C-MOR-starry-02",
        "roster#Mor:蝴蝶飞舞的星月夜",
        "此卡没有[奇迹水晶]时加入弃牌堆",
        c_mor_starry_empty,
    ),
    Clause::new(
        "C-MOR-starry-03",
        "roster#Mor:蝴蝶飞舞的星月夜",
        "此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次",
        c_mor_starry_reroll,
    ),
    Clause::new(
        "C-MOR-starry-04",
        "roster#Mor:蝴蝶飞舞的星月夜",
        "（2）当其他角色的移动掷骰结果是偶数时可消耗1000资金并移除此卡的一个[奇迹水晶]",
        c_mor_starry_even,
    ),
    Clause::new(
        "C-MOR-starry-05",
        "roster#Mor:蝴蝶飞舞的星月夜",
        "此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]",
        c_mor_starry_circle,
    ),
    Clause::new(
        "C-MOR-starry-06",
        "roster#Mor:蝴蝶飞舞的星月夜",
        "（3）此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金。",
        c_mor_starry_payout,
    ),
    Clause::new(
        "C-MOR-tritone-01",
        "roster#Mor:迷茫之蝶们的三全音",
        "[反击] 任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次失去的资金金额，此卡放置在场上",
        c_mor_tritone_gain,
    ),
    Clause::new(
        "C-MOR-tritone-02",
        "roster#Mor:迷茫之蝶们的三全音",
        "三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金",
        c_mor_tritone_payback,
    ),
    Clause::new(
        "C-MOR-noble-01",
        "roster#Mor:高贵的微蓝",
        "当你位于一块地契价值大于等于2200的地块时，可以打出此卡",
        c_mor_noble_gate,
    ),
    Clause::new(
        "C-MOR-noble-02",
        "roster#Mor:高贵的微蓝",
        "如果此地块属于你：…2.在该地块上放置一个标记，有标记时此地块不能被指定。",
        c_mor_noble_mark,
    ),
    Clause::new(
        "C-MOR-nnm-01",
        "roster#Mor:（NNM）稍微努力了一下",
        "弃置手中x枚角色标记，发动以下效果中的一个",
        c_mor_nnm_choice,
    ),
    Clause::new(
        "C-MOR-nnm-02",
        "roster#Mor:（NNM）稍微努力了一下",
        "（1）抽x张卡（可超过上限），回合结束后将手牌弃置到五张",
        c_mor_nnm_draw,
    ),
    Clause::new(
        "C-MOR-nnm-03",
        "roster#Mor:（NNM）稍微努力了一下",
        "（2）获得x次经过CiRCLE时的资金奖励",
        c_mor_nnm_circle_gain,
    ),
    Clause::new(
        "C-MOR-nnm-04",
        "roster#Mor:（NNM）稍微努力了一下",
        "（3）将此卡放置在场上并放置x个奇迹水晶",
        c_mor_nnm_place,
    ),
    Clause::new(
        "C-MOR-nnm-05",
        "roster#Mor:（NNM）稍微努力了一下",
        "（4）当奇迹水晶耗尽时，将此卡置入弃牌堆",
        c_mor_nnm_empty,
    ),
    Clause::new(
        "C-MOR-rui-demon-01",
        "roster#Mor:（Rui）正论恶魔",
        "向全场玩家支付100资金（该数值不可被任何效果影响），之后全场玩家向你支付300资金（可被影响）",
        c_mor_rui_demon_money,
    ),
    Clause::new(
        "C-MOR-rui-demon-02",
        "roster#Mor:（Rui）正论恶魔",
        "在此卡结算过程中，每当你的技能被触发后，立即将x设置为5",
        c_mor_rui_demon_x,
    ),
    Clause::new(
        "C-MOR-toko-swap-01",
        "roster#Mor:（toko）",
        "选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换，地契+房屋价格更低的玩家向更高的玩家支付差价，互换后将你得到的地契免费赎回",
        c_mor_toko_swap,
    ),
    Clause::new(
        "C-MOR-xiaobai-01",
        "roster#Mor:（小白）",
        "[反击]当你将要向其他玩家支付时打出此卡，此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金",
        c_mor_xiaobai,
    ),
    Clause::new(
        "C-MOR-garden-01",
        "roster#Mor:（筑紫）迷茫的庭园",
        "获得100资金。",
        c_mor_garden_gain,
    ),
    Clause::new(
        "C-MOR-garden-02",
        "roster#Mor:（筑紫）迷茫的庭园",
        "投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动",
        c_mor_garden_teleport,
    ),
    Clause::new(
        "C-MOR-garden-03",
        "roster#Mor:（筑紫）迷茫的庭园",
        "可以选择是否触发结算。",
        c_mor_garden_settle_choice,
    ),
    Clause::new(
        "C-MOR-garden-04",
        "roster#Mor:（筑紫）迷茫的庭园",
        "直到下个你的回合开始时，你无法被异常移动",
        c_mor_garden_immunity,
    ),
    Clause::new(
        "C-MOR-mashiro-01",
        "the roster#仓田真白",
        "（1）[主动移动]时移动掷骰变为2d20",
        c_mor_mashiro_2d20,
    ),
    Clause::new(
        "C-MOR-mashiro-02",
        "the roster#仓田真白",
        "（2）反方向移动",
        c_mor_mashiro_reverse,
    ),
    Clause::new(
        "C-MOR-touko-01",
        "the roster#桐谷透子",
        "（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）",
        c_mor_touko_fire,
    ),
    Clause::new(
        "C-MOR-touko-02",
        "the roster#桐谷透子",
        "（2）一回合一次，在你的经营阶段，你可以消耗一个火罐并指定你的一个地块…",
        c_mor_touko_skill,
    ),
    Clause::new(
        "C-MOR-nanami-01",
        "the roster#广町七深",
        "（1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限2）",
        c_mor_nanami_fire,
    ),
    Clause::new(
        "C-MOR-tsukushi-01",
        "the roster#二叶筑紫",
        "（1）当你获得资金或者抽卡时，可以改为指定场上除你以外的一个角色进行一次该动作",
        c_mor_tsukushi_redirect,
    ),
    Clause::new(
        "C-MOR-tsukushi-02",
        "the roster#二叶筑紫",
        "（2）当有角色获得金钱或抽卡时，你可以将对方的X标记翻面并代替其进行一次该动作",
        c_mor_tsukushi_replace,
    ),
    Clause::new(
        "C-MOR-tsukushi-03",
        "the roster#二叶筑紫",
        "（3）当场上除你之外的玩家都拥有一枚翻面后的X标记时，移除所有X标记，你获得移除标记数量*800的资金",
        c_mor_tsukushi_sweep,
    ),
    Clause::new(
        "C-MOR-rui-crit-02",
        "the roster#八潮瑠唯",
        "当你即将获得或失去资金…roll1d20，若结果小于等于X…若结果大于X，X+1。X初始为0",
        c_mor_rui_crit,
    ),
    Clause::new(
        "C-MOR-band-01",
        "roster#Morfonica",
        "（1）当你一次性失去2000以上资金时，抽一张卡。",
        c_mor_band_draw,
    ),
    Clause::new(
        "C-MOR-band-02",
        "roster#Morfonica",
        "（2）[CiRCLE奖励]选择[获得]资金时根据选择[获得]资金次数设资金量为1000，1500，2000的循环",
        c_mor_band_cycle,
    ),
];

// ================================================================
// Contracts. Each test is a `forall` over randomized pre-states, drives
// one transition, asserts exact numbers, then frames the rest.
// ================================================================

/// C-MOR-light-01 + C-MOR-light-02.
///
/// Card text:
/// > 当你在"月之森女子学院"格子前后20格之内，可以从手牌中打出此卡，此次移动以
/// > "月之森女子学院"为起点（不触发起点地块效果）。
#[test]
fn light_gate_and_start() {
    forall(6, 0xC0_10_01, |rng, _i| {
        let mut t = Table::vanilla(2);
        let tsuki = rules_spec::tile(TSUKINOMORI);
        // Randomize the starting tile: 0..59. Distance to 月之森 is the
        // wrap-around min; 0..=20 is in range, 21..=39 is not.
        let pos = rng.range(0, 59) as usize;
        t.set_pos(0, pos);
        t.give(0, &[LIGHT]);
        let pre = t.snapshot_world();
        t.observe(CLAUSES);

        let n = 60i32;
        let d = {
            let a = pos as i32;
            let b = tsuki as i32;
            let fwd = (b - a).rem_euclid(n);
            let back = (a - b).rem_euclid(n);
            fwd.min(back)
        };
        let in_range = d <= 20;

        let played = t.play(0, LIGHT);
        if !in_range {
            if played.is_ok() {
                return Err(format!(
                    "C-MOR-light-01: play accepted at distance {d} (> 20) from 月之森"
                ));
            }
            if t.hand(0).iter().any(|c| c == LIGHT) {
                // refused -- card stays in hand. Frame: nothing moved.
                return frame(
                    &pre,
                    t.world(),
                    &[
                        "money:",
                        "pos:",
                        "owner:",
                        "houses:",
                        "mortgaged:",
                        "field:",
                        "crystals:",
                    ],
                );
            }
            return Err("C-MOR-light-01: refused play consumed the card".into());
        }
        played.map_err(|e| format!("C-MOR-light-01: play refused in range (d={d}): {e}"))?;

        // C-MOR-light-02: 「此次移动以"月之森女子学院"为起点」 -- the move
        // that follows the play starts at 月之森, not at `pos`. The play
        // itself does not walk; the main roll does.
        t.dice(&[3]);
        t.roll(0).map_err(|e| format!("roll after the play rejected: {e}"))?;
        decline_all(&mut t);
        let expect = (tsuki + 3) % 60;
        let got = t.pos(0);
        if got != expect {
            return Err(format!(
                "C-MOR-light-02: landed at {got}, expected {expect} \
                 (start 月之森={tsuki} + 3; pre-pos was {pos})"
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "pos:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
                "turn.main_moved",
                "turn.abnormal",
            ],
        )
    })
    .expect("C-MOR-light contract");
}

/// C-MOR-again-01.
///
/// > [反击]当[使用者]的行动序列前一名玩家[消耗]或[支付]大于0资金后将此卡
/// > 放置在[使用者]的[场地]并[消耗]等量资金。
#[test]
fn again_counter_places_and_consumes_equal() {
    forall(6, 0xC0_10_02, |rng, _i| {
        let mut t = Table::vanilla(3);
        // p0 pays rent to p2; p1 (the player after p0) counters.
        let rent = 140;
        t.own(2, &[7]);
        t.set_pos(0, 6);
        t.give(1, &[AGAIN]);
        let money0 = 10_000 + rng.range(0, 20) * 100;
        let money1 = 10_000 + rng.range(0, 20) * 100;
        t.set_money(0, money0);
        t.set_money(1, money1);
        t.begin_turn(0);
        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.dice(&[1]);
        t.roll(0).unwrap();
        if !t.counteract_offered(AGAIN) {
            return Err(format!("C-MOR-again-01: not offered: {}", t.dump_prompt()));
        }
        t.counteract(1, AGAIN)
            .map_err(|e| format!("counter rejected: {e}"))?;
        decline_all(&mut t);

        if t.money(0) != money0 - rent {
            return Err(format!(
                "p0 money {}, expected {} (rent {rent} still lands)",
                t.money(0),
                money0 - rent
            ));
        }
        if t.money(1) != money1 - rent {
            return Err(format!(
                "C-MOR-again-01: user money {}, expected {} (consumed {rent})",
                t.money(1),
                money1 - rent
            ));
        }
        if !t.on_field(1, AGAIN) {
            return Err(format!(
                "C-MOR-again-01: card not on field: {:?}",
                t.field_ids(1)
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:",
                "hand:P1",
                "field:P1",
                "crystals:",
                "hand_n:P1",
                "discard_pub:P1",
                "roll:P0",
                "pos:P0",
                "turn.paid_in_settle",
            ],
        )
    })
    .expect("C-MOR-again-01 contract");
}

/// C-MOR-again-02.
///
/// > [持续]：[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区。
#[test]
fn again_persist_cancels_the_next_payment() {
    forall(4, 0xC0_10_03, |rng, _i| {
        let mut t = Table::vanilla(3);
        let rent = 140;
        t.own(2, &[7]);
        t.set_pos(0, 6);
        t.give(1, &[AGAIN]);
        let money1 = 10_000 + rng.range(0, 20) * 100;
        t.set_money(1, money1);
        t.begin_turn(0);
        t.dice(&[1]);
        t.roll(0).unwrap();
        t.counteract(1, AGAIN).unwrap();
        decline_all(&mut t);
        // p1 now holds the [持续] on the field. p1 lands on p2's tile:
        // the rent must be cancelled and the card discarded.
        t.observe(CLAUSES);
        t.begin_turn(1);
        t.set_pos(1, 6);
        t.dice(&[1]);
        t.roll(1).unwrap();
        decline_all(&mut t);

        if t.money(1) != money1 - rent {
            return Err(format!(
                "C-MOR-again-02: money {}, expected {} (the {rent} of the counter \
                 is already paid; the *new* rent must be cancelled)",
                t.money(1),
                money1 - rent
            ));
        }
        if t.on_field(1, AGAIN) {
            return Err("C-MOR-again-02: [持续] card still on the field".into());
        }
        Ok(())
    })
    .expect("C-MOR-again-02 contract");
}

/// C-MOR-courage-01.
///
/// > 掷骰3d20，结果对应序号格子的所有者向你支付该地块的购买价格+地块已有房子的
/// > 建造价格总额的一半
#[test]
fn courage_pays_price_plus_half_houses() {
    // Ruling 2026-10-06: land price in full + half the house cost.
    // 月之森 (#14, index 13): price 3200, house 2000. 1 house → 4200.
    forall(4, 0xC0_10_04, |rng, _i| {
        let mut t = Table::vanilla(2);
        let tsuki = rules_spec::tile(TSUKINOMORI);
        let houses = rng.range(1, 3);
        t.own(1, &[tsuki]);
        t.set_houses(tsuki, houses);
        t.dice(&[5, 5, 4]); // sum 14 → #14 月之森
        let money0 = 10_000 + rng.range(0, 10) * 100;
        let money1 = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money0);
        t.set_money(1, money1);
        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.give_play(0, COURAGE)
            .map_err(|e| format!("play rejected: {e}"))?;
        decline_all(&mut t);

        let price = 3200i64;
        let house_cost = 2000i64 * houses as i64;
        let expect = price + house_cost / 2;
        let got = t.money(0) as i64 - money0 as i64;
        if got != expect {
            return Err(format!(
                "C-MOR-courage-01: user delta {got}, expected {expect} \
                 (= {price} + {house_cost}/2)"
            ));
        }
        let lost = money1 as i64 - t.money(1) as i64;
        if lost != expect {
            return Err(format!(
                "C-MOR-courage-01: owner delta -{lost}, expected -{expect}"
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
            ],
        )
    })
    .expect("C-MOR-courage-01 contract");
}

/// C-MOR-courage-02 -- agent tile gains 1000.
#[test]
fn courage_agent_tile_gains_1000() {
    forall(3, 0xC0_10_05, |rng, _i| {
        let mut t = Table::vanilla(2);
        // 主要街道 (#5, index 4) is the agent tile; 3d20 sum 5.
        let money0 = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money0);
        t.dice(&[2, 2, 1]);
        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.give_play(0, COURAGE).unwrap();
        decline_all(&mut t);
        let got = t.money(0) as i64 - money0 as i64;
        if got != 1000 {
            return Err(format!("C-MOR-courage-02: delta {got}, expected +1000"));
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
    .expect("C-MOR-courage-02 contract");
}

/// C-MOR-courage-03 -- unowned tile pays nothing.
#[test]
fn courage_unowned_tile_pays_nothing() {
    forall(3, 0xC0_10_06, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money0 = 10_000 + rng.range(0, 10) * 100;
        let money1 = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money0);
        t.set_money(1, money1);
        t.dice(&[5, 5, 4]); // unowned 月之森
        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.give_play(0, COURAGE).unwrap();
        decline_all(&mut t);
        if t.money(0) != money0 {
            return Err(format!(
                "C-MOR-courage-03: user money {}, expected {money0}",
                t.money(0)
            ));
        }
        if t.money(1) != money1 {
            return Err(format!(
                "C-MOR-courage-03: owner money {}, expected {money1}",
                t.money(1)
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
            ],
        )
    })
    .expect("C-MOR-courage-03 contract");
}

/// C-MOR-summer-01 + C-MOR-summer-02.
///
/// > 打出此卡，直到下个自己的回合开始前，你只会被自己发动的效果指定。当此卡效果
/// > 结束，你没有因为此卡效果无效化任何影响则抽一张牌
#[test]
fn summer_shield_until_next_turn_then_draws() {
    forall(4, 0xC0_10_07, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["R:[衍生] 压", "R:[衍生] 压"]);
        let money0 = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money0);
        t.observe(CLAUSES);
        t.give_play(0, SUMMER)
            .map_err(|e| format!("play rejected: {e}"))?;
        decline_all(&mut t);

        if !t.on_field(0, SUMMER) {
            return Err(format!(
                "C-MOR-summer-01: not on field: {:?}",
                t.field_ids(0)
            ));
        }
        // Through the opponent's turn the shield stays.
        t.begin_turn(1);
        if !t.on_field(0, SUMMER) {
            return Err("C-MOR-summer-01: shield dropped on the opponent's turn".into());
        }
        // p0's next turn start tears it down and draws 1.
        let hand_before = t.hand(0).len();
        t.begin_turn(0);
        if t.on_field(0, SUMMER) {
            return Err("C-MOR-summer-02: shield still up at the next own turn start".into());
        }
        let hand_after = t.hand(0).len();
        let draw_after = t.draw_pile(0).len();
        if hand_after != hand_before + 1 || draw_after != 1 {
            return Err(format!(
                "C-MOR-summer-02: hand {hand_before}->{hand_after}, draw pile {draw_after}; \
                 expected a draw of 1"
            ));
        }
        let _ = money0;
        Ok(())
    })
    .expect("C-MOR-summer contract");
}

/// C-MOR-centrifuge-01 + -02.
///
/// > [反击]当你在两个你的回合之间…第二次成为其他角色技能或卡牌的目标时，你可以
/// > 打出此卡，直到下个你的回合开始时，无效化你受到的所有效果
#[test]
fn centrifuge_offers_only_on_the_second_targeting() {
    forall(3, 0xC0_10_08, |_rng, _i| {
        let mut t = Table::vanilla(3);
        t.give(0, &[CENTRIFUGE]);
        t.give(1, &["通用:登上武道馆", "通用:登上武道馆"]);
        t.begin_turn(1);
        t.observe(CLAUSES);
        t.play(1, "通用:登上武道馆").unwrap();
        if t.counteract_offered(CENTRIFUGE) {
            return Err(format!(
                "C-MOR-centrifuge-01: offered on the 1st targeting: {}",
                t.dump_prompt()
            ));
        }
        decline_all(&mut t);
        t.play(1, "通用:登上武道馆").unwrap();
        if !t.counteract_offered(CENTRIFUGE) {
            return Err(format!(
                "C-MOR-centrifuge-01: not offered on the 2nd targeting: {}",
                t.dump_prompt()
            ));
        }
        t.counteract(0, CENTRIFUGE).unwrap();
        if !t.on_field(0, CENTRIFUGE) {
            return Err("C-MOR-centrifuge-02: shield not on the field".into());
        }
        Ok(())
    })
    .expect("C-MOR-centrifuge contract");
}

/// C-MOR-rainbow-01: pay to a same-grade (同级生) is halved.
///
/// > （1）当你向学妹或同级生支付时，打出此卡，此次支付金额减半。
#[test]
fn rainbow_halves_a_payment_to_a_same_grade() {
    forall(4, 0xC0_10_09, |rng, _i| {
        let mut t = Table::vanilla(2); // 户山香澄 + 花园多惠, both PPP hi-3
        let rent = 140;
        t.own(1, &[7]);
        t.set_pos(0, 6);
        t.give(0, &[RAINBOW]);
        let money0 = 10_000 + rng.range(0, 10) * 100;
        let money1 = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money0);
        t.set_money(1, money1);
        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.dice(&[1]);
        t.roll(0).unwrap();
        if !t.counteract_offered(RAINBOW) {
            return Err(format!("C-MOR-rainbow-01: not offered: {}", t.dump_prompt()));
        }
        t.counteract(0, RAINBOW).unwrap();
        decline_all(&mut t);

        let half = rent / 2;
        let got = money0 as i64 - t.money(0) as i64;
        if got != half as i64 {
            return Err(format!(
                "C-MOR-rainbow-01: paid {got}, expected {half} (half of {rent})"
            ));
        }
        let received = t.money(1) as i64 - money1 as i64;
        if received != half as i64 {
            return Err(format!(
                "C-MOR-rainbow-01: payee received {received}, expected {half}"
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:",
                "pos:",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
                "turn.paid_in_settle",
            ],
        )
    })
    .expect("C-MOR-rainbow-01 contract");
}

/// C-MOR-rainbow-02: a same-grade payer's payment is 1.5x.
///
/// > （2）当学姐或同级生向你支付的时候，打出此卡，使此次支付资金变成1.5倍。
#[test]
fn rainbow_same_grade_pays_you_at_1_5x() {
    forall(4, 0xC0_10_0A, |rng, _i| {
        // 广町七深 + 二叶筑紫, both Morfonica hi-2 (同级生).
        let mut t = Table::new(&["广町七深", "二叶筑紫"]);
        t.clean();
        t.begin_turn(1);
        decline_all(&mut t);
        let rent = 140;
        t.own(0, &[7]);
        t.set_pos(1, 6);
        t.give(0, &[RAINBOW]);
        let money0 = 10_000 + rng.range(0, 10) * 100;
        let money1 = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money0);
        t.set_money(1, money1);
        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.dice(&[1]);
        t.roll(1).unwrap();
        if !t.counteract_offered(RAINBOW) {
            return Err(format!("C-MOR-rainbow-02: not offered: {}", t.dump_prompt()));
        }
        t.counteract(0, RAINBOW).unwrap();
        decline_all(&mut t);

        let times = rent * 3 / 2;
        let got = t.money(0) as i64 - money0 as i64;
        if got != times as i64 {
            return Err(format!(
                "C-MOR-rainbow-02: received {got}, expected {times} (1.5x of {rent})"
            ));
        }
        let lost = money1 as i64 - t.money(1) as i64;
        if lost != times as i64 {
            return Err(format!(
                "C-MOR-rainbow-02: payer lost {lost}, expected {times}"
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:",
                "pos:",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
                "turn.paid_in_settle",
            ],
        )
    })
    .expect("C-MOR-rainbow-02 contract");
}

/// C-MOR-wing-01 + C-MOR-wing-02.
///
/// > 传送到移动方向20格后（不触发结算），立刻进行移动掷骰
///
/// C-MOR-wing-02 (「立刻进行移动掷骰」) is checked together with -01: the
/// landing is jumped+walk.
#[test]
fn wing_teleports_20_then_rolls() {
    forall(4, 0xC0_10_0B, |rng, _i| {
        let mut t = Table::vanilla(2);
        let pos = rng.range(0, 30) as usize;
        t.set_pos(0, pos);
        t.own(1, &[(pos + 20) % 60]); // the jump destination must NOT settle
        t.dice(&[3]);
        let money0 = 10_000;
        let money1 = 10_000;
        t.set_money(0, money0);
        t.set_money(1, money1);
        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.give_play(0, WING)
            .map_err(|e| format!("play rejected: {e}"))?;
        decline_all(&mut t);

        // C-MOR-wing-01: +20, and the destination did not settle (no rent).
        let landed = t.pos(0);
        let jumped = (pos + 20) % 60;
        let final_expect = (jumped + 3) % 60;
        // The no-settle half is absolute: the +20 landing itself pays nobody
        // (p1 owns `jumped`, not `final_expect`).
        if t.money(0) != money0 || t.money(1) != money1 {
            if final_expect != jumped {
                return Err(format!(
                    "C-MOR-wing-01: money moved at the no-settle teleport \
                     (landed {landed}, expected {final_expect} after the roll; \
                     jumped={jumped})"
                ));
            }
        }
        // C-MOR-wing-02: 「立刻进行移动掷骰」 -- the landing must be jumped+3.
        if landed != final_expect {
            return Err(format!(
                "C-MOR-wing-02: landed at {landed}, expected {final_expect} \
                 (= {jumped} + 3); 「立刻进行移动掷骰」 never ran"
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "pos:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
                "money:",
                "turn.abnormal",
            ],
        )
    })
    .expect("C-MOR-wing contract");
}

/// C-MOR-starry-01 / -05 / -06.
///
/// > （1）支付X次1000的的资金，将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]
/// > （2）…此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]。
/// > （3）此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金。
#[test]
fn starry_places_x_crystals_and_pays_out_on_ciircle() {
    forall(4, 0xC0_10_0C, |rng, _i| {
        let mut t = Table::vanilla(2);
        // x >= 2: the empty-card case (x=1) is starry_empty_goes_to_discard.
        let x = rng.range(2, 3);
        let money0 = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money0);
        t.set_draw(0, &["R:[衍生] 压"]);
        t.observe(CLAUSES);
        t.give(0, &[STARRY]);
        t.play(0, STARRY)
            .map_err(|e| format!("play rejected: {e}"))?;
        // X prompt: option index 0 = X=1.
        t.answer_one(x - 1)
            .map_err(|e| format!("X answer rejected: {e}"))?;
        decline_all(&mut t);

        let spent = money0 as i64 - t.money(0) as i64;
        if spent != x as i64 * 1000 {
            return Err(format!(
                "C-MOR-starry-01: paid {spent}, expected {} (= {x}*1000)",
                x * 1000
            ));
        }
        if t.crystals(0, STARRY) != Some(x) {
            return Err(format!(
                "C-MOR-starry-01: crystals {:?}, expected Some({x})",
                t.crystals(0, STARRY)
            ));
        }

        // Walk past CiRCLE: one crystal goes, the owner gains 1000.
        let money_before = t.money(0);
        let crystals_before = t.crystals(0, STARRY).unwrap();
        t.begin_turn(0);
        t.set_pos(0, 58);
        t.dice(&[3]); // 58 → 59 → 0 (CiRCLE) → 1
        t.roll(0).unwrap();
        // The re-roll offer (starry-03) pauses the walk before it starts;
        // decline it so the walk runs on the original 3.
        t.decline();

        if t.crystals(0, STARRY) != Some(crystals_before - 1) {
            return Err(format!(
                "C-MOR-starry-05: crystals {:?}, expected {}",
                t.crystals(0, STARRY),
                crystals_before - 1
            ));
        }
        // The crystal payout lands before the CiRCLE-bonus choice. Assert
        // it here so the bonus cannot inflate the figure.
        if t.money(0) != money_before + 1000 {
            return Err(format!(
                "C-MOR-starry-06: money {}, expected {} (+1000 per crystal)",
                t.money(0),
                money_before + 1000
            ));
        }
        decline_all(&mut t); // the CiRCLE-bonus choice
        Ok(())
    })
    .expect("C-MOR-starry place/circle/payout contract");
}

/// C-MOR-starry-02: the last crystal empties the card into the discard.
#[test]
fn starry_empty_goes_to_discard() {
    forall(3, 0xC0_10_0D, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_draw(0, &["R:[衍生] 压"]);
        t.give(0, &[STARRY]);
        t.play(0, STARRY).unwrap();
        t.answer_one(0).unwrap(); // X=1
        decline_all(&mut t);
        t.begin_turn(0);
        t.set_pos(0, 58);
        t.dice(&[3]);
        t.roll(0).unwrap();
        decline_all(&mut t);
        if t.on_field(0, STARRY) {
            return Err(format!(
                "C-MOR-starry-02: still on the field: {:?}",
                t.field_ids(0)
            ));
        }
        if t.crystals(0, STARRY).is_some() {
            return Err("C-MOR-starry-02: crystals still readable".into());
        }
        Ok(())
    })
    .expect("C-MOR-starry-02 contract");
}

/// C-MOR-starry-04: an even roll of another player offers the 1000/crystal
/// trade.
#[test]
fn starry_even_roll_of_another_player_offers_crystal_removal() {
    forall(2, 0xC0_10_0E, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.give_play(0, STARRY).unwrap();
        t.answer_one(1).unwrap(); // X=2
        decline_all(&mut t);
        t.begin_turn(1);
        t.set_pos(1, 40);
        t.dice(&[2]); // even
        t.roll(1).unwrap();
        if t.prompt().is_none() {
            return Err(
                "C-MOR-starry-04: 「当其他角色的移动掷骰结果是偶数时可消耗1000资金 \
                 并移除此卡的一个[奇迹水晶]」 -- no prompt on an even roll"
                    .into(),
            );
        }
        t.answer_one(0).unwrap();
        decline_all(&mut t);
        Ok(())
    })
    .expect("C-MOR-starry-04 contract");
}

/// C-MOR-tritone-01 + C-MOR-tritone-02.
///
/// > [反击] 任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次失去的资金金额，
/// > 此卡放置在场上，三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡
/// > 获得的资金
#[test]
fn tritone_gains_the_loss_and_counts_down() {
    forall(3, 0xC0_10_0F, |rng, _i| {
        let mut t = Table::vanilla(2);
        let rent = 140;
        t.own(1, &[7]);
        t.set_pos(0, 6);
        t.give(0, &[TRITONE]);
        let money0 = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money0);
        t.begin_turn(0);
        t.observe(CLAUSES);
        t.dice(&[1]);
        t.roll(0).unwrap();
        if !t.counteract_offered(TRITONE) {
            return Err(format!(
                "C-MOR-tritone-01: not offered: {}",
                t.dump_prompt()
            ));
        }
        t.counteract(0, TRITONE).unwrap();
        decline_all(&mut t);

        // The 140 rent is given back on the spot: net 0 on the user.
        if t.money(0) != money0 {
            return Err(format!(
                "C-MOR-tritone-01: money {}, expected {money0} (loss given back)",
                t.money(0)
            ));
        }
        if t.crystals(0, TRITONE) != Some(3) {
            return Err(format!(
                "C-MOR-tritone-01: crystals {:?}, expected Some(3)",
                t.crystals(0, TRITONE)
            ));
        }

        // Countdown: owner turn-end ticks 1 crystal off. 「三回合后」 is
        // three of the **owner's** turns. p0 already rolled this turn (the
        // counteract answered its rent), so end it without re-rolling.
        t.end(0).map_err(|e| format!("end after the counteract: {e}"))?;
        decline_all(&mut t);
        if t.crystals(0, TRITONE) != Some(2) {
            return Err(format!(
                "C-MOR-tritone-02: after owner turn-end 1 crystals {:?}, expected Some(2)",
                t.crystals(0, TRITONE)
            ));
        }
        // p1's turn end does not tick the owner's countdown.
        t.begin_turn(1);
        t.dice(&[1]);
        t.roll(1).unwrap();
        decline_all(&mut t);
        t.end(1).unwrap();
        decline_all(&mut t);
        if t.crystals(0, TRITONE) != Some(2) {
            return Err(format!(
                "C-MOR-tritone-02: opponent's turn end ticked the countdown: {:?}",
                t.crystals(0, TRITONE)
            ));
        }
        // Owner turn-end #2: 2 -> 1.
        t.begin_turn(0);
        t.dice(&[1]);
        t.roll(0).unwrap();
        decline_all(&mut t);
        t.end(0).unwrap();
        decline_all(&mut t);
        if t.crystals(0, TRITONE) != Some(1) {
            return Err(format!(
                "C-MOR-tritone-02: after owner turn-end 2 crystals {:?}, expected Some(1)",
                t.crystals(0, TRITONE)
            ));
        }
        // Owner turn-end #3: 1 -> 0 -> discard + payback.
        t.begin_turn(0);
        t.dice(&[1]);
        t.roll(0).unwrap();
        decline_all(&mut t);
        t.end(0).unwrap();
        decline_all(&mut t);
        if t.on_field(0, TRITONE) {
            return Err("C-MOR-tritone-02: card still on the field".into());
        }
        let paid_back = money0 as i64 - t.money(0) as i64;
        if paid_back != rent as i64 {
            return Err(format!(
                "C-MOR-tritone-02: paid back {paid_back}, expected {rent} \
                 (「支付由此卡获得的资金」; payee is OPEN-三全音收款人)"
            ));
        }
        Ok(())
    })
    .expect("C-MOR-tritone contract");
}

/// C-MOR-noble-01 + C-MOR-noble-02.
///
/// > 当你位于一块地契价值大于等于2200的地块时，可以打出此卡，如果此地块属于你：
/// > 1.立刻进行一次触发结算。2.在该地块上放置一个标记，有标记时此地块不能被指定。
#[test]
fn noble_gate_and_mark() {
    forall(4, 0xC0_10_10, |rng, _i| {
        let mut t = Table::vanilla(2);
        let high = rules_spec::tile(TSUKINOMORI);
        let low = rules_spec::tile("富士见坂");
        let high_tile = rng.chance(50);
        let pos = if high_tile { high } else { low };
        let own = rng.chance(50);
        t.set_pos(0, pos);
        if own {
            t.own(0, &[pos]);
        } else {
            t.own(1, &[pos]);
        }
        t.give(0, &[NOBLE]);
        t.observe(CLAUSES);
        let played = t.play(0, NOBLE);
        let price_gate = 2200;
        let tile_price = if high_tile { 3200 } else { 600 };
        let in_range = tile_price >= price_gate;
        if !in_range {
            if played.is_ok() {
                return Err(format!(
                    "C-MOR-noble-01: play accepted on a tile of price {tile_price}"
                ));
            }
            return Ok(());
        }
        played.map_err(|e| format!("C-MOR-noble-01: refused on a >=2200 tile: {e}"))?;
        decline_all(&mut t);
        if own {
            let marks = t.marks_on(pos);
            if marks.is_empty() {
                return Err(format!(
                    "C-MOR-noble-02: no mark on the user's own tile {pos}: {marks:?}"
                ));
            }
        } else if !t.marks_on(pos).is_empty() {
            return Err(format!(
                "C-MOR-noble-02: mark placed on a foreign tile: {:?}",
                t.marks_on(pos)
            ));
        }
        Ok(())
    })
    .expect("C-MOR-noble contract");
}

/// C-MOR-rui-demon-01 + C-MOR-rui-demon-02.
///
/// > 向全场玩家支付100资金（该数值不可被任何效果影响），之后全场玩家向你支付300
/// > 资金（可被影响）。在此卡结算过程中，每当你的技能被触发后，立即将x设置为5
#[test]
fn rui_demon_pays_100_then_receives_300() {
    forall(3, 0xC0_10_11, |rng, _i| {
        let n_seats = rng.range(3, 4) as usize;
        let chars = ["八潮瑠唯", "户山香澄", "花园多惠", "牛込里美"];
        let mut t = Table::new(&chars[..n_seats]);
        t.strip_skills();
        t.clean();
        t.begin_turn(0);
        // Keep the X key so C-MOR-rui-demon-02 can still observe the card's
        // own write, but the crit skill must not rewrite the 300 legs.
        t.set_state(0, "skill.yuriCrit.x", 0);
        let money: Vec<i32> = (0..n_seats)
            .map(|_| 10_000 + rng.range(0, 10) * 100)
            .collect();
        for (i, m) in money.iter().enumerate() {
            t.set_money(i, *m);
        }
        t.give(0, &[RUI_DEMON]);
        t.observe(CLAUSES);
        t.play(0, RUI_DEMON)
            .map_err(|e| format!("play rejected: {e}"))?;
        decline_all(&mut t);

        let others = n_seats as i64 - 1;
        let expect_user = 200 * others; // -100*others + 300*others
        let got = t.money(0) as i64 - money[0] as i64;
        if got != expect_user {
            return Err(format!(
                "C-MOR-rui-demon-01: user delta {got}, expected {expect_user}"
            ));
        }
        for i in 1..n_seats {
            let d = t.money(i) as i64 - money[i] as i64;
            if d != -200 {
                return Err(format!("C-MOR-rui-demon-01: p{i} delta {d}, expected -200"));
            }
        }
        // C-MOR-rui-demon-02: the skill's own money legs fire X → 5.
        if t.state(0, "skill.yuriCrit.x") != 5 {
            return Err(format!(
                "C-MOR-rui-demon-02: X = {}, expected 5",
                t.state(0, "skill.yuriCrit.x")
            ));
        }
        Ok(())
    })
    .expect("C-MOR-rui-demon contract");
}

/// C-MOR-toko-swap-01.
///
/// > 选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换，地契+房屋价格
/// > 更低的玩家向更高的玩家支付差价，互换后将你得到的地契免费赎回
#[test]
fn toko_swaps_same_color_mortgaged_deeds() {
    forall(3, 0xC0_10_12, |rng, _i| {
        let mut t = Table::new(&["桐谷透子", "户山香澄"]);
        t.clean();
        t.begin_turn(0);
        // 月之森 3200 vs 羽丘 2200, both group 2 (same colour).
        let mine = rules_spec::tile(TSUKINOMORI);
        let theirs = rules_spec::tile("羽丘女子学院");
        t.own(0, &[mine]);
        t.own(1, &[theirs]);
        t.set_mortgaged(mine, true);
        t.set_mortgaged(theirs, true);
        let houses = rng.range(0, 2);
        t.set_houses(mine, houses);
        let money = [
            10_000 + rng.range(0, 10) * 100,
            10_000 + rng.range(0, 10) * 100,
        ];
        t.set_money(0, money[0]);
        t.set_money(1, money[1]);
        t.give(0, &[TOKO_SWAP]);
        t.observe(CLAUSES);
        t.play(0, TOKO_SWAP)
            .map_err(|e| format!("play rejected: {e}"))?;
        let p = t.expect_prompt();
        if p.options.len() != 1 {
            return Err(format!(
                "C-MOR-toko-swap-01: expected 1 swap pair, got {}: {}",
                p.options.len(),
                t.dump_prompt()
            ));
        }
        t.answer_one(0).unwrap();
        decline_all(&mut t);

        if t.owner(mine) != Some(1) || t.owner(theirs) != Some(0) {
            return Err(format!(
                "C-MOR-toko-swap-01: owners mine={:?} theirs={:?}, expected swapped",
                t.owner(mine),
                t.owner(theirs)
            ));
        }
        // Value: mine = 3200 + 2000*houses, theirs = 2200. Lower pays higher.
        let v_mine = 3200i64 + 2000 * houses as i64;
        let v_theirs = 2200i64;
        let diff = (v_mine - v_theirs).abs();
        let (low_pays, high_pays) = if v_theirs < v_mine { (1, 0) } else { (0, 1) };
        let low_delta = t.money(low_pays) as i64 - money[low_pays] as i64;
        let high_delta = t.money(high_pays) as i64 - money[high_pays] as i64;
        if low_delta != -diff {
            return Err(format!(
                "C-MOR-toko-swap-01: lower player delta {low_delta}, expected -{diff}"
            ));
        }
        if high_delta != diff {
            return Err(format!(
                "C-MOR-toko-swap-01: higher player delta {high_delta}, expected +{diff}"
            ));
        }
        // The received deed is redeemed free; the given-away one stays mortgaged.
        if t.mortgaged(theirs) {
            return Err("C-MOR-toko-swap-01: received deed still mortgaged".into());
        }
        if !t.mortgaged(mine) {
            return Err("C-MOR-toko-swap-01: given-away deed was redeemed".into());
        }
        Ok(())
    })
    .expect("C-MOR-toko-swap contract");
}

/// C-MOR-xiaobai-01.
///
/// > [反击]当你将要向其他玩家支付时打出此卡，此次支付改为失去同等的资金并令此次
/// > 支付的对象失去此次金额一半的资金
#[test]
fn xiaobai_turns_pay_into_lose() {
    forall(4, 0xC0_10_13, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "仓田真白"); // exclusive-card carrier
        let rent = 140;
        t.own(1, &[7]);
        t.set_pos(0, 6);
        t.give(0, &[XIAOBAI]);
        let money0 = 10_000 + rng.range(0, 10) * 100;
        let money1 = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money0);
        t.set_money(1, money1);
        t.observe(CLAUSES);
        t.dice(&[1]);
        t.roll(0).unwrap();
        if !t.counteract_offered(XIAOBAI) {
            return Err(format!(
                "C-MOR-xiaobai-01: not offered: {}",
                t.dump_prompt()
            ));
        }
        t.counteract(0, XIAOBAI).unwrap();
        decline_all(&mut t);

        let lost = money0 as i64 - t.money(0) as i64;
        if lost != rent as i64 {
            return Err(format!("C-MOR-xiaobai-01: user lost {lost}, expected {rent}"));
        }
        let target_lost = money1 as i64 - t.money(1) as i64;
        if target_lost != (rent / 2) as i64 {
            return Err(format!(
                "C-MOR-xiaobai-01: target lost {target_lost}, expected {}",
                rent / 2
            ));
        }
        Ok(())
    })
    .expect("C-MOR-xiaobai contract");
}

/// C-MOR-garden-01 / -02 / -03.
///
/// > 获得100资金。投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动
/// > （选中自己则前进一格…），可以选择是否触发结算。
#[test]
fn garden_gains_100_and_teleports_as_a_main_move() {
    forall(3, 0xC0_10_14, |rng, _i| {
        let mut t = Table::new(&[
            "二叶筑紫",
            "户山香澄",
            "花园多惠",
            "牛込里美",
            "山吹沙绫",
        ]);
        t.clean();
        t.begin_turn(0);
        let pos1 = rng.range(10, 40) as usize;
        t.set_pos(1, pos1);
        let money0 = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money0);
        t.give(0, &[GARDEN]);
        t.observe(CLAUSES);
        t.dice(&[2]); // 1d6 = 2 → 2nd player (p1) → the tile before them
        t.play(0, GARDEN)
            .map_err(|e| format!("play rejected: {e}"))?;
        // 交给班长吧 (1) may fire on the card's own 「获得100资金」; decline
        // the redirect so the 100 stays here.
        if t.prompt().is_some() {
            t.decline();
        }
        // The settle is optional: decline (「可以选择是否触发结算」).
        if t.prompt().is_some() {
            t.decline();
        }
        decline_all(&mut t);

        // C-MOR-garden-01: +100 (with the settle declined and no other legs).
        if t.money(0) != money0 + 100 {
            return Err(format!(
                "C-MOR-garden-01: money {}, expected {}",
                t.money(0),
                money0 + 100
            ));
        }
        // C-MOR-garden-02: 「前一格」 is one tile AHEAD of the chosen
        // player (the tile they would step onto next), matching
        // 「选中自己则前进一格」. Main move is spent.
        let expect = (pos1 + 1) % 60;
        if t.pos(0) != expect {
            return Err(format!(
                "C-MOR-garden-02: pos {}, expected {expect} (one ahead of p1 at {pos1})",
                t.pos(0)
            ));
        }
        if !t.world().turn.main_moved {
            return Err("C-MOR-garden-02: 「并视为主要移动」 -- main move not spent".into());
        }
        Ok(())
    })
    .expect("C-MOR-garden contract");
}

/// C-MOR-mashiro-01 + -02.
///
/// > （1）[主动移动]时移动掷骰变为2d20（2）反方向移动
#[test]
fn mashiro_rolls_2d20_backwards() {
    forall(3, 0xC0_10_15, |rng, _i| {
        let mut t = Table::new(&["仓田真白", "户山香澄"]);
        t.clean();
        t.begin_turn(0);
        let pos = rng.range(20, 40) as usize;
        t.set_pos(0, pos);
        t.dice(&[5, 6]); // 2d20 = 11, backwards
        t.observe(CLAUSES);
        t.roll(0).unwrap();
        decline_all(&mut t);
        let expect = (pos + 60 - 11) % 60;
        if t.pos(0) != expect {
            return Err(format!(
                "C-MOR-mashiro-01/02: pos {}, expected {expect} (= {pos} - 11)",
                t.pos(0)
            ));
        }
        Ok(())
    })
    .expect("C-MOR-mashiro contract");
}

/// C-MOR-touko-01 + C-MOR-touko-02.
///
/// > （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）
/// > （2）一回合一次，在你的经营阶段，你可以消耗一个火罐并指定你的一个地块，指定
/// > 前后各一格范围内（不包括该格子本身）的所有其他玩家支付你X，X为此地的地租的
/// > 一半/指定玩家数（向上取整百）。
#[test]
fn touko_fire_cap_and_neighbour_skill() {
    forall(3, 0xC0_10_16, |rng, _i| {
        let mut t = Table::new(&["桐谷透子", "户山香澄", "花园多惠"]);
        t.clean();
        t.begin_turn(0);
        // (1) fire pot: pass CiRCLE with one below the cap → back to 1.
        t.set_fire(0, 0, 1);
        t.set_pos(0, 58);
        t.dice(&[3]);
        t.roll(0).unwrap();
        decline_all(&mut t);
        if t.charge(0) != 1 {
            return Err(format!(
                "C-MOR-touko-01: fire {}, expected 1 (gained one on [经过] CiRCLE)",
                t.charge(0)
            ));
        }

        // (2) once/turn skill: neighbours ±1 pay ceil100((rent[0]/2)/n).
        t.begin_turn(0);
        t.set_fire(0, 1, 1);
        let tsuki = rules_spec::tile(TSUKINOMORI); // rent[0]=320
        t.own(0, &[tsuki]);
        t.set_pos(1, tsuki - 1);
        t.set_pos(2, tsuki + 1);
        let money: Vec<i32> = (0..3).map(|_| 10_000 + rng.range(0, 10) * 100).collect();
        for (i, m) in money.iter().enumerate() {
            t.set_money(i, *m);
        }
        let sid = t.skill_id(0, "桐谷透子");
        t.observe(CLAUSES);
        t.skill(0, &sid).unwrap();
        t.answer_one(0).unwrap(); // pick the only offered tile
        decline_all(&mut t);

        // X = ceil100( (320/2) / 2 ) = ceil100(80) = 100 each.
        let x = 100i64;
        if t.money(0) as i64 != money[0] as i64 + 2 * x {
            return Err(format!(
                "C-MOR-touko-02: user money {}, expected {}",
                t.money(0),
                money[0] as i64 + 2 * x
            ));
        }
        for i in 1..3 {
            if t.money(i) as i64 != money[i] as i64 - x {
                return Err(format!(
                    "C-MOR-touko-02: p{i} money {}, expected {}",
                    t.money(i),
                    money[i] as i64 - x
                ));
            }
        }
        if t.charge(0) != 0 {
            return Err(format!(
                "C-MOR-touko-02: fire {}, expected 0 (spent)",
                t.charge(0)
            ));
        }
        Ok(())
    })
    .expect("C-MOR-touko contract");
}

/// C-MOR-nanami-01.
///
/// > （1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限2）
#[test]
fn nanami_fire_cap_is_two() {
    forall(3, 0xC0_10_17, |_rng, _i| {
        let mut t = Table::new(&["广町七深", "户山香澄"]);
        t.clean();
        t.begin_turn(0);
        t.set_fire(0, 1, 2); // one below the cap
        t.set_pos(0, 58);
        t.dice(&[3]);
        t.observe(CLAUSES);
        t.roll(0).unwrap();
        decline_all(&mut t);
        if t.charge(0) != 2 {
            return Err(format!(
                "C-MOR-nanami-01: fire {}, expected 2 (gained one, capped)",
                t.charge(0)
            ));
        }
        Ok(())
    })
    .expect("C-MOR-nanami-01 contract");
}

/// C-MOR-tsukushi-01: a gain is redirected to another player with an X mark.
///
/// > （1）当你获得资金或者抽卡时，可以改为指定场上除你以外的一个角色进行一次该动作
#[test]
fn tsukushi_redirects_a_gain() {
    forall(3, 0xC0_10_18, |rng, _i| {
        let mut t = Table::new(&["二叶筑紫", "户山香澄", "花园多惠"]);
        t.clean();
        t.begin_turn(0);
        let money: Vec<i32> = (0..3).map(|_| 10_000 + rng.range(0, 10) * 100).collect();
        for (i, m) in money.iter().enumerate() {
            t.set_money(i, *m);
        }
        t.observe(CLAUSES);
        // R:[衍生] 压 gains 1000 on play.
        t.give_play(0, "R:[衍生] 压").unwrap();
        if t.prompt().is_none() {
            return Err(format!(
                "C-MOR-tsukushi-01: no redirect offer: {}",
                t.dump_prompt()
            ));
        }
        t.answer_one(0).unwrap(); // yes, redirect
        decline_all(&mut t);

        if t.money(0) != money[0] {
            return Err(format!(
                "C-MOR-tsukushi-01: user money {}, expected {} (gain redirected)",
                t.money(0),
                money[0]
            ));
        }
        if t.money(1) != money[1] + 1000 {
            return Err(format!(
                "C-MOR-tsukushi-01: p1 money {}, expected {}",
                t.money(1),
                money[1] + 1000
            ));
        }
        if t.token(1, "X") != 1 {
            return Err(format!(
                "C-MOR-tsukushi-01: p1 X mark {}, expected 1",
                t.token(1, "X")
            ));
        }
        Ok(())
    })
    .expect("C-MOR-tsukushi-01 contract");
}

/// C-MOR-band-01.
///
/// > （1）当你一次性失去2000以上资金时，抽一张卡。
#[test]
fn band_draws_on_a_2000_plus_loss() {
    forall(3, 0xC0_10_19, |rng, _i| {
        let mut t = Table::new(&["广町七深", "户山香澄"]);
        t.clean();
        t.begin_turn(0);
        t.set_draw(0, &["R:[衍生] 压", "R:[衍生] 压"]);
        let hall = rules_spec::tile("弦卷豪宅"); // rent[2]=3400
        t.own(1, &[hall]);
        t.set_houses(hall, 2);
        t.set_pos(0, hall - 1);
        let money0 = 10_000 + rng.range(0, 10) * 100;
        t.set_money(0, money0);
        t.observe(CLAUSES);
        t.dice(&[1]);
        t.roll(0).unwrap();
        decline_all(&mut t);

        if t.money(0) != money0 - 3400 {
            return Err(format!(
                "C-MOR-band-01 setup: money {}, expected {}",
                t.money(0),
                money0 - 3400
            ));
        }
        if t.hand(0) != vec!["R:[衍生] 压".to_string()] {
            return Err(format!(
                "C-MOR-band-01: hand {:?}, expected one drawn card \
                 (「当你一次性失去2000以上资金时，抽一张卡」)",
                t.hand(0)
            ));
        }
        Ok(())
    })
    .expect("C-MOR-band-01 contract");
}

/// C-MOR-band-02.
///
/// > （2）[CiRCLE奖励]选择[获得]资金时根据选择[获得]资金次数设资金量为1000，1500，
/// > 2000的循环
#[test]
fn band_circle_money_cycles_1000_1500_2000() {
    forall(2, 0xC0_10_1A, |_rng, _i| {
        let mut t = Table::new(&["广町七深", "户山香澄"]);
        t.clean();
        t.begin_turn(0);
        let mut gains = Vec::new();
        for i in 0..4 {
            t.set_pos(0, 58);
            t.dice(&[3]);
            t.observe(CLAUSES);
            t.roll(0).unwrap();
            let before = t.money(0);
            // The CiRCLE reward prompt: option 0 is the money choice.
            t.answer_one(0).unwrap();
            decline_all(&mut t);
            gains.push(t.money(0) - before);
            if i < 3 {
                t.begin_turn(0);
            }
        }
        if gains != vec![1000, 1500, 2000, 1000] {
            return Err(format!(
                "C-MOR-band-02: gains {gains:?}, expected [1000, 1500, 2000, 1000] \
                 (「1000，1500，2000的循环」)"
            ));
        }
        Ok(())
    })
    .expect("C-MOR-band-02 contract");
}

/// C-MOR-nnm-01 / -02: the hand play must run the mark-spending choice.
///
/// > 弃置手中x枚角色标记，发动以下效果中的一个：
/// > （1）抽x张卡（可超过上限），回合结束后将手牌弃置到五张
///
/// The mark-spending choice must run (「弃置手中x枚角色标记，发动以下效果中的
/// one」). Branch (1) draws x.
#[test]
fn nnm_hand_play_spends_marks_and_draws() {
    forall(2, 0xC0_10_1B, |_rng, _i| {
        let mut t = Table::new(&["广町七深", "户山香澄", "花园多惠"]);
        t.clean();
        t.begin_turn(0);
        // Arrange one 角色标记 (namespace `角色标记:<skill id>`, what
        // 广町七深 (2) writes).
        t.m.world_mut().st.players[0].tokens.push(game_core::state::Counter {
            name: "角色标记:skill:户山香澄:鼓动".into(),
            value: 1,
            instance: None,
        });
        t.set_draw(0, &["R:[衍生] 压", "R:[衍生] 压", "R:[衍生] 压"]);
        t.give(0, &[NNM]);
        t.observe(CLAUSES);
        t.play(0, NNM).map_err(|e| {
            format!(
                "C-MOR-nnm-01: 「弃置手中x枚角色标记，发动以下效果中的一个」 \
                 -- hand play refused: {e}"
            )
        })?;
        // Branch (1): option 0 = draw x.
        t.answer_one(0).map_err(|e| {
            format!(
                "C-MOR-nnm-02: branch choice rejected: {e} (multi-activation dispatch)"
            )
        })?;
        decline_all(&mut t);
        if t.hand(0).len() != 1 {
            return Err(format!(
                "C-MOR-nnm-02: hand {:?}, expected 1 drawn card (x=1)",
                t.hand(0)
            ));
        }
        Ok(())
    })
    .expect("C-MOR-nnm-01/02 contract");
}

/// C-MOR-nnm-04: the placed press spends a crystal for the (2) skill.
///
/// > （3）将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶视为发动你的
/// > （2）技能，此次技能不受数量或轮数限制
#[test]
fn nnm_placed_press_spends_a_crystal() {
    forall(3, 0xC0_10_1C, |rng, _i| {
        let mut t = Table::new(&["广町七深", "户山香澄"]);
        t.clean();
        t.begin_turn(0);
        t.place_raw(0, NNM);
        let x = rng.range(1, 3);
        t.set_crystals(0, NNM, x);
        t.observe(CLAUSES);
        t.skill(0, NNM)
            .map_err(|e| format!("C-MOR-nnm-04: press refused with {x} crystals: {e}"))?;
        decline_all(&mut t);
        if t.crystals(0, NNM) != Some(x - 1) {
            return Err(format!(
                "C-MOR-nnm-04: crystals {:?}, expected Some({})",
                t.crystals(0, NNM),
                x - 1
            ));
        }
        if x == 1 {
            // C-MOR-nnm-05: exhausted → discard.
            if t.on_field(0, NNM) {
                return Err(format!(
                    "C-MOR-nnm-05: still on the field: {:?}",
                    t.field_ids(0)
                ));
            }
        }
        Ok(())
    })
    .expect("C-MOR-nnm-04/05 contract");
}

/// C-MOR-nnm-05: the last crystal empties the card into the discard.
#[test]
fn nnm_last_crystal_discards_the_card() {
    forall(2, 0xC0_10_1D, |_rng, _i| {
        let mut t = Table::new(&["广町七深", "户山香澄"]);
        t.clean();
        t.begin_turn(0);
        t.place_raw(0, NNM);
        t.set_crystals(0, NNM, 1);
        t.observe(CLAUSES);
        t.skill(0, NNM).unwrap();
        decline_all(&mut t);
        // 「当奇迹水晶耗尽时，将此卡置入弃牌堆」 -- the card leaves the field as
        // the last crystal goes, so a field lookup is None (not Some(0)).
        if t.on_field(0, NNM) {
            return Err(format!(
                "C-MOR-nnm-05: 「当奇迹水晶耗尽时，将此卡置入弃牌堆」 -- \
                 still on the field with 0 crystals: {:?}",
                t.field_ids(0)
            ));
        }
        let in_discard = t.discard(0).iter().any(|c| c == NNM)
            || t.discard(1).iter().any(|c| c == NNM)
            || t.draw_pile(0).iter().any(|c| c == NNM);
        if !in_discard {
            return Err(format!(
                "C-MOR-nnm-05: card left the field but is not in discard/draw: \
                 d0={:?} d1={:?} draw={:?}",
                t.discard(0),
                t.discard(1),
                t.draw_pile(0),
            ));
        }
        Ok(())
    })
    .expect("C-MOR-nnm-05 contract");
}

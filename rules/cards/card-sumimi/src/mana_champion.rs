//! `Sumimi:（真奈）歌唱大赛5连冠` -- C# `CardManaChampion` (MatchHost.cs:11529-11591):
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:（真奈）歌唱大赛5连冠`）:
//! > （真奈）歌唱大赛5连冠：
//! > [反击]当你或你的格子即将受到来自你以外的效果影响时打出此卡，此时场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡，且其反击卡中针对打出玩家自身的效果改为你。若以此种方式使你免于受到该影响，打出那张[反击]卡的玩家可抽一张卡。若没有人在此卡的效果期间打出[反击]卡，你抽一张卡。
//!
//! [反击] that lets the other players counteract as if they were the target.
//!
//! CH-05 join: the `SELF_REDIRECT` property marks this counter as opening a
//! join -- self-targeted effects of later counters answering the same link are
//! redirected to this card's user (「改为你」). The joiner's own body takes its
//! draw when the shield lands; this body takes the fallback draw when nobody
//! shielded.

use card_sdk::abi::{prop, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const MANA_CHAMPION: CardDef = CardDef::new(
    "Sumimi:（真奈）歌唱大赛5连冠",
    &[On::Counteract(
        &[ChainKind::Effect],
        "by != owner && by >= 0 && effect.hits(owner)",
        None,
        counteract,
    )],
)
.props(&[(prop::SELF_REDIRECT, 1)])
.legacy(&[(0, legacy_can_counteract)]);

/// G4 audit oracle (docs/GUARDS.md §5.1).
fn legacy_can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「当你或你的格子即将受到来自你以外的效果影响时打出此卡」
    // C# `H.HitByOtherCard(t, seat)` (MatchHost.cs:19235-19255) =
    // `t.ByCard >= 0 && t.ByCard != seat` and (kind "target"/"abnormal" ->
    // `t.Target == seat`, kind "pay" -> `t.Pay.from == seat`).
    // Re-verified against the v26 pipeline: a tile hit lands as
    // `t.Target = owner` (H.TargetTile, MatchHost.cs:19178-19216), so
    // 「或你的格子」 is covered by `target() == player_id`.
    if !trigger::by_card().is_some_and(|by| by != player_id) {
        return false;
    }
    // 规则书[反击]: 「被其他人的卡的效果影响」 is one condition on the *effect*,
    // and now reads as one: any effect another player's card declared at me. It
    // used to be reconstructed by unioning `Target`/`Abnormal` (via `t.Target`)
    // with `Pay` (via `t.Pay.from`) and matching on two different fields.
    // `effect::hits` covers both because a payment touches its payer *and* its
    // payee.
    ctx::effect::hits(player_id)
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「若没有人在此卡的效果期间打出[反击]卡，你抽一张卡。」
    // C# `CardManaChampion.Counteract` (MatchHost.cs:11543-11590) walks `H.Others(i)`
    // offering each a nested `H.Counteract(copy, p)` with `Target = p`; `anyone` is
    // set only when a nested counteraction lands, and `if (!anyone)` draws 1.
    ctx::log(
        player_id,
        &Msg::new(key!("mana_champion_log")).player_id("who", player_id),
    );
    // CH-05 join: the `SELF_REDIRECT` property (set at declaration) retargets
    // self-targeted effects of later counters answering this link to `player_id`
    // (「改为你」). A joiner that lands the shield draws itself (see
    // `net_error.rs`); we take the fallback draw only when nobody shielded.
    //
    // TODO(规则书)[judgement](ABI): the full nested PlayCtx (a per-seat
    //   「如同自身的对应目标被指定一般」 window with its own chain) is still
    //   open -- see CH-05. The join is currently the outer ring after this
    //   counter's declaration, which covers the g19 shape. The 「若以此种方式
    //   使你免于受到该影响」 half (joiner draw) lives in the joiner's body.
    if ctx::designation_cancelled(player_id) {
        // A joiner shielded us; the joiner draws itself. No fallback.
        // TODO(规则书): the joiner's draw is 真奈's clause, not the joiner's
        //   own effect -- once the nested window exists, move the draw here.
        return Ok(());
    }
    // 规则书[反击]: 「若没有人在此卡的效果期间打出[反击]卡，你抽一张卡。」 --
    // C# `H.DrawR(i, 1, ...)` on `!anyone` (MatchHost.cs:11586-11589).
    // TODO(规则书): this also fires when someone joined but did not shield
    //   (the rulebook's fallback is 「没有人…打出[反击]卡」); needs a
    //   joiner-count once the nested window reports one.
    ctx::draw(player_id, 1)?;
    Ok(())
}

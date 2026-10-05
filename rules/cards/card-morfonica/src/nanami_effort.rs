//! `Mor:（NNM）稍微努力了一下` -- C# `CardNanamiEffort` (MatchHost.cs:4805-4899): spend
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（NNM）稍微努力了一下`）:
//! > （NNM）稍微努力了一下：
//! > 弃置手中x枚角色标记，发动以下效果中的一个：
//! >
//! > （1）抽x张卡（可超过上限），回合结束后将手牌弃置到五张
//! >
//! > （2）获得x*2000资金
//! >
//! > （3）将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶视为发动你的
//! > （2）技能，此次技能不受数量或轮数限制
//! >
//! > （4）当奇迹水晶耗尽时，将此卡置入弃牌堆
//!
//! x character tokens for one of three effects.

use card_sdk::ctx::{self, CardPile};
use card_sdk::{key, CardDef, On, Msg};

pub const NANAMI_EFFORT: CardDef = CardDef::new("Mor:（NNM）稍微努力了一下", &[
    On::Play(nanami_effort),
    On::AtEnd(discard_down_to_five),
]);

fn nanami_effort(player_id: i32) {
    // 规则书: 「弃置手中x枚角色标记，发动以下效果中的一个」
    // C# `Tokens` walks `players[player_id].tokens` for `name.StartsWith("角色标记:") &&
    //   value > 0` (`MatchHost.cs:4805-4810`), `H.AskNumber` picks x in `1..total`,
    //   then one `H.AskPick` per spent token and a second `H.AskPick` over the
    //   three effects.
    // TODO(ABI): 「弃置手中x枚角色标记」 -- needs a token-list query so the player's
    //   `角色标记:*` counters can be counted (`CardNanamiEffort.Tokens`) and spent
    //   (`H.AddTok(i, name, -1)`). `ctx::ask_number` (C# `H.AskNumber`) exists now,
    //   but without the token list there is no `total` to size x against, so the
    //   play still folds.
    // C# `WhyNot` also refuses the play with no character tokens
    // ("你没有角色标记") -- blocked on the same token-list query.
    ctx::log(player_id, &Msg::new(key!("nanami_effort_no_tok")).player_id("who", player_id));
    // Effect (1) body is `effect_draw` below -- the [AtEnd] scheduling half is
    // live there (`ctx::before_turn_end`); the draw half is still TODO'd.
    // Effect (2): `ctx::gain(player_id, x * 2000, ...)`.
    // TODO(规则书)（2）: 「获得x*2000资金」 -- `ctx::gain(player_id, x * 2000, ...)`, blocked
    //   on x.
    // Effect (3): `ctx::place_card` + `ctx::add_crystals(player_id, x, 0)`.
    // TODO(规则书)（3）: 「你可以移除一个奇迹水晶视为发动你的（2）技能，此次技能不受
    //   数量或轮数限制」 -- the crystal counter is `ctx::add_crystals` now, but
    //   spending one for the owner's skill (2) needs the skill hook
    //   (`ISkillHook.SkillAnnounced` / `H.AnnounceSkill`); x is blocked on the
    //   token-list query.
    // Effect (4): when the crystals hit 0 -- `ctx::add_crystals` returns 0 ->
    //   `ctx::unplace_card` + `ctx::to_discard` (C# `CardNanamiEffort.Take`).
    // TODO(规则书)（4）: 「当奇迹水晶耗尽时，将此卡置入弃牌堆」 -- the empty branch is
    //   `unplace_card` + `to_discard`, but the crystal-spending path is the skill
    //   hook above (held), so nothing can drain the crystals yet.
}

/// Effect (1) 「抽x张卡（可超过上限），回合结束后将手牌弃置到五张」 -- C# case 0 of
/// the play's `H.AskPick` (`MatchHost.cs:4874-4878`). The discard-down half is
/// C# `H._turnCtx.AtEnd.Add(() => H.DiscardDownTo(i, 5, CardName))` =
/// `ctx::before_turn_end` (C# `AtEnd`, pre-wear-off) queuing the `On::AtEnd`
/// body below. Unreachable until the token-list query lands (the play folds
/// first); written now so the queued scheduling is already the right hook.
#[allow(dead_code)]
fn effect_draw(player_id: i32, _x: i32) {
    // TODO(规则书)（1）: 「抽x张卡（可超过上限）」 -- the draw itself is
    //   `ctx::draw(player_id, x)`, but x is blocked on the token-list query in the
    //   play and 「可超过上限」 needs the NoLimit hand-limit attachment (C#
    //   `NoLimitFx.NoHandLimit`).
    // 规则书（1）: 「回合结束后将手牌弃置到五张」 -- C#
    // `H._turnCtx.AtEnd.Add(() => H.DiscardDownTo(i, 5, CardName))`.
    ctx::before_turn_end(player_id);
}

/// 规则书（1）: 「回合结束后将手牌弃置到五张」 -- C# `H._turnCtx.AtEnd.Add(() =>
/// H.DiscardDownTo(i, 5, CardName))`. Scheduled by the play's effect (1) via
/// `ctx::before_turn_end(player_id)`.
fn discard_down_to_five(player_id: i32) {
    let hand = ctx::cards_in(player_id, CardPile::Hand);
    let mut extra = hand.len() as i32 - 5;
    for card in &hand {
        if extra <= 0 {
            break;
        }
        if ctx::discard_from_hand(player_id, card) {
            extra -= 1;
        }
    }
    if extra < 0 {
        return;
    }
    ctx::log(player_id, &Msg::new(key!("nanami_effort_discard_down")).player_id("who", player_id));
}

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

use card_sdk::{ctx, key, CardDef, Msg};

pub const NANAMI_EFFORT: CardDef = CardDef {
    id: "Mor:（NNM）稍微努力了一下",
    play: Some(nanami_effort),
    can_react: None,
    react: None,
    why_not: None,
};

fn nanami_effort(seat: i32) {
    // 规则书: 「弃置手中x枚角色标记，发动以下效果中的一个」
    // C# `Tokens` walks `seats[seat].tokens` for `name.StartsWith("角色标记:") &&
    //   value > 0` (`MatchHost.cs:4805-4810`), `H.AskNumber` picks x in `1..total`,
    //   then one `H.AskPick` per spent token and a second `H.AskPick` over the
    //   three effects.
    // TODO(ABI): 「弃置手中x枚角色标记」 -- needs a token-list query so the seat's
    //   `角色标记:*` counters can be counted (`CardNanamiEffort.Tokens`) and spent
    //   (`H.AddTok(i, name, -1)`). `ctx::ask_number` (C# `H.AskNumber`) exists now,
    //   but without the token list there is no `total` to size x against, so the
    //   play still folds.
    // C# `WhyNot` also refuses the play with no character tokens
    // ("你没有角色标记") -- blocked on the same token-list query.
    ctx::log(seat, &Msg::new(key!("nanami_effort_no_tok")).seat("who", seat));
    // TODO(规则书)（1）: 「抽x张卡（可超过上限），回合结束后将手牌弃置到五张」 -- needs
    //   the NoLimit hand-limit attachment (C# `NoLimitFx.NoHandLimit`), `H.DrawR`,
    //   and the turn-end `H.DiscardDownTo(i, 5, ...)`.
    // TODO(规则书)（2）: 「获得x*2000资金」 -- `ctx::gain(seat, x * 2000, ...)`, blocked
    //   on x.
    // TODO(规则书)（3）: 「将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶视为
    //   发动你的（2）技能，此次技能不受数量或轮数限制」 -- needs the field-card crystal
    //   counter (`H.PlaceFromPlay(c, -1, -1, x)`) and the skill hook
    //   (`ISkillHook.SkillAnnounced` / `H.AnnounceSkill`) to spend a crystal for the
    //   owner's skill (2) outside its usual limits.
    // TODO(规则书)（4）: 「当奇迹水晶耗尽时，将此卡置入弃牌堆」 -- needs the crystal
    //   counter plus `H.Unplace(this, "discard", "奇迹水晶用完了")` on empty
    //   (C# `CardNanamiEffort.Take`).
}
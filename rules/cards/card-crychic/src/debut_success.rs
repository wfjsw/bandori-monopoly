//! `CRYCHIC:初演大成功` -- C# `CardDebutSuccess` (MatchHost.cs:3146-3163): this
//! turn's money never drops, then stun + a band crystal after the turn.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:初演大成功`）:
//! > 初演大成功：
//! >   打出此卡后，本回合内你的资金不会下降（除拍卖与写明不受资金变动效果影响的情况外），回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const DEBUT_SUCCESS: CardDef = CardDef {
    id: "CRYCHIC:初演大成功",
    play: Some(debut_success),
    can_react: None,
    react: None,
    why_not: None,
};

fn debut_success(seat: i32) {
    // 规则书: 「打出此卡后，本回合内你的资金不会下降」 -- C# `H._turnCtx.NoMoneyLoss = true`.
    ctx::log(seat, &Msg::new(key!("debut_success_note")).seat("who", seat));
    // TODO(ABI): 「本回合内你的资金不会下降（除拍卖与写明不受资金变动效果影响的情况外）」
    //   -- needs the turn-scoped NoMoneyLoss flag (C# `H._turnCtx.NoMoneyLoss`,
    //   honoured by `H.Money` except for auctions and effects marked immune to
    //   money deltas). A `set_slot(seat, "debut_no_loss", 1)` stand-in has no
    //   consumer until that hook exists.
    // TODO(ABI): 「回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶」
    //   -- needs the turn-end AfterEnd scheduler (C# `H._turnCtx.AfterEnd.Add(...)`
    //   -> `CardDebutSuccess.After`). Once scheduled this is
    //   `ctx::give_stun(seat, 1)` + `ctx::add_band_crystals(seat, 1, 0)`; applying
    //   them at play time would stun the seat for the rest of this turn, which is
    //   not the rule.
}
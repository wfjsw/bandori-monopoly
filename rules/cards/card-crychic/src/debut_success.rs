//! `CRYCHIC:初演大成功` -- C# `CardDebutSuccess` (MatchHost.cs:3146-3163): this
//! turn's money never drops, then stun + a band crystal after the turn.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:初演大成功`）:
//! > 初演大成功：
//! >   打出此卡后，本回合内你的资金不会下降（除拍卖与写明不受资金变动效果影响的情况外），回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶。
//!

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const DEBUT_SUCCESS: CardDef = CardDef::new("CRYCHIC:初演大成功", &[
    On::Play(None, debut_success),
    On::AtEnd(at_end)]);

fn debut_success(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「打出此卡后，本回合内你的资金不会下降」 -- C# `H._turnCtx.NoMoneyLoss = true`.
    ctx::log(player_id, &Msg::new(key!("debut_success_note")).player_id("who", player_id));
    ctx::set_no_money_loss(player_id);
    // 规则书: 「回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶」
    //   -- C# `H._turnCtx.AfterEnd.Add(() => After(i))`.
    ctx::at_turn_end(player_id);
    Ok(())
}

/// C# `CardDebutSuccess.After` -- the scheduled turn-end body.
fn at_end(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶」
    ctx::give_stun(player_id, 1);
    ctx::add_band_crystals(player_id, 1, 0);
    Ok(())
}

//! `PP:[若宫伊芙]属于我的武士道！` -- C# `CardEveBushido`: stay in play, roll 12d4
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[若宫伊芙]属于我的武士道！`）:
//! > [若宫伊芙]属于我的武士道！：
//! > [手]：
//! > 将此卡放置在[使用者]场上并投掷12d4，获得投掷结果*60的资金。
//! > [持续]：
//! >
//! > （1）[使用者]抽卡后为此卡添加1个[奇迹水晶]（上限3）。
//! >
//! > （2）[结算]前如果此卡上有[奇迹水晶]且[拥有者]所在格子的地契主人为其他玩家则使用1个[奇迹水晶]并依次进行以下操作：
//! > 1. 地契主人投掷1d6；
//! > 2. [拥有者]投掷3d4；
//! > 3. 投掷点数低的玩家[支付]投掷点数高的玩家[拥有者]所在格子的房屋数量加1×100（如果平局则双方互不支付），如果[共鸣]则[支付]金额改为“投掷点差”×50。
//!
//! for money. The duel runs in the `SettleBefore` hook; the crystal growth
//! runs on the `Drew` hook.

use card_sdk::abi::{TriggerKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const EVE_BUSHIDO: CardDef = CardDef::new("PP:[若宫伊芙]属于我的武士道！", &[
    On::Play(None, eve_bushido),
    On::Hook(&[HookKind::Drew], drew_guard, drew),
    On::Hook(&[HookKind::SettleBefore], |_| true, settle_before)]);

fn eve_bushido(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]场上」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "PP:[若宫伊芙]属于我的武士道！", &Msg::new(key!("eve_bushido_note")));
    // 规则书[手]: 「并投掷12d4，获得投掷结果*60的资金」
    // `ctx::roll` honours a forced extreme (「以理论最大值或最小值结算」).
    let n = ctx::roll(player_id, 12, 4);
    ctx::gain(player_id, n * 60, &Msg::new(key!("eve_bushido_why")).i("n", n as i64));
    Ok(())
}

/// C# `CardEveBushido.Drew` -- once per draw batch, add a crystal (cap 3).
/// 规则书[持续]（1）: 「[使用者]抽卡后为此卡添加1个[奇迹水晶]（上限3）」
/// Pure guard for [`drew`] -- the activation gate. `false`
/// means the card is not activated at all.
fn drew_guard(player_id: i32) -> bool {
    ctx::is_placed() && trigger::player_id() == player_id
}

fn drew(player_id: i32) -> card_sdk::Asked {
    if trigger::value() <= 0 {
        return Ok(());
    }
    ctx::add_crystals(1, 3);
    Ok(())
}

/// C# `CardEveBushido.SettleBefore` -- the owner is settling on someone else's
/// deed and this card still holds a miracle crystal: duel the landlord.
fn settle_before(player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::SettleBefore || !ctx::is_placed() {
        return Ok(());
    }
    if trigger::player_id() != player_id || ctx::crystals() <= 0 {
        return Ok(());
    }
    let at = trigger::tile();
    if at < 0 {
        return Ok(());
    }
    let owner = ctx::tile_owner(at);
    if owner < 0 || owner == player_id || ctx::player_out(owner) {
        return Ok(());
    }
    // 规则书[持续]（2）: 「使用1个[奇迹水晶]」
    ctx::add_crystals(-1, 0);
    // 规则书[持续]（2）1: 「地契主人投掷1d6」
    let a = ctx::roll(owner, 1, 6);
    // 规则书[持续]（2）2: 「[拥有者]投掷3d4」
    let b = ctx::roll(player_id, 3, 4);
    // 规则书[持续]（2）3: 「如果平局则双方互不支付」
    if a == b {
        ctx::log(player_id, &Msg::new(key!("eve_bushido_tie")));
        return Ok(());
    }
    // 规则书[持续]（2）3: 「[支付]……房屋数量加1×100」
    let mut amount = (ctx::houses_of(at) + 1) * 100;
    // 规则书[持续]（2）3: 「如果[共鸣]则[支付]金额改为“投掷点差”×50」 -- 「改为」 is
    // unconditional; the C# only swapped when it was the better deal, which the
    // clause does not say.
    if crate::resonance::try_resonance(player_id)? {
        amount = (a - b).abs() * 50;
    }
    let (from, to) = if a < b { (owner, player_id) } else { (player_id, owner) };
    ctx::transfer(from, to, amount, &Msg::new(key!("eve_bushido_why")).i("n", amount as i64))?;
    Ok(())
}
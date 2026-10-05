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

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const EVE_BUSHIDO: CardDef = CardDef::new("PP:[若宫伊芙]属于我的武士道！", &[
    On::Play(eve_bushido),
    On::Hook(&[TriggerKind::Drew], drew),
    On::Hook(&[TriggerKind::SettleBefore], settle_before),
]);

fn eve_bushido(player_id: i32) {
    // 规则书[手]: 「将此卡放置在[使用者]场上」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "PP:[若宫伊芙]属于我的武士道！", &Msg::new(key!("eve_bushido_note")));
    // 规则书[手]: 「并投掷12d4，获得投掷结果*60的资金」
    // TODO: C# uses H.CardRoll, which honours PlayCtx.Extreme (forced max/min dice).
    let n = ctx::roll(player_id, 12, 4);
    ctx::gain(player_id, n * 60, &Msg::new(key!("eve_bushido_why")).i("n", n as i64));
}

/// C# `CardEveBushido.Drew` -- once per draw batch, add a crystal (cap 3).
/// 规则书[持续]（1）: 「[使用者]抽卡后为此卡添加1个[奇迹水晶]（上限3）」
fn drew(player_id: i32) {
    if !ctx::is_placed(player_id) || trigger::player_id() != player_id {
        return;
    }
    if trigger::value() <= 0 {
        return;
    }
    ctx::add_crystals(player_id, 1, 3);
}

/// C# `CardEveBushido.SettleBefore` -- the owner is settling on someone else's
/// deed and this card still holds a miracle crystal: duel the landlord.
fn settle_before(player_id: i32) {
    if trigger::kind() != TriggerKind::SettleBefore || !ctx::is_placed(player_id) {
        return;
    }
    if trigger::player_id() != player_id || ctx::crystals(player_id) <= 0 {
        return;
    }
    let at = trigger::tile();
    if at < 0 {
        return;
    }
    let owner = ctx::tile_owner(at);
    if owner < 0 || owner == player_id || ctx::player_out(owner) {
        return;
    }
    // 规则书[持续]（2）: 「使用1个[奇迹水晶]」
    ctx::add_crystals(player_id, -1, 0);
    // 规则书[持续]（2）1: 「地契主人投掷1d6」
    let a = ctx::roll(owner, 1, 6);
    // 规则书[持续]（2）2: 「[拥有者]投掷3d4」
    let b = ctx::roll(player_id, 3, 4);
    // 规则书[持续]（2）3: 「如果平局则双方互不支付」
    if a == b {
        ctx::log(player_id, &Msg::new(key!("eve_bushido_tie")));
        return;
    }
    // 规则书[持续]（2）3: 「[支付]……房屋数量加1×100」
    let amount = (ctx::houses_of(at) + 1) * 100;
    // TODO(规则书): [持续]（2）3: 「如果[共鸣]则[支付]金额改为“投掷点差”×50」 -- needs
    // H.TryResonance (discard 「PP:[衍生]共鸣」 from hand) to swap the amount to
    // `abs(a - b) * 50` when that is the better deal.
    let (from, to) = if a < b { (owner, player_id) } else { (player_id, owner) };
    ctx::transfer(from, to, amount, &Msg::new(key!("eve_bushido_why")).i("n", amount as i64));
}
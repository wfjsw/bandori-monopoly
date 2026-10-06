//! `Mor:勇气展翅高飞之时` -- C# `CardCourageWings` (MatchHost.cs:4779-4803).
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:勇气展翅高飞之时`）:
//! > 勇气展翅高飞之时： 
//! >  掷骰3d20，结果对应序号格子的所有者向你支付该地块的购买价格+地块已有房子的建造价格总额的一半，若为地产商地块，获得1000资金
//!

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const COURAGE_WINGS: CardDef =
    CardDef::new("Mor:勇气展翅高飞之时", &[On::Play(None, courage_wings)]);

fn courage_wings(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「掷骰3d20，结果对应序号格子」 -- `ctx::roll` honours a forced
    // extreme (「以理论最大值或最小值结算」) when one is armed.
    let roll = ctx::roll(player_id, 3, 20);
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    let tile = (roll - 1) % n;
    // 规则书: 「若为地产商地块，获得1000资金」
    if ctx::is_agent(tile) {
        ctx::gain(player_id, 1000, &Msg::new(key!("courage_wings_agent")));
        return Ok(());
    }
    // 规则书: 「所有者向你支付该地块的购买价格+地块已有房子的建造价格总额的一半」
    let owner = ctx::tile_owner(tile);
    if owner < 0 || owner == player_id || ctx::player_out(owner) {
        // C#: 掷到了 …：没有别的主人，没有效果
        ctx::log(
            player_id,
            &Msg::new(key!("courage_wings_none")).tile("tile", tile),
        );
        return Ok(());
    }
    // 「购买价格+地块已有房子的建造价格总额」 is `buy_price` (land + houses);
    // 「…的一半」 is the C# `(price + houses * house) / 2`.
    let amount = ctx::buy_price(tile) / 2;
    ctx::transfer(
        owner,
        player_id,
        amount,
        &Msg::new(key!("courage_wings_why")),
    )?;
    Ok(())
}

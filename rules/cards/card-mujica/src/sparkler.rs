//! `Mujica:燃尽前的线香花火` -- C# `CardSparkler` (MatchHost.cs:5542-5580): place
//! with 2 miracle crystals; each turn end burns one for an extra turn.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:燃尽前的线香花火`）:
//! > 燃尽前的线香花火：
//! > 将此卡放置于场上并放置2个奇迹水晶（上限2），你的回合结束后自动移除一个奇迹水晶并使你获得一个额外回合，最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]。
//!

use card_sdk::abi::{TriggerKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "Mujica:燃尽前的线香花火";

pub const SPARKLER: CardDef = CardDef::new("Mujica:燃尽前的线香花火", &[
    On::Play(sparkler),
    // C# `CardSparkler.TurnEndAfter` -> `Burn` -- a field hook on the card's own
    // turn end while it is in play (ABI v23 `TurnEndAfter`: after `TurnEnd`,
    // matching the C# `Fx.TurnEndAfter` dispatch).
    On::Hook(&[HookKind::TurnEndAfter], turn_end),
]);

fn sparkler(player_id: i32) {
    // 规则书: 「将此卡放置于场上并放置2个奇迹水晶（上限2）」
    // C# `H.PlaceFromPlay(c, -1, -1, 2)` loads the card with 2 crystals
    // (`MaxCrystals = 2`).
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("sparkler_note")));
    ctx::add_crystals(player_id, 2, 2);
    ctx::log(player_id, &Msg::new(key!("sparkler_placed")).player_id("who", player_id));
}

/// C# `CardSparkler.TurnEndAfter` -> `Burn` (MatchHost.cs:5560-5578).
/// Runs through the Fx hook dispatch at `turnEndAfter` (ABI v23
/// `TriggerKind::TurnEndAfter`), so this is a field effect, not a [反击].
/// `turn` is the player whose turn ended -- only the card's own turn end counts
/// (`turn != Player` -> skip).
fn turn_end(player_id: i32) {
    if trigger::player_id() != player_id || !ctx::is_placed(player_id) {
        return;
    }
    // 规则书: 「你的回合结束后自动移除一个奇迹水晶并使你获得一个额外回合」
    // C# `AddCrystals(-1, "回合结束")` then `H.GiveExtraTurn(Seat, CardName)`.
    ctx::add_crystals(player_id, -1, 0);
    ctx::give_extra_turn(player_id);
    ctx::log(player_id, &Msg::new(key!("sparkler_burned")).player_id("who", player_id));
    // 规则书: 「最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]」
    // C# `if (Crystals <= 0) { H.Unplace(this, "discard", "奇迹水晶用完了");
    // H.GiveStun(Player, 2, ...); }`.
    if ctx::crystals(player_id) <= 0 {
        ctx::unplace_card(player_id);
        ctx::to_discard(player_id, ID);
        ctx::give_stun(player_id, 2);
        ctx::log(player_id, &Msg::new(key!("sparkler_out")).player_id("who", player_id));
    }
}

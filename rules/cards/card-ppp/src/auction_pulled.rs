//! `PPP:[衍生]拍卖撤下来了` -- C# `CardAuctionPulled` (MatchHost.cs:8971-9017):
//! auto-placed when drawn; eats a placed 仓库里的Random Star each turn start and
//! raises the owner's fire-pot cap.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:[衍生]拍卖撤下来了`）:
//! > [衍生]拍卖撤下来了：
//! > [特]：
//! > 此卡加入手牌时将此卡放置在自身场上，每回合开始时将自身场上的“仓库里的Random Star”[移除]（如果有，然后失去540资金）
//! > [持续]：此卡拥有者火罐上限加1且不受任何其他效果影响。
//!
//! A pure [特] card (C# has no `Play`, `Normal => false`, `Immune => true`).

use card_sdk::abi::{TriggerKind, HookKind, CardPile};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "PPP:[衍生]拍卖撤下来了";
const STAR: &str = "PPP:仓库里的Random Star";

pub const AUCTION_PULLED: CardDef = CardDef::new(ID, &[
    On::Hook(&[HookKind::Drawn, HookKind::TurnStart], hook),
]);

/// `Fx.Drawn` (C# `CardAuctionPulled.Drawn`) -- pull the card out of the hand
/// and place it on the owner's field the moment it arrives.
/// `Fx.TurnStart` (C# `CardAuctionPulled.TurnStart` -> `Remove`) -- eat the
/// placed 仓库里的Random Star and charge 540.
fn hook(player_id: i32) {
    match trigger::kind() {
        TriggerKind::Drawn => {
            if !trigger::card_is(ID) {
                return;
            }
            if !ctx::take_from_hand(player_id, ID) {
                return;
            }
            ctx::set_dest(ctx::Dest::Field);
            ctx::place_card(player_id, ID, &Msg::new(key!("auction_pulled_note")));
            // 规则书[持续]: 「此卡拥有者火罐上限加1」 -- C# `FireMaxDelta() => 1`.
            ctx::add_fire_max(player_id, 1);
            ctx::log(player_id, &Msg::new(key!("auction_pulled_placed")).player_id("who", player_id));
            // TODO(ABI)[持续]: 「且不受任何其他效果影响」 -- needs the card `Immune`
            //   flag (C# `CardAuctionPulled.Immune => true`) so other effects skip
            //   this card.
        }
        TriggerKind::TurnStart => {
            // C# `TurnStart(int turn)`: `if (turn != Player) return null;` -- only
            // the owner's own turn start.
            if trigger::player_id() != player_id || !ctx::is_placed(player_id) {
                return;
            }
            // 规则书[特]: 「每回合开始时将自身场上的“仓库里的Random Star”[移除]
            //   （如果有，然后失去540资金）」
            let field = ctx::cards_in(player_id, CardPile::Field);
            if !field.iter().any(|c| c == STAR) {
                return;
            }
            // C# `H.Unplace(star, "removed", ...)` -- off the field, out of the
            // game. `take_card(Field, ..)` drops it without sending it anywhere.
            if ctx::take_card(player_id, CardPile::Field, STAR) {
                ctx::log(player_id, &Msg::new(key!("auction_pulled_removed")).player_id("who", player_id));
                ctx::pay(player_id, 540, &Msg::new(key!("auction_pulled_paid")));
            }
        }
        _ => {}
    }
}

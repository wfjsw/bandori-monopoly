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
//! A pure [特] card (C# has no `Play`, `Normal => false`, `Immune => true`):
//! every clause is a hook the ABI does not carry yet (TODO below).

use card_sdk::CardDef;

pub const AUCTION_PULLED: CardDef = CardDef {
    id: "PPP:[衍生]拍卖撤下来了",
    play: None,
    can_react: None,
    react: None,
    why_not: None,
};

// TODO(ABI)[特]: 「此卡加入手牌时将此卡放置在自身场上」 -- needs the Fx.Drawn hook
//   (C# `CardAuctionPulled.Drawn`: pull the id out of the hand and
//   `H.PlaceCard(seat, seat, Id, ..., "加入手牌时放到场上")`).
// TODO(ABI)[特]: 「每回合开始时将自身场上的“仓库里的Random Star”[移除]（如果有，然后
//   失去540资金）」 -- needs the Fx.TurnStart hook (C# `CardAuctionPulled.TurnStart`
//   -> `Remove`) plus unplacing *another* card (`H.Unplace(star, ...)`;
//   `ctx::unplace_card` only drops this card) and then
//   `ctx::pay(seat, 540, &...)`.
// TODO(ABI)[持续]: 「此卡拥有者火罐上限加1」 -- needs a fire-cap delta (C#
//   `Card.FireMaxDelta() => 1`). `ctx::fire_max` only reads the cap; there is no
//   `add_fire_max`.
// TODO(ABI)[持续]: 「且不受任何其他效果影响」 -- needs the card `Immune` flag
//   (C# `CardAuctionPulled.Immune => true`) so other effects skip this card.
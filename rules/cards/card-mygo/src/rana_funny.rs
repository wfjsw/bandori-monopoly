//! `MyGO:（乐奈）有趣的女人` -- C# `CardRanaFunny` (MatchHost.cs:6792-6854):
//! plant this card on the current tile; passers-by who do not settle there
//! grow a miracle crystal on it, and at 5+ the next foreign passer is trapped.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:（乐奈）有趣的女人`）:
//! > （乐奈）有趣的女人：
//! > 将此卡置于当前格子上，每当有人经过且未在其上[触发结算]时为其增加一个奇迹水晶，当奇迹水晶总数为5或以上时使下一个经过的你以外的玩家选择失去一个“抹茶芭菲”或强制停下并[触发结算]，如果强制停下则此卡洗入弃牌堆。 由此卡效果导致[触发结算]时需支付资金减半
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const RANA_FUNNY: CardDef = CardDef {
    id: "MyGO:（乐奈）有趣的女人",
    play: Some(rana_funny),
    can_react: None,
    react: None,
    why_not: None,
};

fn rana_funny(seat: i32) {
    // 规则书: 「将此卡置于当前格子上」 -- C# `H.PlaceFromPlay(c, c.Seat, pos)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "MyGO:（乐奈）有趣的女人", &Msg::new(key!("rana_funny_note")));
    ctx::log(seat, &Msg::new(key!("rana_funny_placed")).seat("who", seat));
    // TODO(规则书): 「将此卡置于当前格子上」 -- the placement is bound to the
    // seat's current tile (`Card.Tile`); `place_card` only attaches the card to
    // the seat's field, so needs field-card tile placement.
    // TODO(规则书): 「每当有人经过且未在其上[触发结算]时为其增加一个奇迹水晶」 -- needs
    // the Fx.PassTile hook (C# `CardRanaFunny.PassTile`) and a per-card crystal
    // counter (`AddCrystals(1, ...)`).
    // TODO(规则书): 「当奇迹水晶总数为5或以上时使下一个经过的你以外的玩家选择失去一个
    // “抹茶芭菲”或强制停下并[触发结算]，如果强制停下则此卡洗入弃牌堆。由此卡效果导致[触发结算]
    // 时需支付资金减半」 -- the trap is the same Fx.PassTile path (C#
    // `CardRanaFunny.Trap`): an `ask_yes` between `ctx::add_tok(who,
    // "抹茶芭菲", -1, ...)` and a forced stop (`H.AbnormalGate` / `m.Stopped`
    // / `m.Resolve = true` with `m.PayFactor *= 0.5`, then `H.Unplace(this,
    // "discard", ...)`). The parfait token ops (`tok` / `add_tok`) are in the
    // vocabulary once the pass hook can raise the prompt; the forced-stop
    // gate and the pay factor are not.
}
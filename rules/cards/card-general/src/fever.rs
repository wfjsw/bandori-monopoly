//! `通用:[衍生]FEVER!` -- C# `CardFever`: stays in play, money to the owner +X.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:[衍生]FEVER!`）:
//! > [衍生]FEVER!：
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]。
//! > [持续]：
//! >
//! > （1）[拥有者]被[支付]或[获得]资金时将金额额外提高X；X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）。
//! >
//! > （2）[拥有者]回合开始时将此卡放入[使用者]弃卡区。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

const ID: &str = "通用:[衍生]FEVER!";

pub const FEVER: CardDef = CardDef::new("通用:[衍生]FEVER!", &[
    On::Play(fever),
    // C# `CardFever.PayAdd` / `CardFever.TurnStart` -- field hooks, not [反击].
    On::Hook(&[TriggerKind::PayAdd, TriggerKind::TurnStart], react),
]);

fn fever(player_id: i32) {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("fever_note")));
    // C# `H.PlaceFromPlay(c).Mem["x"] = c.N(0, 600)` -- the printed 600 is the
    // card's number, read through `ctx::n` in `x_of` (the C# stores the played
    // value in `Mem["x"]`; equivalent while `PlayCtx.Doubled` is unported).
    // 规则书（1）[持续] runs in `react` at `payAdd`; 规则书（2）[持续] at `turnStart`.
}

/// C# `CardFever.X` -- 600 minus 200 per *other* card on the owner's field
/// (`H.PlacedOf(Seat).Count(p => p != this && !p.FaceDown)`), free to go below 0.
/// TODO(ABI): the face-down half of the C# filter (`!p.FaceDown`) is not
/// expressible -- `cards_in(Field)` lists face-down cards too.
fn x_of(player_id: i32) -> i32 {
    let others = (ctx::cards_in(player_id, ctx::CardPile::Field).len() as i32 - 1).max(0);
    // 规则书（1）: 「X为600」 -- C# `c.N(0, 600)`; the 200-per-card drop is the
    // C# `X` property body, not one of the card's declared numbers.
    ctx::n(0, 600) - 200 * others
}

fn react(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    match trigger::kind() {
        // 规则书（1）[持续]: 「[拥有者]被[支付]或[获得]资金时将金额额外提高X；X为600，
        // [拥有者]场上每拥有一张卡则X降低200（可小于0）」 -- C# `CardFever.PayAdd`
        // (`p.to == Player && p.amount > 0` -> `p.amount = Math.Max(0, p.amount + x)`).
        // `t.target` is the payee (a gain is the bank paying the owner) and
        // `t.value` the pending amount, rewritten with `set_pay_amount`.
        TriggerKind::PayAdd => {
            if trigger::target() != player_id || trigger::value() <= 0 {
                return;
            }
            let x = x_of(player_id);
            let amount = trigger::value();
            trigger::set_pay_amount((amount + x).max(0));
            // C# `p.Note("FEVER! " + ((x >= 0) ? "+" : "") + x)`.
            ctx::log(player_id, &Msg::new(key!("fever_pay")).i("x", x as i64));
        }
        // 规则书（2）[持续]: 「[拥有者]回合开始时将此卡放入[使用者]弃卡区」
        TriggerKind::TurnStart => {
            if trigger::player_id() != player_id {
                return;
            }
            ctx::unplace_card(player_id);
            ctx::to_discard(player_id, ID);
            ctx::log(player_id, &Msg::new(key!("fever_unplaced")).player_id("who", player_id));
        }
        _ => {}
    }
}

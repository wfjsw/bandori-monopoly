//! `AG:宣战布告` -- C# `CardDeclareWar` (MatchHost.cs:921-955):
//! [反击] when another player's card affects you, they pay you 500 and you draw.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:宣战布告`）:
//! > 宣战布告：
//! > [手]：
//! > [反击]当你或你拥有的格子被其他玩家的卡效果影响时：[指定]那名玩家。被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const DECLARE_WAR: CardDef = CardDef {
    id: "AG:宣战布告",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「当你或你拥有的格子被其他玩家的卡效果影响时」 -- C#
    // `H.HitByOtherCard(t, seat)`: `t.ByCard` is a different seat and the trigger
    // is `target` / `pay` / `abnormal` aimed at you.
    // TODO(ABI): `t.ByCard` (the seat that played the causing card) is not in the
    // trigger vocabulary. `trigger::seat()` is the mover/payer; it approximates
    // the card player only on `target` / `abnormal` (C# `t.Seat` is the card
    // player there). The `pay` branch therefore cannot tell a card-caused payment
    // from rent and is left out (conservative).
    // TODO(规则书): 「或你拥有的格子」 -- the C# `HitByOtherCard` only reacts to
    // effects aimed at the seat, not at tiles it owns.
    let by = trigger::seat();
    if by < 0 || by == seat {
        return false;
    }
    match trigger::kind() {
        // C# `target`/`abnormal`: `t.Target == seat`, `!H.Out(t.ByCard)`.
        TriggerKind::Target | TriggerKind::Abnormal => {
            trigger::target() == seat && !ctx::seat_out(by)
        }
        _ => false,
    }
}

fn react(seat: i32) {
    let by = trigger::seat();
    if by < 0 || by == seat {
        return;
    }
    // 规则书[反击]: 「[指定]那名玩家」 -- the designation is the card's player.
    // TODO(ABI): needs the `H.Target` targeting gate (`CardDef.Targeting`) so
    // counters like a designation-intercept can answer (C# `H.Target(c, byCard, r)`).
    // 规则书[反击]: 「被[指定]的玩家[支付][使用者]500资金」
    let why = Msg::new(key!("declare_war_why")).card("card", "AG:宣战布告");
    ctx::transfer(by, seat, 500, &why);
    // 规则书[反击]: 「且[使用者]抽1张卡」
    ctx::draw(seat, 1);
}
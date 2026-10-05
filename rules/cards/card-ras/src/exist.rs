//! `RAS:EXIST` -- C# `CardExist` (MatchHost.cs:9321-9366).
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:EXIST`）:
//! > EXIST：
//! > 将此卡放置于自己场上，直到自己的下一回合开始，场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你，你的下回合开始时将其翻入弃牌堆，若在此期间此卡没有造成影响，抽1张卡
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const EXIST: CardDef = CardDef::new("RAS:EXIST", &[
    On::Play(exist),
    On::Hook(&[TriggerKind::TurnStart], turn_start),
]);

const ID: &str = "RAS:EXIST";

/// C# `Mem["used"]` -- set by the redirect when this card retargeted a card.
const SLOT_USED: &str = "exist_used";

fn exist(player_id: i32) {
    // 规则书: 「将此卡放置于自己场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("exist_note")));
    ctx::set_slot(player_id, SLOT_USED, 0);
    ctx::log(player_id, &Msg::new(key!("exist_placed")).player_id("who", player_id));
    // TODO(规则书): 「直到自己的下一回合开始，场上及打出的所有对单一玩家生效的手卡
    // （包括其他玩家指向自身的卡）的目标将改为你」 -- needs the Fx.Redirect /
    // IRedirect hook (C# `CardExist.Redirects`) that retargets single-target cards
    // at this player while the card is placed.
}

/// C# `CardExist.TurnStart` -> `End()` -- flip the card into the discard at the
/// start of this player's next turn; draw 1 when it never redirected anything.
fn turn_start(player_id: i32) {
    if !ctx::is_placed(player_id) || trigger::player_id() != player_id {
        return;
    }
    // 规则书: 「你的下回合开始时将其翻入弃牌堆」 -- C# `H.Unplace(this, "discard")`.
    let used = ctx::slot(player_id, SLOT_USED);
    ctx::unplace_card(player_id);
    ctx::to_discard(player_id, ID);
    // 规则书: 「若在此期间此卡没有造成影响，抽1张卡」 -- C# draws 1 only when
    // `Mem["used"]` was never set. The redirect hook that sets it is still
    // missing, so `SLOT_USED` is always 0 today (over-generous draw).
    // TODO(规则书): 「若在此期间此卡没有造成影响，抽1张卡」 -- needs the redirect
    // hook (C# `CardExist.Used` -> `Mem["used"] = 1`) to mark the card as having
    // fired; without it the draw always happens.
    if used == 0 {
        ctx::draw(player_id, 1);
    }
    ctx::log(player_id, &Msg::new(key!("exist_ended")).player_id("who", player_id));
}

//! `Sumimi:Here the world` -- C# `CardHereTheWorld` (MatchHost.cs:11313-11406):
//! [反击] onto the field of whoever played two cards this turn.
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:Here the world`）:
//! > Here the world：
//! >
//! > （1）[反击] 当有人同一回合内打出两张卡时，将此卡放置于对方场上。
//! >
//! > （2）场上有此卡的玩家下次抽卡时，将那张卡背面朝上放置于此卡上并为其放置3个奇迹水晶，那名玩家的每个回合开始时移除一个，当奇迹水晶数为0时，那名玩家将那张卡加入手牌，并使此卡使用者抽一张卡。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const HERE_THE_WORLD: CardDef = CardDef {
    id: "Sumimi:Here the world",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书（1）[反击]: 「当有人同一回合内打出两张卡时」 -- C# `t.Kind == "twoCards" && t.Seat != seat`.
    if trigger::kind() != TriggerKind::TwoCards {
        return false;
    }
    let them = trigger::seat();
    them != seat && !ctx::seat_out(them)
}

fn react(seat: i32) {
    let them = trigger::seat();
    // 规则书（1）[反击]: 「将此卡放置于对方场上」 -- C# `H.PlaceFromPlay(c, c.Trigger.Seat)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(them, "Sumimi:Here the world", &Msg::new(key!("here_the_world_note")));
    ctx::log(seat, &Msg::new(key!("here_the_world_placed")).seat("who", them).seat("by", seat));
    // TODO(规则书)（2）: 「场上有此卡的玩家下次抽卡时，将那张卡背面朝上放置于此卡上并为其放置3个奇迹水晶，
    // 那名玩家的每个回合开始时移除一个，当奇迹水晶数为0时，那名玩家将那张卡加入手牌，并使此卡使用者抽一张卡。」
    // -- needs the Fx.Drew / Fx.TurnStart persistent hooks (C#
    // `CardHereTheWorld.Drew` / `Tick`), a per-field-card crystal counter and a
    // held-card slot (`Mem["held"]` / `Crystals`; the ABI only has
    // `band_crystals` and seat tokens, not per-card crystals), plus `H.Unplace`
    // back to hand and a draw for the user (`H.DrawR(user, 1)`).
}
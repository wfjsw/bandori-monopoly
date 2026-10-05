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

use alloc::string::String;
use alloc::vec::Vec;

use card_sdk::abi::{TriggerKind, ChainKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

const ID: &str = "Sumimi:Here the world";

/// C# `Card.User` stand-in: the player that played this reaction (1-based; 0 = unset).
/// The card sits on another player's field; the draw-on-return goes to the user.
const SLOT_USER: &str = "here_user";

/// C# `Mem["held"]` -- the held card's id, packed as UTF-8 bytes (4 per slot,
/// little-endian) because `set_slot` is i32-only. C# stores a card-database
/// index; the bytes reconstruct the same id.
const SLOT_HELD_LEN: &str = "here_held_len";
/// Up to 64 bytes -- every `data/cards.json` id fits (max 52 bytes today).
const HELD_SLOTS: usize = 16;

/// Slot key for packed byte-chunk `i` (0..HELD_SLOTS).
fn held_key(i: usize) -> String {
    // "here_held_b0" .. "here_held_b15" without `format!` (keeps wasm32 lean).
    let mut s = String::from("here_held_b");
    if i >= 10 {
        s.push(char::from(b'0' + (i / 10) as u8));
    }
    s.push(char::from(b'0' + (i % 10) as u8));
    s
}

fn store_held(player_id: i32, id: &str) {
    let b = id.as_bytes();
    let n = b.len().min(HELD_SLOTS * 4);
    ctx::set_slot(player_id, SLOT_HELD_LEN, n as i32);
    for i in 0..HELD_SLOTS {
        let mut v: i32 = 0;
        for j in 0..4 {
            let k = i * 4 + j;
            if k < n {
                v |= i32::from(b[k]) << (8 * j);
            }
        }
        ctx::set_slot(player_id, &held_key(i), v);
    }
}

fn load_held(player_id: i32) -> Option<String> {
    let n = ctx::slot(player_id, SLOT_HELD_LEN);
    if n <= 0 {
        return None;
    }
    let n = n as usize;
    let mut b: Vec<u8> = Vec::with_capacity(n);
    for i in 0..HELD_SLOTS {
        let v = ctx::slot(player_id, &held_key(i));
        for j in 0..4 {
            if b.len() < n {
                b.push(((v >> (8 * j)) & 0xff) as u8);
            }
        }
    }
    String::from_utf8(b).ok()
}

fn clear_held(player_id: i32) {
    ctx::set_slot(player_id, SLOT_HELD_LEN, 0);
    for i in 0..HELD_SLOTS {
        ctx::set_slot(player_id, &held_key(i), 0);
    }
}

pub const HERE_THE_WORLD: CardDef = CardDef::new("Sumimi:Here the world", &[
    On::CounterAct(&[ChainKind::TwoCards], can_react, react),
    // 规则书（2）: the hold at the owner's next draw (C# `CardHereTheWorld.Drew`).
    On::Hook(&[HookKind::Drew], |_| true, drew),
    // 规则书（2）: the crystal tick at the owner's turn start (C# `TurnStart` -> `Tick`).
    On::Hook(&[HookKind::TurnStart], |_| true, turn_start),
]);

fn can_react(player_id: i32) -> bool {
    // 规则书（1）[反击]: 「当有人同一回合内打出两张卡时」 -- C# `t.Kind == "twoCards" && t.Seat != seat`.
    if trigger::kind() != TriggerKind::TwoCards {
        return false;
    }
    let them = trigger::player_id();
    them != player_id && !ctx::player_out(them)
}

fn react(player_id: i32) {
    let them = trigger::player_id();
    // 规则书（1）[反击]: 「将此卡放置于对方场上」 -- C# `H.PlaceFromPlay(c, c.Trigger.Seat)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(them, ID, &Msg::new(key!("here_the_world_note")));
    // C# `Card.User` = the reactor; the return draw goes to them.
    ctx::set_slot(them, SLOT_USER, player_id + 1);
    ctx::log(player_id, &Msg::new(key!("here_the_world_placed")).player_id("who", them).player_id("by", player_id));
}

/// 规则书（2）: 「场上有此卡的玩家下次抽卡时，将那张卡背面朝上放置于此卡上并为其放置3个奇迹水晶」
/// -- C# `CardHereTheWorld.Drew` (MatchHost.cs:11358-11373): the placed card
/// watches the owner's draw batch (`Fx.Drew`, `t.value` = count) and holds the
/// first card face-down with 3 miracle crystals.
fn drew(player_id: i32) {
    if trigger::kind() != TriggerKind::Drew {
        return;
    }
    // C# `if (player_id != Player || ...)` -- only the owner's own draws.
    if trigger::player_id() != player_id || !ctx::is_placed(player_id) {
        return;
    }
    // C# `if (... || Mem.ContainsKey("held"))` -- already holding one.
    if ctx::slot(player_id, SLOT_HELD_LEN) > 0 {
        return;
    }
    let got = trigger::value();
    if got <= 0 {
        return;
    }
    // C# `Drew(player_id, cards)` takes `cards[0]` -- the first card of the batch
    // (`trigger::cards()` = the drawn cards in draw order, C# `t.cards`).
    let cards = trigger::cards();
    let Some(id) = cards.first().cloned() else {
        return;
    };
    // C# `H._hidden[Player].hand.Remove(id)` -- hold it face-down (out of hand).
    if !ctx::take_from_hand(player_id, &id) {
        return;
    }
    store_held(player_id, &id);
    // 规则书（2）: 「并为其放置3个奇迹水晶」
    ctx::set_crystals(player_id, 3);
    ctx::log(player_id, &Msg::new(key!("here_the_world_held")).player_id("who", player_id));
}

/// 规则书（2）: 「那名玩家的每个回合开始时移除一个，当奇迹水晶数为0时，那名玩家将那张卡
/// 加入手牌，并使此卡使用者抽一张卡。」 -- C# `CardHereTheWorld.TurnStart` -> `Tick`
/// (MatchHost.cs:11375-11406).
fn turn_start(player_id: i32) {
    if trigger::kind() != TriggerKind::TurnStart {
        return;
    }
    // C# `if (turn != Player || !Mem.ContainsKey("held") || !H._placed.Contains(this))`.
    if trigger::player_id() != player_id || !ctx::is_placed(player_id) {
        return;
    }
    if ctx::slot(player_id, SLOT_HELD_LEN) <= 0 {
        return;
    }
    // C# `AddCrystals(-1, "回合开始")`.
    let left = ctx::add_crystals(player_id, -1, 0);
    if left > 0 {
        return;
    }
    // C# `Tick` at 0: return the held card, unplace this one, draw for the user.
    let user = ctx::slot(player_id, SLOT_USER) - 1;
    if let Some(id) = load_held(player_id) {
        // C# `H.AddToHand(Seat, held)`.
        ctx::add_to_hand(player_id, &id);
    }
    clear_held(player_id);
    ctx::log(player_id, &Msg::new(key!("here_the_world_returned")).player_id("who", player_id));
    // C# `H.Unplace(this, "discard", "结束了")`.
    ctx::unplace_card(player_id);
    ctx::to_discard(player_id, ID);
    ctx::set_slot(player_id, SLOT_USER, 0);
    // C# `if (!H.Out(user)) yield return H.DrawR(user, 1, CardName)`.
    if user >= 0 && !ctx::player_out(user) {
        ctx::draw(user, 1);
    }
}

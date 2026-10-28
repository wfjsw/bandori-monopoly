//! `Sumimi:Here the world` -- C# `CardHereTheWorld` (MatchHost.cs:11313-11406):
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:Here the world`）:
//! > Here the world：
//! >  
//! > （1）[反击] 当有人同一回合内打出两张卡时，将此卡放置于对方场上。
//! > （2）场上有此卡的玩家下次抽卡时，将那张卡背面朝上放置于此卡上并为其放置3个奇迹水晶，那名玩家的每个回合开始时移除一个，当奇迹水晶数为0时，那名玩家将那张卡加入手牌，并使此卡使用者抽一张卡。
//!
//! [反击] onto the field of whoever played two cards this turn.

use alloc::string::String;
use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, ChainKind};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

const ID: &str = "Sumimi:Here the world";

/// C# `Card.User` stand-in: the player that played this counteraction (1-based; 0 = unset).
/// The card sits on another player's field; the draw-on-return goes to the user.
/// A **prop of this instance**, so two copies on one field keep their own user.
const USER_PROP: &str = "here_user";

/// C# `Mem["held"]` -- the held card's id, packed as UTF-8 bytes (4 per prop,
/// little-endian) because a prop is i32-only. C# stores a card-database
/// index; the bytes reconstruct the same id. The length is an on-card counter
/// (`here_held_len`) so a `pre` or the body can read it; both live on the
/// **instance**, not the player.
const HELD_LEN: &str = "here_held_len";
/// Up to 64 bytes -- every `the roster` id fits (max 52 bytes today).
const HELD_PROPS: usize = 16;

/// Prop key for packed byte-chunk `i` (0..HELD_PROPS).
fn held_key(i: usize) -> String {
    // "here_held_b0" .. "here_held_b15" without `format!` (keeps wasm32 lean).
    let mut s = String::from("here_held_b");
    if i >= 10 {
        s.push(char::from(b'0' + (i / 10) as u8));
    }
    s.push(char::from(b'0' + (i % 10) as u8));
    s
}

fn store_held(id: &str) {
    let b = id.as_bytes();
    let n = b.len().min(HELD_PROPS * 4);
    ctx::set_counter(HELD_LEN, n as i32);
    for i in 0..HELD_PROPS {
        let mut v: i32 = 0;
        for j in 0..4 {
            let k = i * 4 + j;
            if k < n {
                v |= i32::from(b[k]) << (8 * j);
            }
        }
        ctx::set_prop(&held_key(i), v);
    }
}

fn load_held() -> Option<String> {
    let n = ctx::counter(HELD_LEN);
    if n <= 0 {
        return None;
    }
    let n = n as usize;
    let mut b: Vec<u8> = Vec::with_capacity(n);
    for i in 0..HELD_PROPS {
        let v = ctx::prop(&held_key(i));
        for j in 0..4 {
            if b.len() < n {
                b.push(((v >> (8 * j)) & 0xff) as u8);
            }
        }
    }
    String::from_utf8(b).ok()
}

fn clear_held() {
    ctx::set_counter(HELD_LEN, 0);
    for i in 0..HELD_PROPS {
        ctx::set_prop(&held_key(i), 0);
    }
}

pub const HERE_THE_WORLD: CardDef = CardDef::new(
    "Sumimi:Here the world",
    &[
        On::Counteract(
            &[ChainKind::TwoCards],
            "actor != owner",
            Some(can_counteract),
            counteract).counter_reason(key!("here_the_world_when")),
        // TODO(规则书)（当有人同一回合内打出两张卡时）: 打出的反击也是「打出」的一张
        // -- `turn.cards_played` 只在 `play_from_hand` 里加，而反击宣言走
        // `hand_counteractions`/`declare_one`，一对里含反击就永远凑不到两张。
        // 规则书: 「场上有此卡的玩家下次抽卡时」 -- the hold at the owner's next
        // draw (C# `CardHereTheWorld.Drew`). The hold gate is re-checked in the
        // body against **this** instance: `card.counter(…)` in a `pre` resolves
        // by card id, which two copies on one field would share.
        On::Action(Action::Drew("actor == owner && card.placed", None, drew)),
        // 规则书: 「那名玩家的每个回合开始时移除一个」 -- the crystal tick at the
        // owner's turn start (C# `TurnStart` -> `Tick`).
        On::Action(Action::TurnStart("actor == owner && card.placed", None, turn_start)),
        // TODO(规则书)（场上有此卡的玩家下次抽卡时）: 被扣住的卡只记在本卡的
        // 计数/属性上，此卡在完成前离场就没人把它送回任何堆 -- 场上卡还没有“离场”
        // 钩子（只有玩家离场的 `leaveAfter`），cleanup 无处可挂。
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Here the world"#)
        .raw_name(r#"Here the world"#)
        .tags(&[r#"反击"#])
        .text(CardText::items(&[
            it(r#"[反击] 当有人同一回合内打出两张卡时，将此卡放置于对方场上。"#),
            it(r#"场上有此卡的玩家下次抽卡时，将那张卡背面朝上放置于此卡上并为其放置3个奇迹水晶，那名玩家的每个回合开始时移除一个，当奇迹水晶数为0时，那名玩家将那张卡加入手牌，并使此卡使用者抽一张卡。"#),
        ]))
        // sheet cell L7
        .fields(&[("group", r#"Sumimi"#), ("owner", "")]));

/// Residual the condition cannot express: the actor is still in the game
/// (`actor != owner` is the `pre`).
fn can_counteract(player_id: i32) -> bool {
    let them = trigger::player_id();
    let _ = player_id;
    !ctx::player_out(them)
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    let them = trigger::player_id();
    // 规则书（1）[反击]: 「将此卡放置于对方场上」 -- C# `H.PlaceFromPlay(c, c.Trigger.Seat)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(them, ID, &Msg::new(key!("here_the_world_note")));
    // C# `Card.User` = the counteractor; the return draw goes to them. Written
    // on the instance `place_card` just bound (not on the player).
    ctx::set_prop(USER_PROP, player_id + 1);
    ctx::log(
        player_id,
        &Msg::new(key!("here_the_world_placed"))
            .player_id("who", them)
            .player_id("by", player_id),
    );
    Ok(())
}

/// 规则书（2）: 「场上有此卡的玩家下次抽卡时，将那张卡背面朝上放置于此卡上并为其放置3个奇迹水晶」
/// -- C# `CardHereTheWorld.Drew` (MatchHost.cs:11358-11373): the placed card
/// watches the owner's draw (`Fx.Drew`, raised once per single card with
/// `value` = 1) and holds that card with 3 miracle crystals.
fn drew(player_id: i32) -> card_sdk::Asked {
    // The instance's own hold latch (`card.counter` in the `pre` is name-keyed).
    if ctx::counter(HELD_LEN) > 0 {
        return Ok(());
    }
    let got = trigger::value();
    if got <= 0 {
        return Ok(());
    }
    // C# `Drew(player_id, cards)` takes `cards[0]` -- the card of this
    // single-card draw (`trigger::cards()` = the drawn cards in draw order, C#
    // `t.cards`).
    let cards = trigger::cards();
    let Some(&id) = cards.first() else {
        return Ok(());
    };
    // C# `H._hidden[Player].hand.Remove(id)` -- hold it out of hand.
    // TODO(规则书)（将那张卡背面朝上放置于此卡上）: 书里把它背面朝上放在此卡上 --
    // 我们只把它从手牌取出、把 id 记在本卡上，卡本身不入场也不翻面。
    if !ctx::take_card(player_id, id.as_str(), ctx::CardPile::Hand) {
        return Ok(());
    }
    store_held(id.as_str());
    // 规则书（2）: 「并为其放置3个奇迹水晶」
    ctx::set_crystals(3);
    ctx::log(
        player_id,
        &Msg::new(key!("here_the_world_held")).player_id("who", player_id),
    );
    Ok(())
}

/// 规则书（2）: 「那名玩家的每个回合开始时移除一个，当奇迹水晶数为0时，那名玩家将那张卡
/// 加入手牌，并使此卡使用者抽一张卡。」 -- C# `CardHereTheWorld.TurnStart` -> `Tick`
/// (MatchHost.cs:11375-11406).
fn turn_start(player_id: i32) -> card_sdk::Asked {
    // Only this instance's own hold ticks (`card.counter` in the `pre` is
    // name-keyed, so two copies would read each other's latch).
    if ctx::counter(HELD_LEN) <= 0 {
        return Ok(());
    }
    // C# `AddCrystals(-1, "回合开始")`.
    let left = ctx::add_crystals(-1, 0)?;
    if left > 0 {
        return Ok(());
    }
    // C# `Tick` at 0: return the held card, unplace this one, draw for the user.
    let user = ctx::prop(USER_PROP) - 1;
    if let Some(id) = load_held() {
        // C# `H.AddToHand(Seat, held)`.
        ctx::add_to_dest(player_id, &id, ctx::Dest::Hand);
    }
    clear_held();
    ctx::log(
        player_id,
        &Msg::new(key!("here_the_world_returned")).player_id("who", player_id),
    );
    // C# `H.Unplace(this, "discard", "结束了")`.
    // TODO(规则书)（当奇迹水晶数为0时，那名玩家将那张卡加入手牌）: 书里不说此卡
    // 何时离场 -- 我们完成时把它弃置（W3-HERE-01 / C-Q49）。
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::set_prop(USER_PROP, 0);
    // C# `if (!H.Out(user)) yield return H.DrawR(user, 1, CardName)`.
    if user >= 0 && !ctx::player_out(user) {
        ctx::draw(user, 1)?;
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn drew_kani(player_id: i32) -> card_sdk::Asked {
    drew(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_start_kani(player_id: i32) -> card_sdk::Asked {
    turn_start(player_id)
}

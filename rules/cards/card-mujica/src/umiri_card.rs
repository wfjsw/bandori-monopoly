//! `Mujica:（海铃）` -- C# `CardUmiriCard` (MatchHost.cs:6117-6258): a travelling
//! placed card that walks one player per turn and takes band skill cards.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（海铃）`）:
//! > （海铃）
//! >
//! > （1）将此卡放置于在此卡使用者下一名行动的玩家场上，轮到使用者的回合开始时，将其移动到其所在场的玩家行动序列后一名的玩家场上。
//! >
//! > （2）使用者打出此卡时以及使用者的回合开始时，从卡堆拿取场上有此卡的玩家的所有乐队技能卡（相同乐队技能卡的效果不可叠加），但不视为那个乐队的角色。
//! >
//! > （3）当此卡回到使用者场上时，使用者回合结束时将此卡与使用者拿取的所有乐队技能卡置入弃牌堆，抽一张卡。
//!

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

const ID: &str = "Mujica:（海铃）";

/// C# `Card.User` stand-in: the player that played the card (the card may sit on
/// another player's field). Stored 1-based at the placement player so 0 reads as
/// "unset".
const USER_KEY: &str = "umiri_user";

pub const UMIRI_CARD: CardDef = CardDef::new("Mujica:（海铃）", &[
    On::Play(Some(cant_play), umiri_card),
    // 规则书（1）: the hop at the user's turn start (C# `CardUmiriCard.TurnStart`).
    On::Hook(&[HookKind::TurnStart], |_| true, turn_start),
    // 规则书（3）: the discard + draw when the card is back at the user's field
    // (C# `CardUmiriCard.TurnEnd` -> `End`).
    On::Hook(&[HookKind::TurnEnd], |_| true, turn_end)]);

/// C# `CardUmiriCard.WhyNot` -- 「没有别的玩家」 when `H.Others` is empty.
fn cant_play(player_id: i32) -> Option<Msg> {
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("x_no_others")));
    }
    None // playable
}

fn umiri_card(player_id: i32) {
    // 规则书（1）: 「将此卡放置于在此卡使用者下一名行动的玩家场上」 -- C#
    // `H.PlaceFromPlay(c, NextOf(seat))` puts the card on the next live player's
    // field. `place_card` always places at the given player; the "next actor" is
    // the next live player in turn order (`NextOf` walks `player_id + 1 ..` skipping
    // outed players).
    let next = next_of(player_id);
    if next == player_id {
        // C# `if (num != player_id)` -- nothing to do with no other live player.
        return;
    }
    // 规则书（1）: 「将此卡放置于在此卡使用者下一名行动的玩家场上」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(next, ID, &Msg::new(key!("umiri_note")));
    ctx::set_slot(next, USER_KEY, player_id + 1);
    ctx::log(
        player_id,
        &Msg::new(key!("umiri_placed"))
            .player_id("who", player_id)
            .player_id("holder", next),
    );
    // 规则书（2）: 「使用者打出此卡时以及使用者的回合开始时，从卡堆拿取场上有此卡的玩家的所有乐队技能卡（相同乐队技能卡的效果不可叠加），但不视为那个乐队的角色。」
    // C# `CardUmiriCard.Take` copies every non-extra `BandBase` of the holder
    // onto the user as `extra: true` (stackable band skills, but "not that
    // band's character"). The ABI has no band-skill-card inventory (only
    // `band_crystals` / `add_band_crystals` for the crystal counter).
    // TODO(ABI): `H._fx[player_id].bands` enumeration + `H.MakeBand(band, user, extra)`
    // so the take / drop bookkeeping (C# `_taken`) can run.
}

/// C# `CardUmiriCard.TurnStart` (MatchHost.cs:6189-6214) -- at the user's turn
/// start, hop the card to the next live player after its holder (or back to the
/// user). Runs through the Fx hook dispatch at `turnStart`, so this is a field
/// effect, not a [反击].
fn turn_start(player_id: i32) {
    // C# `if (turn != User || !H._placed.Contains(this) || Player == User) return null;`
    let user = ctx::slot(player_id, USER_KEY) - 1;
    if trigger::player_id() != user || player_id == user || !ctx::is_placed(player_id) {
        return;
    }
    // 规则书（1）: 「轮到使用者的回合开始时，将其移动到其所在场的玩家行动序列后一名的玩家场上。」
    // C# `int num = NextOf(Player); if (num == User) Player = User; else Player = num`.
    let dest = next_of(player_id);
    ctx::unplace_card(player_id);
    ctx::set_slot(player_id, USER_KEY, 0);
    ctx::place_card(dest, ID, &Msg::new(key!("umiri_note")));
    ctx::set_slot(dest, USER_KEY, user + 1);
    if dest == user {
        // C# `H.Log("place", User, "「（海铃）」回到了 ... 的场上：回合结束时放入弃卡区")`.
        ctx::log(
            user,
            &Msg::new(key!("umiri_returned")).player_id("who", user),
        );
    } else {
        // C# `H.Log("place", num, "「（海铃）」移到了 ... 的场上")`.
        ctx::log(
            dest,
            &Msg::new(key!("umiri_moved")).player_id("who", dest).player_id("user", user),
        );
        // 规则书（2）: `Take()` -- the band-skill take on the new holder. Held
        // (no `H._fx[player_id].bands` / `H.MakeBand` in the ABI); see `umiri_card`.
    }
}

/// C# `CardUmiriCard.TurnEnd` -> `End` (MatchHost.cs:6238-6252) -- when the
/// card is back on the user's field at the user's turn end, discard it and
/// draw 1. Runs through the Fx hook dispatch at `turnEnd`, so this is a field
/// effect, not a [反击].
fn turn_end(player_id: i32) {
    // C# `if (turn != User || Player != User || !H._placed.Contains(this)) return null;`
    let user = ctx::slot(player_id, USER_KEY) - 1;
    if trigger::player_id() != user || player_id != user || !ctx::is_placed(player_id) {
        return;
    }
    // 规则书（3）: 「当此卡回到使用者场上时，使用者回合结束时将此卡与使用者拿取的所有乐队技能卡置入弃牌堆，抽一张卡。」
    // C# `End`: `H.Unplace(this, "discard", "回到了使用者的场上")` +
    // `H.DrawR(User, 1, ...)`. The `Detach` -> `Drop` of the taken band cards
    // is the held half (no band inventory in the ABI).
    ctx::unplace_card(player_id);
    ctx::to_discard(player_id, ID);
    ctx::set_slot(player_id, USER_KEY, 0);
    ctx::draw(user, 1);
    ctx::log(user, &Msg::new(key!("umiri_ended")).player_id("who", user));
}

/// C# `CardUmiriCard.NextOf` -- the next live player after `holder` in turn order
/// (wraps to `holder` when there is none). Not `H.Neighbor` / `ctx::neighbor`:
/// that one skips exiled players too and returns -1 instead of `holder`.
fn next_of(holder: i32) -> i32 {
    let n = ctx::player_count();
    if n <= 0 {
        return holder;
    }
    for i in 1..=n {
        let s = (holder + i).rem_euclid(n);
        if !ctx::player_out(s) {
            return s;
        }
    }
    holder
}

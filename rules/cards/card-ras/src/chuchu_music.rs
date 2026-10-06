//! `RAS:（chuchu）演奏我的音乐吧` -- C# `CardChuchuMusic` (MatchHost.cs:9905-10002):
//! park on another player; their buys pay the user and pass the card along.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（chuchu）演奏我的音乐吧`）:
//! > （chuchu）演奏我的音乐吧：
//! >  将此卡放置于你以外的一名玩家场上并为其添加3个奇迹水晶，那名玩家的每个回合结束时失去一个；场上存在此卡的玩家下次购买地契时，[使用者]获得100资金，将此卡移至除[使用者]外行动序列下一名玩家的场上并将奇迹水晶补充至3个；此卡进入弃牌堆前每触发一次该效果，此卡获得资金时额外获得100（上限500）。此卡奇迹水晶为0时，放入[使用者]的弃牌堆并使[使用者]抽一张卡。
//!

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const CHUCHU_MUSIC: CardDef = CardDef::new(
    "RAS:（chuchu）演奏我的音乐吧",
    &[
        On::Play(Some(cant_play), play),
        On::Hook(&[HookKind::TurnEnd], turn_end_guard, turn_end),
        On::Hook(&[HookKind::BuyAfter], buy_after_guard, buy_after),
    ],
);

const ID: &str = "RAS:（chuchu）演奏我的音乐吧";

/// The player who played the card (C# `User`).
const SLOT_USER: &str = "chuchu_music_user";
/// How many times the buy effect has fired (C# `Mem["hits"]`).
const SLOT_HITS: &str = "chuchu_music_hits";

/// C# `CardChuchuMusic.WhyNot`: 「没有别的玩家」 when `H.Others(seat)` is empty.
fn cant_play(player_id: i32) -> Option<Msg> {
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("chuchu_music_no_others")));
    }
    None
}

fn play(player_id: i32) -> card_sdk::Asked {
    let others = ctx::others(player_id);
    // C# `WhyNot` refuses the play with no other player alive; the gate is now
    // `cant_play` above.
    if others.is_empty() {
        return Ok(());
    }
    // 规则书: 「将此卡放置于你以外的一名玩家场上」 -- C# `H.PickTarget` over
    // `H.Others(seat)` then `H.PlaceFromPlay(c, r.index, -1, 3)`.
    // `H.PickTarget` = the player prompt, then the `H.Target` gate (C#
    // `Target(c, r.index, t); res.index = t.yes ? t.index : -1`); a failed
    // target means `c.Effective = false` and no placement.
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("chuchu_music_title")),
        &Msg::new(key!("chuchu_music_ask")),
        &others,
    )?;
    let hit = match ctx::target(who) {
        Some(h) => h,
        None => return Ok(()),
    };
    // 规则书: 「将此卡放置于你以外的一名玩家场上」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(hit, ID, &Msg::new(key!("chuchu_music_note")));
    // 规则书: 「并为其添加3个奇迹水晶」 -- C# `H.PlaceFromPlay(c, r.index, -1, 3)`.
    ctx::set_crystals(3);
    // C# `User` is the player who played the card; `Mem["hits"]` starts at 0.
    ctx::set_slot(hit, SLOT_USER, player_id);
    ctx::set_slot(hit, SLOT_HITS, 0);
    ctx::log(
        player_id,
        &Msg::new(key!("chuchu_music_placed"))
            .player_id("who", player_id)
            .player_id("target", hit),
    );
    Ok(())
}

/// `DecayCard.TurnEnd` (C# `CardChuchuMusic : DecayCard`) -- burn one miracle
/// crystal at the holder's turn end; at 0 the card goes to the user's discard
/// and the user draws (C# `Empty` -> `Done`).
/// Pure guard for [`turn_end`] -- the activation gate. `false`
/// means the card is not activated at all.
fn turn_end_guard(player_id: i32) -> bool {
    ctx::is_placed() && trigger::player_id() == player_id
}

fn turn_end(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「那名玩家的每个回合结束时失去一个」 -- C# `AddCrystals(-1, ...)`.
    if ctx::add_crystals(-1, 0) > 0 {
        return Ok(());
    }
    // 规则书: 「此卡奇迹水晶为0时，放入[使用者]的弃牌堆并使[使用者]抽一张卡」
    // -- C# `Empty` -> `Done`: unplace, discard for the user, user draws 1.
    let user = ctx::slot(player_id, SLOT_USER);
    ctx::set_transfer_to_dest(
        if user >= 0 { user } else { player_id },
        ctx::Dest::Graveyard,
    );
    if user >= 0 && !ctx::player_out(user) {
        ctx::draw(user, 1);
    }
    ctx::log(
        player_id,
        &Msg::new(key!("chuchu_music_empty")).player_id("who", player_id),
    );
    Ok(())
}

/// C# `CardChuchuMusic.Bought` / `Move` -- when the holder buys a tile, the user
/// gains `Pay` money, the card hops to the next non-user player, and crystals
/// refill to 3.
/// Pure guard for [`buy_after`] -- the activation gate. `false`
/// means the card is not activated at all.
fn buy_after_guard(player_id: i32) -> bool {
    ctx::is_placed() && trigger::player_id() == player_id
}

fn buy_after(player_id: i32) -> card_sdk::Asked {
    let user = ctx::slot(player_id, SLOT_USER);
    if user < 0 || ctx::player_out(user) {
        return Ok(());
    }
    // 规则书: 「场上存在此卡的玩家下次购买地契时，[使用者]获得100资金」
    // -- C# `Pay = Math.Min(500, 100 + 100 * Hits)`; each trigger bumps Hits.
    let hits = ctx::slot(player_id, SLOT_HITS);
    let pay = (100 + 100 * hits).min(500);
    ctx::set_slot(player_id, SLOT_HITS, hits + 1);
    ctx::gain(
        user,
        pay,
        &Msg::new(key!("chuchu_music_gain")).i("n", pay as i64),
    );
    // 规则书: 「将此卡移至除[使用者]外行动序列下一名玩家的场上并将奇迹水晶补充至3个」
    // -- C# `Move()`: next player in action order skipping the user and outed players.
    let n = ctx::player_count();
    let mut to = -1;
    for i in 1..n {
        let cand = (player_id + i) % n;
        if cand != user && !ctx::player_out(cand) {
            to = cand;
            break;
        }
    }
    if to < 0 {
        return Ok(());
    }
    // Carry the user/hits slots across the hop.
    ctx::set_slot(to, SLOT_USER, user);
    ctx::set_slot(to, SLOT_HITS, hits + 1);
    ctx::unplace_self();
    ctx::place_card(to, ID, &Msg::new(key!("chuchu_music_note")));
    // 规则书: 「并将奇迹水晶补充至3个」 -- C# `Crystals = 3`.
    ctx::set_crystals(3);
    ctx::log(
        user,
        &Msg::new(key!("chuchu_music_moved"))
            .player_id("who", to)
            .i("n", pay as i64),
    );
    Ok(())
}

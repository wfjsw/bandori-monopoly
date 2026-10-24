//! `skill:桐谷透子:天上天下，唯我独尊`
//!
//! 规则书（skill sheet, 桐谷透子）:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）
//! > （2）一回合一次，在你的经营阶段，你可以消耗一个火罐并指定你的一个地块，
//! > 指定前后各一格范围内（不包括该格子本身）的所有其他玩家支付你X，X为此地的
//! > 地租的一半/指定玩家数（向上取整百）。
//!
//! （1） is `Pass` onto CiRCLE.
//!
//! （2） 「指定前后各一格范围内（不包括该格子本身）」 is the two neighbours of
//! the named tile, and 「所有其他玩家支付你X」 splits one rent-derived figure
//! across whoever is standing there. X is 「此地的地租的一半/指定玩家数（向上
//! 取整百）」 -- half the tile's rent, divided by how many players are being
//! charged, rounded **up to a hundred**.

use bandori_std::ctx::{self, state};
use bandori_std::state_key;
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};

/// Latch for 「一回合一次」.
const USED: &str = "skill.kiritani.used";

pub const KIRITANI_ZENITH: CardDef = CardDef::new(
    "skill:桐谷透子:天上天下，唯我独尊",
    &[
        On::Play("", None, Some(can_use), use_skill),
        On::Action(Action::TurnStartBefore("", None, None, declare_cap)),
        On::Action(Action::DeckAtGameStart("", None, None, declare_cap)),
        On::Action(Action::Pass(
            bandori_std::guards::OWN_START,
            None,
            None,
            on_pass,
        )),
        On::Action(Action::TurnStartBefore(
            card_sdk::pre::MINE,
            None,
            None,
            reset,
        )),
    ],
)
.silent(&[1, 2, 4])
// 「（初始1，上限1）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 1),
    (bandori_std::fire::FIRE_CAP, 1),
]);

/// 「初始1，上限1」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

fn reset(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, USED, 0);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("kiritani_gain")))?;
    Ok(())
}

/// （2） 「一回合一次」 + 「消耗一个火罐」.
fn can_use(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, USED) != 0 {
        return Some(Msg::new(key!("kiritani_used")));
    }
    if bandori_std::fire::get(player_id) < 1 {
        return Some(Msg::new(key!("kiritani_no_fire")));
    }
    let mine = ctx::owned_tiles(player_id);
    if mine.is_empty() {
        return Some(Msg::new(key!("kiritani_no_tile")));
    }
    None
}

/// （2）「指定你的一个地块，指定前后各一格范围内（不包括该格子本身）的所有
/// 其他玩家支付你X」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let mine = ctx::owned_tiles(player_id);
    if mine.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("kiritani_title")),
        &Msg::new(key!("kiritani_ask")),
        &mine
            .iter()
            .map(|&t| Msg::new(key!("kiritani_option")).tile("tile", t))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(&tile) = mine.get(pick) else {
        return Ok(());
    };
    // 「指定前后各一格范围内（不包括该格子本身）」 -- the two neighbours.
    let mut targets: alloc::vec::Vec<i32> = alloc::vec::Vec::new();
    for t in neighbours(tile) {
        for p in ctx::players_on(t, player_id) {
            if p != player_id && !ctx::player_out(p) {
                targets.push(p);
            }
        }
    }
    if targets.is_empty() {
        ctx::log(
            player_id,
            &Msg::new(key!("kiritani_nobody")).tile("tile", tile),
        );
        return Ok(());
    }
    if !bandori_std::fire::spend(player_id, 1, &Msg::new(key!("kiritani_spend")))? {
        return Ok(());
    }
    state::set(player_id, USED, 1);
    // X = 「此地的地租的一半/指定玩家数（向上取整百）」.
    let half = ctx::rent_of(tile) / 2;
    let x = ceil_hundred(half / targets.len() as i32);
    for p in targets {
        ctx::transfer(
            p,
            player_id,
            x,
            &Msg::new(key!("kiritani_why"))
                .tile("tile", tile)
                .n("n", x as i64),
        )?;
    }
    Ok(())
}

/// The two tiles either side of `tile`, wrapping.
fn neighbours(tile: i32) -> alloc::vec::Vec<i32> {
    let n = ctx::tile_count();
    if n <= 0 {
        return alloc::vec::Vec::new();
    }
    alloc::vec![(tile - 1).rem_euclid(n), (tile + 1).rem_euclid(n),]
}

/// 「向上取整百」 -- round up to a multiple of 100.
fn ceil_hundred(x: i32) -> i32 {
    if x <= 0 {
        0
    } else {
        ((x + 99) / 100) * 100
    }
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn use_skill_kani(player_id: i32) -> card_sdk::Asked {
    use_skill(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn reset_kani(player_id: i32) -> card_sdk::Asked {
    reset(player_id)
}

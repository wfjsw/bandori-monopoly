//! `PPP:[衍生]Popipapapipopa` -- C# `CardPopipapapipopa` (MatchHost.cs:8352-8402):
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:[衍生]Popipapapipopa`）:
//! > [衍生]Popipapapipopa：
//! > [持续]
//!
//! > （1）此卡拥有者每次[经过]“东京外”，“江户川乐器店”，“山吹面包房”，“星之鼓动山丘”或“流星堂”时为此卡添加1个[奇迹水晶]（上限10个）。
//!
//! > （2）消耗或支付时可使用此卡多个奇迹水晶，每个将资金变动减少150（最少0）。
//!
//! placed card that banks [奇迹水晶] when its owner passes five named tiles and
//! spends them to shrink payments.
//! A pure [持续] card: the C# has no `Play`, it is placed by `CardPipopa`
//! (`place_card` in `pipopa.rs`). Both continuous clauses are Fx hooks.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const POPIPAPAPIPOPA: CardDef = CardDef::new(
    "PPP:[衍生]Popipapapipopa",
    &[
        On::Hook(&[HookKind::PassTile], Some(pass_tile_guard), pass_tile, ""),
        On::Hook(&[HookKind::PayChoose], Some(pay_choose_guard), pay_choose, ""),
    ],
);

/// C# `CardPopipapapipopa.Spots` -- the five tiles that feed a crystal.
const SPOTS: [&str; 5] = [
    "东京外",
    "江户川乐器店",
    "山吹面包房",
    "星之鼓动山丘",
    "流星堂",
];

/// C# `MaxCrystals = 10`.
const MAX_CRYSTALS: i32 = 10;

/// `Fx.PassTile` (C# `CardPopipapapipopa.PassTile`) -- the owner passing one of
/// the five named tiles banks a crystal on this card.
/// Pure guard for [`pass_tile`] -- the activation gate. `false`
/// means the card is not activated at all.
fn pass_tile_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // C# `m.Seat != Seat` -- only the owner's own move feeds this card.
    if trigger::player_id() != player_id {
        return Ok(());
    }
    let t = trigger::tile();
    if t < 0 {
        return Ok(());
    }
    // 规则书[持续]（1）: 「每次[经过]…时为此卡添加1个[奇迹水晶]（上限10个）。」
    let mut hit = false;
    for name in SPOTS {
        if ctx::tile_named(name) == t {
            hit = true;
            break;
        }
    }
    if !hit {
        return Ok(());
    }
    let before = ctx::crystals();
    let after = ctx::add_crystals(1, MAX_CRYSTALS)?;
    if after > before {
        ctx::log(
            player_id,
            &Msg::new(key!("popipapapipopa_crystal"))
                .player_id("who", player_id)
                .tile("tile", t)
                .i("n", after as i64),
        );
    }
    Ok(())
}

/// `Fx.PayChoose` (C# `CardPopipapapipopa.PayChoose` -> `Use`) -- the owner may
/// spend crystals to shrink the pending payment by 150 each.
/// Pure guard for [`pay_choose`] -- the activation gate. `false`
/// means the card is not activated at all.
fn pay_choose_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn pay_choose(player_id: i32) -> card_sdk::Asked {
    // C# `p.from != Player` -- only the owner's own payment.
    if trigger::player_id() != player_id {
        return Ok(());
    }
    let amount = trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    let have = ctx::crystals();
    if have <= 0 {
        return Ok(());
    }
    // C# `max = Math.Min(Crystals, (p.amount + 149) / 150)` -- never ask for
    // more than can actually cut the payment to zero.
    let max = have.min((amount + 149) / 150);
    // 规则书[持续]（2）: 「消耗或支付时可使用此卡多个奇迹水晶，每个将资金变动减少150（最少0）。」
    let n = ctx::ask_number(
        player_id,
        &Msg::new(key!("popipapapipopa_title")),
        &Msg::new(key!("popipapapipopa_ask"))
            .n("money", amount as i64)
            .i("n", have as i64),
        0,
        max,
    )?;
    if n <= 0 {
        return Ok(());
    }
    ctx::add_crystals(-n, 0)?;
    let cut = 150 * n;
    // C# `p.amount = Math.Max(0, p.amount - 150 * r.value)`.
    trigger::set_pay_amount((amount - cut).max(0));
    ctx::log(
        player_id,
        &Msg::new(key!("popipapapipopa_used"))
            .player_id("who", player_id)
            .i("n", n as i64)
            .n("money", cut as i64),
    );
    Ok(())
}

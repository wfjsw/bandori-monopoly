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
        // 规则书[持续]（1）: 「此卡拥有者每次[经过]“东京外”，“江户川乐器店”，
        // “山吹面包房”，“星之鼓动山丘”或“流星堂”时」 -- the five named spots
        // are the condition.
        On::Hook(
            &[HookKind::PassTile],
            "card.placed && actor == owner && (tile.id == tile_named('东京外') || tile.id == tile_named('江户川乐器店') || tile.id == tile_named('山吹面包房') || tile.id == tile_named('星之鼓动山丘') || tile.id == tile_named('流星堂'))",
            None,
            pass_tile,
        ),
        // 规则书[持续]（2）: 「消耗或支付时可使用此卡多个奇迹水晶」 -- a payment
        // the owner owes, with a crystal to spend.
        On::Hook(
            &[HookKind::PayChoose],
            "card.placed && actor == owner && value > 0 && card.counter('crystals') > 0",
            None,
            pay_choose,
        ),
    ],
);

// C# `CardPopipapapipopa.Spots` -- the five tiles that feed a crystal (now the
// `PassTile` pre's `tile_named` disjunction).

/// C# `MaxCrystals = 10`.
const MAX_CRYSTALS: i32 = 10;

/// `Fx.PassTile` (C# `CardPopipapapipopa.PassTile`) -- the owner passing one of
/// the five named tiles banks a crystal on this card.
fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // `card.placed && actor == owner && (tile.id == tile_named('…') || …)` is
    // the pre.
    let t = trigger::tile();
    // 规则书[持续]（1）: 「每次[经过]…时为此卡添加1个[奇迹水晶]（上限10个）。」
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
fn pay_choose(player_id: i32) -> card_sdk::Asked {
    // `card.placed && actor == owner && value > 0 && card.counter('crystals') > 0` is the pre.
    let amount = trigger::value();
    let have = ctx::crystals();
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

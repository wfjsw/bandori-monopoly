//!
//! 规则书（docs/rulebook/cards.json, id `RAS:狂乱Hey Kids!!`）:
//! > 狂乱Hey Kids!!：
//! > [反击] 在属于你的格子上结算时，打出此卡，将本次结算改为：将本格上的房屋转移到属于你的可建造格子上。转移的房屋与目标格子中，至少一方数量为1，且每个目标格子至多获得1层房屋。转移时，消耗的房屋造价等于获得的房屋总造价，超出的部分作为现金获得；随后，你失去“转移后各格房屋造价总和－获得房屋数量×500”的资金。
//!
//! replace the settle with a house transfer off the settled tile.

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ChainKind;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const HEY_KIDS: CardDef = CardDef::new(
    "RAS:狂乱Hey Kids!!",
    // `ChainKind::SettleBody`, not `Settle` (`docs/TILES.md`): 「将本次结算改为」
    // is a **body replacement**, so it answers the body link and `settleAfter`
    // still runs. Answering the outer `settle` would mean 「the settle never
    // happened」.
    &[On::Counteract(
        &[ChainKind::SettleBody],
        // TODO(规则书)（在属于你的格子上结算时）: 规则书[反击]: 「在属于你的格子上结算时」 -- on *another* player's settle
        // on my tile. A mortgaged deed charges no rent (the settle offers a forced
        // purchase instead), so it does not open this.
        "actor != owner && tile.id >= 0 && tile.owner == owner && tile.mortgaged == 0 && tile.houses > 0",
        Some(can_counteract),
        counteract).declares_negation().counter_reason(key!("hey_kids_when"))],
)
.props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"狂乱Hey Kids!!"#)
        .raw_name(r#"狂乱Hey Kids!!"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 在属于你的格子上结算时，打出此卡，将本次结算改为：将本格上的房屋转移到属于你的可建造格子上。转移的房屋与目标格子中，至少一方数量为1，且每个目标格子至多获得1层房屋。转移时，消耗的房屋造价等于获得的房屋总造价，超出的部分作为现金获得；随后，你失去“转移后各格房屋造价总和－获得房屋数量×500”的资金。"#))
        // sheet cell H4
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", "")]));

/// 「属于你的可建造格子」 -- the same gate the build step uses, so a transfer target cannot
/// be one the engine would refuse to build on.
fn buildable_targets(player_id: i32, from: i32) -> Vec<i32> {
    ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&t| t != from && ctx::can_build_on(player_id, t))
        .collect()
}

/// 规则书[反击]: 「在属于你的格子上结算时，打出此卡」
///
/// **User ruling (2026-10-06):** this fires when **another** player settles
/// rent on *my* tiles -- 「在属于你的格子上结算时」 is someone else's settle on
/// my square, not my own. A non-rent payment (a card's [支付] that is not a
/// tile [结算]) does not open the window.
fn can_counteract(player_id: i32) -> bool {
    // The settler / tile shape is the condition; there must be a buildable target to
    // transfer the houses onto (`can_build_on` is an engine predicate).
    !buildable_targets(player_id, trigger::tile()).is_empty()
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // Cancelling the settle is what makes the transfer *replace* it rather than
    // happen on top of it.
    trigger::set_cancelled();
    let from = trigger::tile();
    if from < 0 {
        return Ok(());
    }
    let targets = buildable_targets(player_id, from);
    if targets.is_empty() {
        return Ok(());
    }
    let houses = ctx::houses_of(from);
    // `max` only bounds the offer; a re-run sees the source already drained
    // and so a shorter list. `ask_number` answers `min + index` off the logged
    // index rather than clamping to the current list, so the pick survives.
    let max = houses.min(targets.len() as i32).max(1);
    let k = ctx::ask_number(
        player_id,
        &Msg::new(key!("hey_kids_title")),
        &Msg::new(key!("hey_kids_ask_count")).tile("tile", from),
        1,
        max,
    )?;
    // 规则书: 「转移的房屋与目标格子中，至少一方数量为1」 -- every target takes one
    // house and the moved count equals the target count, so the two sides are
    // always equal and only `k == 1` satisfies the clause.
    let k = k.max(1);
    // Pick `k` distinct targets (one house each). `ask_tiles` answers the tile
    // id, so a re-run resolves the pick even though `buildable_targets` has
    // changed under the houses this body already moved.
    let mut picked: Vec<i32> = Vec::new();
    for n in 0..k {
        let left: Vec<i32> = targets
            .iter()
            .copied()
            .filter(|t| !picked.contains(t))
            .collect();
        let mut labels: Vec<Msg> = Vec::with_capacity(left.len());
        let mut prices: Vec<i32> = Vec::with_capacity(left.len());
        for &t in &left {
            labels.push(Msg::new("ask.tileOption").tile("tile", t));
            prices.push(0);
        }
        let t = ctx::ask_tiles(
            player_id,
            &Msg::new(key!("hey_kids_title")),
            &Msg::new(key!("hey_kids_ask_target"))
                .i("n", (n + 1) as i64)
                .i("k", k as i64),
            &left,
            &labels,
            &prices,
            -1,
        )?;
        if t < 0 {
            break;
        }
        picked.push(t);
    }
    // Transfer: one house per picked target, source decrement capped at zero.
    // The loop runs the whole `picked` list on every pass: each pair of
    // `add_house` calls owns one answer slot, so an already-applied pair
    // replays as a no-op and a fresh one applies -- breaking out on the
    // (already drained) source count would skip the rest.
    let mut cost_out = 0i32; // source build cost consumed
    let mut cost_in = 0i32; // target build cost gained
    let moved = picked.len() as i32;
    for &t in &picked {
        ctx::add_house(from, -1);
        cost_out += ctx::build_cost(from);
        ctx::add_house(t, 1);
        cost_in += ctx::build_cost(t);
    }
    if moved == 0 {
        return Ok(());
    }
    ctx::log(
        player_id,
        &Msg::new(key!("hey_kids_moved"))
            .tile("tile", from)
            .i("count", moved as i64),
    );
    if cost_out > cost_in {
        ctx::gain(
            player_id,
            cost_out - cost_in,
            &Msg::new(key!("hey_kids_gain_diff")),
        )?;
    }
    let fee = cost_in - moved * 500;
    if fee > 0 {
        ctx::pay(player_id, fee, &Msg::new(key!("hey_kids_lose_fee")))?;
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

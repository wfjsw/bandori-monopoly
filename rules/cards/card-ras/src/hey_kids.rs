//! `RAS:狂乱Hey Kids!!` -- C# `CardHeyKids` (MatchHost.cs:9368-9440): [反击]
//! replace the settle with a house transfer off the settled tile.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:狂乱Hey Kids!!`）:
//! > 狂乱Hey Kids!!：
//! > [反击] 在属于你的格子上结算时，打出此卡，将本次结算改为：将本格上的房屋转移到属于你的可建造格子上。转移的房屋与目标格子中，至少一方数量为1，且每个目标格子至多获得1层房屋。转移时，消耗的房屋造价等于获得的房屋总造价，超出的部分作为现金获得；随后，你失去“转移后各格房屋造价总和－获得房屋数量×500”的资金。
//!

use alloc::vec::Vec;

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const HEY_KIDS: CardDef = CardDef::new("RAS:狂乱Hey Kids!!", &[
    On::CounterAct(&[ChainKind::Settle], can_react, react),
]);

/// C# `Targets(player_id, from)` -- owned tiles (≠ `from`) that `WhyNotBuildOn`
/// would allow. `WhyNotBuildOn` (kind ring, rent capacity, house cap, NoBuild
/// event, Fx.CanBuild, …) is still missing; the filter approximates with the
/// checks now in the vocabulary.
// TODO(规则书): `H.WhyNotBuildOn` for the full buildable-target filter (kind
//   != "ring", rent-array capacity, house cap, `H._turnCtx.NoBuild`, event
//   "卡池BUG", `Fx.CanBuild`) is still missing; only buyable / mortgaged /
//   can-pay are checked here.
/// 「属于你的可建造格子」 -- the same gate the build step uses (`WhyNotBuildOn`),
/// so a transfer target cannot be one the engine would refuse to build on.
fn buildable_targets(player_id: i32, from: i32) -> Vec<i32> {
    ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&t| t != from && ctx::can_build_on(player_id, t))
        .collect()
}

/// 规则书[反击]: 「在属于你的格子上结算时，打出此卡」
fn can_react(player_id: i32) -> bool {
    if trigger::kind() != TriggerKind::Settle {
        return false;
    }
    if trigger::player_id() != player_id {
        return false;
    }
    let t = trigger::tile();
    if t < 0 || ctx::tile_owner(t) != player_id {
        return false;
    }
    // 规则书[反击]: 「将本格上的房屋转移到…」 -- the tile must have houses to
    // transfer (C# `H.State.houses[t.Tile] > 0`).
    if ctx::houses_of(t) <= 0 {
        return false;
    }
    !buildable_targets(player_id, t).is_empty()
}

fn react(player_id: i32) {
    // 规则书[反击]: 「将本次结算改为：将本格上的房屋转移到属于你的可建造格子上」
    // -- C# `c.Trigger.Cancelled = true` then `H.AskNumber` + `H.AskTileOf` +
    // `H.AddHouse` per target. Cancelling the settle is what makes the transfer
    // *replace* it rather than happen on top of it.
    trigger::set_cancelled();
    let from = trigger::tile();
    if from < 0 {
        return;
    }
    let targets = buildable_targets(player_id, from);
    if targets.is_empty() {
        return;
    }
    // 规则书[反击]: 「转移的房屋与目标格子中，至少一方数量为1，且每个目标格子至多获得1层房屋」
    // -- the C# moves one house per picked target in a loop, so each target
    // gains at most 1; `H.AskNumber` clamps `k` to `min(houses[from],
    // targets.Count)`.
    let houses = ctx::houses_of(from);
    let max = houses.min(targets.len() as i32);
    if max < 1 {
        return;
    }
    let k = ctx::ask_number(
        player_id,
        &Msg::new(key!("hey_kids_title")),
        &Msg::new(key!("hey_kids_ask_count")).tile("tile", from),
        1,
        max,
    );
    let k = k.clamp(1, max);
    // Pick `k` distinct targets (one house each).
    let mut picked: Vec<i32> = Vec::new();
    for n in 0..k {
        let left: Vec<i32> = targets
            .iter()
            .copied()
            .filter(|t| !picked.contains(t))
            .collect();
        if left.is_empty() {
            break;
        }
        let t = ctx::ask_tile(
            player_id,
            &Msg::new(key!("hey_kids_title")),
            &Msg::new(key!("hey_kids_ask_target")).i("n", (n + 1) as i64).i("k", k as i64),
            &left,
        );
        picked.push(t);
    }
    // Transfer: one house per picked target, source decrement capped at zero.
    let mut cost_out = 0i32; // source build cost consumed
    let mut cost_in = 0i32; // target build cost gained
    let mut moved = 0i32;
    for &t in &picked {
        if ctx::houses_of(from) <= 0 {
            break;
        }
        // 规则书[反击]: 「转移的房屋与目标格子中，至少一方数量为1，且每个目标格子至多获得1层房屋」
        // -- `add_house(t, 1)` clamps at the C# max, matching `H.AddHouse`.
        ctx::add_house(from, -1);
        cost_out += ctx::build_cost(from);
        ctx::add_house(t, 1);
        cost_in += ctx::build_cost(t);
        moved += 1;
    }
    if moved == 0 {
        return;
    }
    ctx::log(
        player_id,
        &Msg::new(key!("hey_kids_moved")).tile("tile", from).i("count", moved as i64),
    );
    // 规则书[反击]: 「转移时，消耗的房屋造价等于获得的房屋总造价，超出的部分作为现金获得」
    // -- C# `H.GainR(i, num - gained, ...)` when the consumed house build cost
    // exceeds the total build cost of the houses placed on the targets.
    if cost_out > cost_in {
        ctx::gain(player_id, cost_out - cost_in, &Msg::new(key!("hey_kids_gain_diff")));
    }
    // 规则书[反击]: 「随后，你失去"转移后各格房屋造价总和－获得房屋数量×500"的资金」
    // -- C# `H.LoseR(i, gained - picked.Count * 500, ...)` when positive.
    let fee = cost_in - moved * 500;
    if fee > 0 {
        ctx::pay(player_id, fee, &Msg::new(key!("hey_kids_lose_fee")));
    }
}

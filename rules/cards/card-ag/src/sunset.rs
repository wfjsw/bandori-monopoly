//! `AG:即使夕阳落山` -- C# `CardSunset` (MatchHost.cs:1014-1094):
//! strip a house from four built deeds and make everyone else pay X.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:即使夕阳落山`）:
//! > 即使夕阳落山：
//! > [限]：
//! > 第100轮次开始前此卡的X为原本X的一半。
//! > [手]：
//! > [指定][使用者]拥有的四个有房子的格子和[使用者]以外的所有玩家。将X设为“被[指定]格子的地契价格和建造已有房屋的造价之和”÷“[使用者]以外的[存活]玩家数量”向上取整10。依次进行以下操作：
//! > 1. 删除所有被[指定]格子上的1栋房；
//! > 2. 所有被[指定]的玩家[支付][使用者]X资金。
//!

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const SUNSET: CardDef = CardDef::new("AG:即使夕阳落山", &[
    On::Play(play),
    On::CantPlay(cant_play),
]);

/// Owned tiles with at least one house (C# `CardSunset.Built`).
fn built(player_id: i32) -> Vec<i32> {
    ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&t| ctx::houses_of(t) > 0)
        .collect()
}

/// C# `CardSunset.WhyNot`: needs 4 built deeds, then someone to pay.
fn cant_play(player_id: i32) -> Option<Msg> {
    if built(player_id).len() < 4 {
        return Some(Msg::new(key!("sunset_no_built")));
    }
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("sunset_no_others")));
    }
    None
}

/// `CeilTo(x, 10)` on the rational `num / den` (C# `MatchHost.CeilTo`:
/// `(int)Math.Ceiling(x / unit - 1E-09) * unit`).
fn ceil10(num: i64, den: i64) -> i64 {
    if num <= 0 || den <= 0 {
        return 0;
    }
    let d = 10 * den;
    let q = if num % d == 0 { num / d } else { num / d + 1 };
    q * 10
}

fn play(player_id: i32) {
    // 规则书[手]: 「[指定][使用者]拥有的四个有房子的格子」 -- C# picks 4 of
    // `Built(i)` (owned tiles with `houses[t] > 0`) one `H.AskTileOf` at a time,
    // or takes them all when fewer than 4 remain.
    let mut picked: Vec<i32> = Vec::new();
    let mut pool: Vec<i32> = built(player_id);
    if pool.is_empty() {
        return;
    }
    while picked.len() < 4 && !pool.is_empty() {
        if pool.len() + picked.len() <= 4 {
            picked.extend(pool.iter().copied());
            break;
        }
        let t = ctx::ask_tile(
            player_id,
            &Msg::new(key!("sunset_title")),
            &Msg::new(key!("sunset_pick")).i("n", picked.len() as i64 + 1),
            &pool,
        );
        picked.push(t);
        pool.retain(|&x| x != t);
    }
    // 规则书[手]: 「[使用者]以外的所有玩家」 (`ctx::others` drops out players).
    let targets = ctx::others(player_id);
    if targets.is_empty() {
        return;
    }
    // 规则书[手]: 「将X设为“被[指定]格子的地契价格和建造已有房屋的造价之和”÷“[使用者]以外的[存活]玩家数量”向上取整10」
    // -- C# `picked.Sum(t => H._tiles[t].price + H.State.houses[t] * H._tiles[t].house)`:
    // land price (`ctx::tile_price`) plus `houses_of(t)` times the per-house
    // build cost (`ctx::build_cost` = `H._tiles[t].house`).
    let mut sum = 0i64;
    for &t in &picked {
        sum += ctx::tile_price(t) as i64 + ctx::houses_of(t) as i64 * ctx::build_cost(t) as i64;
    }
    let n = targets.len() as i64;
    let mut x = ceil10(sum, n.max(1));
    // 规则书[限]: 「第100轮次开始前此卡的X为原本X的一半」
    // -- C# `if (H.State.round < 100) x = CeilTo(x / 2.0, 10)`.
    if ctx::round_no() < 100 {
        x = ceil10(x, 2);
        ctx::log(player_id, &Msg::new(key!("sunset_halved")));
    }
    let x = x.clamp(0, i32::MAX as i64) as i32;
    ctx::log(player_id, &Msg::new(key!("sunset_x")).n("x", x as i64));
    // 规则书[手]: 「[指定][使用者]以外的所有玩家」 -- C# `H.TargetAll(c, list, got)`,
    // run before the house removal (the C# order). The gate is the full
    // targeting pipeline (out / exile / ImmuneAll / Untargetable / the `target`
    // [反击] window; no redirect on `TargetAll`) and `got` keeps only the players
    // actually hit.
    let got = ctx::target_all(&targets);
    // 规则书[手] 1.: 「删除所有被[指定]格子上的1栋房」 -- C# `H.State.houses[t]--`
    // for each picked tile with a house (unconditional on the targeting result).
    for &t in &picked {
        if ctx::houses_of(t) > 0 {
            let left = ctx::add_house(t, -1);
            ctx::log(player_id, &Msg::new(key!("sunset_demolish")).tile("tile", t).i("left", left as i64));
        }
    }
    // 规则书[手] 2.: 「所有被[指定]的玩家[支付][使用者]X资金」 -- only the players
    // `H.TargetAll` actually landed on (`got`).
    let why = Msg::new(key!("sunset_why")).n("x", x as i64);
    for t in got {
        ctx::transfer(t, player_id, x, &why);
    }
}
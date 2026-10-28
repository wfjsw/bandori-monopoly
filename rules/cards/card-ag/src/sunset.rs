//! `AG:即使夕阳落山` -- C# `CardSunset` (MatchHost.cs:1014-1094):
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
//! strip a house from four built deeds and make everyone else pay X.

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{para, sec, steps, CardText};

pub const SUNSET: CardDef = CardDef::new("AG:即使夕阳落山", &[
    On::Play("", Some(play_guard), play)
])
// 规则书[手]: 「[指定][使用者]拥有的四个有房子的格子和[使用者]以外的所有玩家」
// -- the play designates the picked tiles and every other player, so
// 网络链接异常 「取消其对目标之一的[指定]」 sees the designations.
.props(&[(card_sdk::abi::prop::DESIGNATES, 1)])
.meta(&card_sdk::meta::CardMeta::new()
        .name(r#"即使夕阳落山"#)
        .raw_name(r#"即使夕阳落山"#)
        .tags(&[r#"手"#, r#"限"#])
        .text(CardText::of(&[
            sec(r#"[限]："#, &[
                para(r#"第100轮次开始前此卡的X为原本X的一半。"#),
            ]),
            sec(r#"[手]："#, &[
                para(r#"[指定][使用者]拥有的四个有房子的格子和[使用者]以外的所有玩家。将X设为“被[指定]格子的地契价格和建造已有房屋的造价之和”÷“[使用者]以外的[存活]玩家数量”向上取整10。依次进行以下操作："#),
                steps(&[
                    r#"删除所有被[指定]格子上的1栋房；"#,
                    r#"所有被[指定]的玩家[支付][使用者]X资金。"#,
                ]),
            ]),
        ]))
        // sheet cell C5
        .fields(&[("group", r#"Afterglow"#), ("owner", "")]));

/// Owned tiles with at least one house.
/// 玩家有房子的格子
fn built(player_id: i32) -> Vec<i32> {
    ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&t| ctx::houses_of(t) > 0)
        .collect()
}

/// 玩家必须拥有至少四个有房子的格子，且必须有其他玩家存在，否则无法使用此卡。
fn play_guard(player_id: i32) -> Option<Msg> {
    // 规则书[手]: 「[指定][使用者]拥有的四个有房子的格子」
    // TODO(规则书)（四个有房子的格子）: the cell names four without saying what
    // happens when fewer exist -- we refuse the play (OPEN-少于四格; C-AG-sunset-02
    // leaves the designated set undefined).
    if built(player_id).len() < 4 {
        return Some(Msg::new(key!("sunset_no_built")));
    }
    // 规则书[手]: 「[使用者]以外的所有玩家」
    // TODO(规则书)（[使用者]以外的所有玩家）: the cell states no living-other
    // play condition -- we refuse the play with none, and the body early-returns
    // too, so step 1's demolition never runs (the X divisor would be 0).
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("sunset_no_others")));
    }
    None
}

/// Ceil to the nearest multiple of 10 for the rational `num / den`. Returns 0 if `num` or `den` is non-positive.
///
/// Stays in `i32`: instead of `ceil(num / (10 * den)) * 10` (whose `10 * den`
/// intermediate can overflow) this derives the answer from the quotient and
/// remainder of `num / den`, and the round-up step saturates.
fn ceil10(num: i32, den: i32) -> i32 {
    if num <= 0 || den <= 0 {
        return 0;
    }
    let q = num / den;
    // `num / den` is already a multiple of 10 exactly when `q % 10 == 0` and `num % den == 0`.
    if q % 10 == 0 && num % den == 0 {
        q
    } else {
        (q / 10).saturating_add(1).saturating_mul(10)
    }
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「[指定][使用者]拥有的四个有房子的格子」 -- C# picks 4 of
    // `Built(i)` (owned tiles with `houses[t] > 0`) one `H.AskTileOf` at a time,
    // or takes the whole remainder in one pass when at most four candidates are
    // left (`pool.len() + picked.len() <= 4`) -- with exactly four built, or later
    // once a refused designation (below) has shrunk the pool.
    let mut picked: Vec<i32> = Vec::new();
    let mut pool: Vec<i32> = built(player_id);
    if pool.is_empty() {
        return Ok(());
    }
    while picked.len() < 4 && !pool.is_empty() {
        if pool.len() + picked.len() <= 4 {
            // Designate the remainder in one pass. A tile a 「有标记时此地块不能
            // 被指定」 mark refuses stays out of the designated set.
            while let Some(t) = pool.pop() {
                if ctx::target_tile(t) {
                    picked.push(t);
                }
            }
            break;
        }
        let t = ctx::ask_tile(
            player_id,
            &Msg::new(key!("sunset_title")),
            &Msg::new(key!("sunset_pick")).i("n", picked.len() as i64 + 1),
            &pool,
        )?;
        pool.retain(|&x| x != t);
        // 规则书[手]: 「[指定]…有房子的格子」 -- the pick is a designation and
        // must pass the targeting gate (`H.TargetTile`); a NO_TARGET-marked
        // tile cannot be designated, so it stays out and the loop asks again.
        if ctx::target_tile(t) {
            picked.push(t);
        }
    }
    // 规则书[手]: 「[使用者]以外的所有玩家」 (`ctx::others` drops out players).
    // TODO(规则书)（[使用者]以外的所有玩家）: with no other player this returns
    // before step 1 as well -- only the zero-other guard keeps it unreachable.
    let targets = ctx::others(player_id);
    if targets.is_empty() {
        return Ok(());
    }
    // 规则书[手]: 「将X设为“被[指定]格子的地契价格和建造已有房屋的造价之和”÷“[使用者]以外的[存活]玩家数量”向上取整10」
    // -- C# `picked.Sum(t => H._tiles[t].price + H.State.houses[t] * H._tiles[t].house)`:
    // land price (`ctx::tile_price`) plus `houses_of(t)` times the per-house
    // build cost (`ctx::build_cost` = `H._tiles[t].house`).
    let mut sum = 0;
    for &t in &picked {
        sum += ctx::tile_price(t) + ctx::houses_of(t) * ctx::build_cost(t);
    }
    let n = targets.len() as i32;
    let mut x = ceil10(sum, n.max(1));
    // 规则书[限]: 「第100轮次开始前此卡的X为原本X的一半」
    // -- C# `if (H.State.round < 100) x = CeilTo(x / 2.0, 10)`.
    if ctx::round_no() < 100 {
        // TODO(规则书)（第100轮次开始前此卡的X为原本X的一半）: the cell states
        // no rounding for the half -- we round the halved X up to 10 (the C#
        // `CeilTo`; still OPEN-取整顺序).
        x = ceil10(x, 2);
        ctx::log(player_id, &Msg::new(key!("sunset_halved")));
    }
    ctx::log(player_id, &Msg::new(key!("sunset_x")).n("x", x as i64));
    // 规则书[手]: 「[使用者]以外的所有玩家」 -- C# `H.TargetAll(c, list, got)`,
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
            ctx::log(
                player_id,
                &Msg::new(key!("sunset_demolish"))
                    .tile("tile", t)
                    .i("left", left as i64),
            );
        }
    }
    // 规则书[手] 2.: 「所有被[指定]的玩家[支付][使用者]X资金」 -- only the players
    // `H.TargetAll` actually landed on (`got`).
    let why = Msg::new(key!("sunset_why")).n("x", x as i64);
    for t in got {
        ctx::transfer(t, player_id, x, &why)?;
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

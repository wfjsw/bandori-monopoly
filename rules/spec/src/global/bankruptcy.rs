//! G-BANKRUPT -- [破产], [存活], cash-out, auction, leaving.
//!
//! Owned by unit 7. Append to [`CLAUSES`]; do not edit other areas.

use spec_kit::{Clause, Step};

/// Bankruptcy clauses.
pub const CLAUSES: &[Clause] = &[G_BANKRUPT_01];

/// A bankrupt seat's buyable deeds become unowned.
///
/// * **ID:** G-BANKRUPT-01
/// * **Source:** `rulebook-doc.md:134`
/// * **Quote:** 「当玩家进入[破产]状态或选择离开游戏（弃权）时进行以下操作：
///   1. 将其控制的所有棋子，角色卡，乐队卡，和手卡移出游戏。所有其正在生效的卡，技能效果停止生效。
///   2. **其拥有的[可购买格子]变成无主状态**并抽取3张其中格子的地契进行[拍卖]（少于3则全部）。」
/// * **Trigger:** an observed input whose event window contains a `bankrupt`
///   event for player `P` (or a leave that cashes out).
/// * **Condition:** `P` owned at least one buyable tile in `pre`.
/// * **Effect:** in `post`, no buyable tile is owned by `P`. (The auction of
///   min(3, n) deeds is unit 7's separate clause; this one only checks the
///   un-own step.)
/// * **Frame:** n/a (read of `owners`).
/// * **Rulings & OPEN:** 「[可购买格子]」 excludes CiRCLE / 咖啡厅 / 江户川 /
///   流星堂 / [地产商] (`rulebook-doc.md:65`); a non-buyable tile's owner is
///   not covered here. Bankruptcy money has **no ledger legs** (the engine
///   cashes mortgages silently and zeroes the payer) -- that is a documented
///   observability gap and is why G-MONEY-01 skips `bankrupt` windows.
/// * **Executable:** [`g_bankrupt_01`].
const G_BANKRUPT_01: Clause = Clause::new(
    "G-BANKRUPT-01",
    "rulebook-doc.md:134",
    "其拥有的[可购买格子]变成无主状态",
    g_bankrupt_01,
);

/// Executable form of [`G_BANKRUPT_01`].
pub fn g_bankrupt_01(step: &Step<'_>) -> Result<(), String> {
    let mut who: Vec<i32> = Vec::new();
    for e in step.events {
        if e.r#type == "bankrupt" && e.player_id >= 0 {
            who.push(e.player_id);
        }
    }
    if who.is_empty() {
        return Ok(());
    }
    let n = step.post.st.owners.len();
    for &p in &who {
        for t in 0..n {
            if step.post.st.owners[t] == p {
                // Only buyable tiles must un-own (CiRCLE / 咖啡厅 / 江户川 /
                // 流星堂 / [地产商] are not -- `TileData::is_buyable`).
                let buyable = step.data.tiles.get(t).map(|d| d.is_buyable()).unwrap_or(true);
                if buyable {
                    return Err(format!(
                        "bankrupt player {p} still owns buyable tile {t}"
                    ));
                }
            }
        }
    }
    Ok(())
}
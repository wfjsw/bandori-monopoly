//! Static evaluation at the horizon (`docs/BOT.md` §3.4): net worth.
//!
//! Net worth = cash + deeds at value + houses at value, normalised by the
//! board's total value so a score is roughly in `[-1, 1]` across game stages.
//! Deliberately simple: no rent-expectation / set-completion terms yet (the
//! rulebook's scoring weights are the next step, once B0's numbers say the
//! rollout cost can afford them).
//!
//! Mortgaged deeds count at half value (what selling them back yields);
//! houses count at their build cost.

use game_core::data::GameData;
use game_core::state::MatchState;

/// Normalised net worth of `seat` in `st`, in roughly `[-1, 1]`.
pub fn net_worth(data: &GameData, st: &MatchState, seat: usize) -> f64 {
    let me = match st.players.get(seat) {
        Some(p) if !p.out() => p,
        // Bankrupt / left: the floor.
        _ => return -1.0,
    };
    let mut worth = me.money as f64;
    for (t, &owner) in st.owners.iter().enumerate() {
        if owner != seat as i32 {
            continue;
        }
        let tile = match data.tiles.get(t) {
            Some(x) => x,
            None => continue,
        };
        let land = if st.mortgaged.get(t).copied().unwrap_or(false) {
            (tile.price / 2) as f64
        } else {
            tile.price as f64
        };
        let houses = st.houses.get(t).copied().unwrap_or(0).max(0) as f64;
        worth += land + houses * (tile.house.max(0) as f64);
    }
    let scale = normaliser(data, st);
    (worth / scale).clamp(-1.0, 1.0)
}

/// Roughly "everything on the table": start money for every seat plus the
/// board's land + full house costs. Constant per data set / player count, so
/// the normaliser is stable across a game.
fn normaliser(data: &GameData, st: &MatchState) -> f64 {
    let n = st.players.len().max(1) as f64;
    let board: f64 = data
        .tiles
        .iter()
        .map(|t| {
            let land = t.price.max(0) as f64;
            let houses = (t.rent.len().saturating_sub(1) as f64) * (t.house.max(0) as f64);
            land + houses
        })
        .sum();
    // `engine::world::START_MONEY` is crate-private; the rulebook's start
    // money is public knowledge (`docs/ENGINE.md`) and stable at 10_000.
    const START_MONEY: f64 = 10_000.0;
    let start = n * START_MONEY;
    (start + board).max(1.0)
}

/// Net worth difference (me minus the field's mean), normalised -- a slightly
/// sharper signal than absolute worth when the field is uneven.
pub fn relative_worth(data: &GameData, st: &MatchState, seat: usize) -> f64 {
    let mine = net_worth(data, st, seat);
    let alive = st.players.iter().filter(|p| !p.out()).count().max(1) as f64;
    let sum: f64 = (0..st.players.len())
        .map(|i| net_worth(data, st, i))
        .sum();
    mine - (sum - mine) / (alive - 1.0).max(1.0)
}

/// The *simulation outcome* of a finished game for `seat`, in `[-1, 1]`:
/// rank 1 → `+1`, last → `-1`, linear in between; a bankrupt seat is always
/// `-1`. Kept separate from [`relative_worth`] so the Monte-Carlo backup
/// carries win/score statistics and the heuristic backup carries the eval
/// (Lanctot et al. CIG 2014, `docs/BOT-RESEARCH` #3).
pub fn terminal_value(st: &MatchState, seat: usize) -> f64 {
    let Some(me) = st.players.get(seat) else {
        return -1.0;
    };
    if me.out() {
        return -1.0;
    }
    let n = st.players.len().max(2) as f64;
    let rank = me.rank.max(1) as f64;
    // rank 1 → +1, rank n → -1.
    2.0 * (n - rank) / (n - 1.0) - 1.0
}
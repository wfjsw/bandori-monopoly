//! `Mujica:会被骗着买水晶的人` -- C# `CardCrystalSwap` (MatchHost.cs:5981-6049):
//! move one miracle crystal from one card to another.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:会被骗着买水晶的人`）:
//! > 会被骗着买水晶的人：
//! >  将场上一张卡上的一个奇迹水晶移动到另一张可以放置奇迹水晶的卡上。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const CRYSTAL_SWAP: CardDef = CardDef {
    id: "Mujica:会被骗着买水晶的人",
    play: Some(crystal_swap),
    can_react: None,
    react: None,
    // C# `CardCrystalSwap.WhyNot` refuses without two crystal slots (one of
    // them holding a crystal) -- 「场上没有能移动的奇迹水晶」. TODO(规则书):
    // the `why_not` gate once the placed-card inventory + per-card `Crystals`
    // hooks below are in the ABI (the gate is the same `Slots()` query the
    // effect needs).
    why_not: None,
};

fn crystal_swap(seat: i32) {
    // 规则书: 「将场上一张卡上的一个奇迹水晶移动到另一张可以放置奇迹水晶的卡上」
    // C# `CardCrystalSwap.Slots` enumerates every placed card whose text
    // mentions 奇迹水晶 (plus each seat's band card). The ABI has no placed-card
    // inventory and no per-card crystal field, so the pools are empty.
    // TODO(ABI): a placed-card query (`H._placed`) + per-card `Crystals`
    // get/set (C# `AddCrystals`) so the two ask_pick pools can be built.
    let from: alloc::vec::Vec<i32> = alloc::vec::Vec::new();
    let to: alloc::vec::Vec<i32> = alloc::vec::Vec::new();
    if from.is_empty() || to.is_empty() {
        // C# `c.Effective = false` + a log line when no crystal can move.
        ctx::log(seat, &Msg::new(key!("crystal_swap_none")).seat("who", seat));
        return;
    }
    // The prompts are kept so the shape matches the C# once the hooks land.
    let _src = ctx::ask_pick(
        seat,
        &Msg::new(key!("crystal_swap_title")),
        &Msg::new(key!("crystal_swap_from")),
        &[Msg::new(key!("crystal_swap_placeholder"))],
    );
    let _dst = ctx::ask_pick(
        seat,
        &Msg::new(key!("crystal_swap_title")),
        &Msg::new(key!("crystal_swap_to")),
        &[Msg::new(key!("crystal_swap_placeholder"))],
    );
    // TODO(规则书): the move itself (C# `src.AddCrystals(-1)` +
    // `dst.AddCrystals(1)`) -- needs the crystal get/set hooks above. The
    // band-card side (C# `band.AddCr` / `H.AddBandCrystals`) is partially
    // covered by `add_band_crystals`, but only for the seat's own band card.
}
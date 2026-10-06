//! `Mujica:（初华）我，无畏悲伤` -- C# `CardUikaFearless` (MatchHost.cs:5711-5743):
//! if you settled on a memory tile since CiRCLE, draw 1 or fix this turn's roll.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（初华）我，无畏悲伤`）:
//! > （初华）我，无畏悲伤：
//! >  打出此卡时若自从上一次[经过]CiRCLE后有在任何[回忆地块][触发结算]，你可选择抽1张卡或使你本回合的投掷结果可定义为1-6以内的任何数字
//!

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const UIKA_FEARLESS: CardDef = CardDef::new(
    "Mujica:（初华）我，无畏悲伤",
    &[On::Play(None, uika_fearless)],
);

fn uika_fearless(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「若自从上一次[经过]CiRCLE后有在任何[回忆地块][触发结算]」 -- C#
    // `H.V(i, "uikaMemory")` is bumped on every memory-tile settle and cleared
    // when CiRCLE is passed. The Ave Mujica 三角初华 skill (`Imprisoned XII`)
    // already watches exactly that and publishes it under a neutral key, so
    // this card reads `memory.dirty` rather than carrying its own hooks (this
    // card is a hand card -- its own `pass` / `settle` entries would not run).
    let memory = ctx::state::get(player_id, "memory.dirty");
    if memory <= 0 {
        // C# `c.Effective = false` + a log line; no set_effective hook yet.
        ctx::log(
            player_id,
            &Msg::new(key!("uika_fearless_no_memory")).player_id("who", player_id),
        );
        return Ok(());
    }
    // 规则书: 「你可选择抽1张卡或使你本回合的投掷结果可定义为1-6以内的任何数字」
    let mut options = alloc::vec::Vec::new();
    options.push(Msg::new(key!("uika_fearless_draw")));
    // C# only offers the fix-roll option when `H.MoveWhyNot(i) == null &&
    // H._turnCtx.Plan.FixedRoll < 0`; `ctx::cant_move` is `H.MoveWhyNot` and
    // `ctx::fixed_roll` is `Plan.FixedRoll` (`None` when unset, i.e. `< 0`).
    let can_move = ctx::cant_move(player_id).is_none() && ctx::fixed_roll().is_none();
    if can_move {
        options.push(Msg::new(key!("uika_fearless_fix")));
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("uika_fearless_title")),
        &Msg::new(key!("uika_fearless_ask")),
        &options,
    )?;
    if pick == 1 && can_move {
        // 规则书: 「使你本回合的投掷结果可定义为1-6以内的任何数字」
        // C# `H.AskNumber(..., 1, 6, ...)` then `H._turnCtx.Plan.FixedRoll = n`.
        let n = ctx::ask_number(
            player_id,
            &Msg::new(key!("uika_fearless_fix_title")),
            &Msg::new(key!("uika_fearless_fix_ask")),
            1,
            6,
        )?;
        // C# `H._turnCtx.Plan.FixedRoll = Math.Max(1, rn.value)`.
        let n = n.max(1);
        ctx::set_fixed_roll(n);
        ctx::log(
            player_id,
            &Msg::new(key!("uika_fearless_fixed"))
                .player_id("who", player_id)
                .i("n", n as i64),
        );
    } else {
        // 规则书: 「抽1张卡」
        ctx::draw(player_id, 1);
        ctx::log(
            player_id,
            &Msg::new(key!("uika_fearless_drew")).player_id("who", player_id),
        );
    }
    Ok(())
}

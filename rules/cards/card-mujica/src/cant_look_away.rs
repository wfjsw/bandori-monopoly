//! `Mujica:无法将视线移开` -- C# `CardCantLookAway` (MatchHost.cs:5654-5710):
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:无法将视线移开`）:
//! > 无法将视线移开：
//! >  使当前回合内对你打出过[反击]的所有玩家向你选择的方向强制移动1~4以内的任意步数并[触发结算] （此卡可作为[反击]在有玩家对你使用[反击]后立即使用），若使用者在自身回合内选择了使用者自己进行强制移动，则视为其主要移动
//!
//! force-move the player who [反击]'d you this turn 1-4 tiles with settle.

use card_sdk::abi::ChainKind;
use bandori_std::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const CANT_LOOK_AWAY: CardDef = CardDef::new(
    "Mujica:无法将视线移开",
    &[
        On::Play("", None, None, play),
        // 「此卡可作为[反击]在有玩家对你使用[反击]后立即使用」 -- the timing is
        // a counter used *against you*. The engine's counter link carries
        // `target` = the countered seat and `actor` = the counter's declarer
        // (see `build_round`), so `target == owner && actor != owner` is exactly
        // 「有玩家对你使用[反击]」. `Counteracted` is the named wire kind; the
        // round on a counter also presents it as the counter's own `card` /
        // `skillUsed` kind (a counter to a counter is answered there), so both
        // selectors are declared.
        On::Counteract(
            &[ChainKind::Counteracted, ChainKind::Card, card_sdk::abi::ActionKind::CardUsed.as_trigger()],
            "target == owner && actor != owner", Some(key!("cant_look_away_when")),
            None,
            counteract,
        ),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"无法将视线移开"#)
        .raw_name(r#"无法将视线移开"#)
        .tags(&[r#"反击"#])
        .text(r#"使当前回合内对你打出过[反击]的所有玩家向你选择的方向强制移动1~4以内的任意步数并[触发结算] （此卡可作为[反击]在有玩家对你使用[反击]后立即使用），若使用者在自身回合内选择了使用者自己进行强制移动，则视为其主要移动"#)
        // sheet cell J7
        .fields(&[("group", r#"Ave Mujica"#), ("owner", "")]));

fn play(player_id: i32) -> card_sdk::Asked {
    run(player_id)?;
    Ok(())
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    run(player_id)?;
    Ok(())
}

fn run(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「使当前回合内对你打出过[反击]的所有玩家」 -- C#
    // `H._counteractedAgainst` filtered by `target == player_id && turn == H.TurnKey`.
    //
    // TODO(规则书): 「使当前回合内对你打出过[反击]的所有玩家…」 needs a
    //   per-turn counteraction history (who countered whom). The engine has
    //   none: `ctx::turn_key()` only names the turn, so the full list cannot
    //   be built (BACKLOG ABI-03). Today this body moves only the immediate
    //   counterer (`trigger::player_id()` of the link this counter answers),
    //   which is less than the book. A fix needs: (1) an SDK query for the
    //   per-turn counteraction history (declarer × countered seat), (2) cond
    //   vocab to filter it, (3) an append at the counter-declaration loop
    //   (`wasm_rules.rs` ~5480, where `link.target` = the countered seat and
    //   `actor`/`player_id` = the declarer), (4) an ABI bump.
    let counter_user = trigger::player_id();
    if counter_user < 0 || counter_user == player_id {
        return Ok(());
    }
    if ctx::player_out(counter_user) {
        return Ok(());
    }
    // 规则书: 「向你选择的方向强制移动1~4以内的任意步数」
    // C# `H.AskNumber(i, ..., 1, 4, ...)` (an AskPick over the range).
    let n = ctx::ask_number(
        player_id,
        &Msg::new(key!("cant_look_away_title")),
        &Msg::new(key!("cant_look_away_steps")).player_id("who", counter_user),
        1,
        4,
    )?;
    let forward = ctx::ask_pick(
        player_id,
        &Msg::new(key!("cant_look_away_dir_title")),
        &Msg::new(key!("cant_look_away_dir_ask")),
        &[
            Msg::new(key!("cant_look_away_forward")),
            Msg::new(key!("cant_look_away_backward")),
        ],
    )? == 0;
    // 规则书: 「强制移动1~4以内的任意步数并[触发结算]」 -- C# `H.ForceWalk(p,
    // forward ? n : -n, resolve: true, ...)` (MatchHost.cs:5708) builds
    // `MoveCtx { Steps = n, Reverse = !forward, Resolve = true,
    // Forced = true }`. The MoveCtx shape maps onto `ctx::plan::*`.
    ctx::plan::set_steps(n);
    ctx::plan::set_reverse(!forward);
    ctx::plan::set_resolve(true);
    ctx::log(
        player_id,
        &Msg::new(key!("cant_look_away_ordered"))
            .player_id("who", counter_user)
            .i("n", n as i64),
    );
    // 规则书: 「强制移动1~4以内的任意步数并[触发结算]」 -- run the shaped walk
    // now (C# `H.ForceWalk`, MatchHost.cs:23453-23480, through the move
    // plan).
    ctx::card_move(counter_user);
    // 规则书: 「若使用者在自身回合内选择了使用者自己进行强制移动，则视为其主要移动」
    // -- `card_move` runs `MainMoveAs` (MatchHost.cs:23102-23120), which sets
    // `_turnCtx.MainMoved` when the walked player is the turn player, so a
    // self walk on one's own turn consumes the main move.
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

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}

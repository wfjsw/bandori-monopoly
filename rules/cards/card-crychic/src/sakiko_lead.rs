//! `CRYCHIC:（祥子）带领着大家` -- C# `CardSakikoLead` (MatchHost.cs:3332-3353):
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:（祥子）带领着大家`）:
//! > （祥子）带领着大家：
//! >  回合开始时若你与其他玩家重合，可打出此卡并记录那些玩家，使你的下次主要移动结果对那些玩家一起执行，你先触发结算，此后其他玩家按行动顺序依次触发结算；触发结算时进行的支付价格减半。
//!
//! record the players sharing your tile; they follow your next main move.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const SAKIKO_LEAD: CardDef = CardDef::new(
    "CRYCHIC:（祥子）带领着大家",
    &[On::Play(Some(cant_play), sakiko_lead, "")],
);

fn cant_play(player_id: i32) -> Option<Msg> {
    // C# `CardSakikoLead.WhyNot`: refuses with 「没有和你重合的玩家」 when no
    // other player shares this player's tile, then defers to `H.MoveWhyNot`.
    let pos = ctx::player_pos(player_id);
    if pos < 0 || ctx::players_on(pos, player_id).is_empty() {
        return Some(Msg::new(key!("sakiko_lead_no_shared")));
    }
    // 规则书: 「你的下次主要移动」 -- the lead rides the turn's main move, so the
    // C# then defers to `H.MoveWhyNot` (own turn, main move still available,
    // turn's move not skipped).
    ctx::cant_move(player_id)
}

fn sakiko_lead(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「回合开始时若你与其他玩家重合，可打出此卡并记录那些玩家」 -- C#
    // `H.SeatsOn(H.State.seats[seat].pos, seat)` (other players still in the game on
    // this tile).
    let pos = ctx::player_pos(player_id);
    let mut shared = ctx::players_on(pos, player_id);
    // 规则书: 「并记录那些玩家」 -- the log is the visible record (C# keeps the
    // list on `LeadFx.Who`).
    //
    // 规则书: 「你先触发结算，此后其他玩家按行动顺序依次触发结算」 -- the list is
    // recorded in **action order** starting from the seat after the user, which
    // is the order `plan::add_follower` replays them in.
    let n = ctx::player_count().max(1);
    shared.sort_by_key(|&w| (w - player_id).rem_euclid(n));
    for &who in &shared {
        ctx::log(
            player_id,
            &Msg::new(key!("sakiko_lead_rec"))
                .player_id("who", player_id)
                .player_id("other", who),
        );
        // 规则书: 「使你的下次主要移动结果对那些玩家一起执行」 -- C# `LeadFx.Who`
        // + `Follow`: after the user settles, the engine replays this move's
        // result for each follower (`ctx::plan::add_follower`).
        ctx::plan::add_follower(who);
    }
    // 规则书: 「触发结算时进行的支付价格减半」 -- C# `LeadFx.Mark` on the player's
    // next main move (`m.PayFactor *= 0.5`). Milli-units: 500 = x0.5. Written on
    // the turn's move plan here at 「回合开始时」 (before the dice), the same
    // `MoveCtx` the C# marks at `MoveBefore`; with the default factor of 1.0
    // `*= 0.5` and `= 0.5` agree. The followers' replays carry the same plan,
    // so their settle payments are halved too.
    ctx::plan::set_pay_factor(500);
    Ok(())
}

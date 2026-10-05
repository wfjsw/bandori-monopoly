//! `CRYCHIC:（祥子）带领着大家` -- C# `CardSakikoLead` (MatchHost.cs:3332-3353):
//! record the players sharing your tile; they follow your next main move.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:（祥子）带领着大家`）:
//! > （祥子）带领着大家：
//! >  回合开始时若你与其他玩家重合，可打出此卡并记录那些玩家，使你的下次主要移动结果对那些玩家一起执行，你先触发结算，此后其他玩家按行动顺序依次触发结算；触发结算时进行的支付价格减半。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const SAKIKO_LEAD: CardDef = CardDef {
    id: "CRYCHIC:（祥子）带领着大家",
    play: Some(sakiko_lead),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // C# `CardSakikoLead.WhyNot`: refuses with 「没有和你重合的玩家」 when no
    // other seat shares this seat's tile, then defers to `H.MoveWhyNot`.
    let pos = ctx::seat_pos(seat);
    if pos < 0 || ctx::seats_on(pos, seat).is_empty() {
        return Some(Msg::new(key!("sakiko_lead_no_shared")));
    }
    // TODO(规则书): the C# then defers to `H.MoveWhyNot` (only before the turn's
    //   main move) -- that hook is still missing, so this gate is over-permissive
    //   on the move window.
    None
}

fn sakiko_lead(seat: i32) {
    // 规则书: 「回合开始时若你与其他玩家重合，可打出此卡并记录那些玩家」 -- C#
    // `H.SeatsOn(H.State.seats[seat].pos, seat)` (other seats still in the game on
    // this tile).
    let pos = ctx::seat_pos(seat);
    let shared = ctx::seats_on(pos, seat);
    // 规则书: 「并记录那些玩家」 -- the log is the visible record (C# keeps the
    // list on `LeadFx.Who`).
    for &who in &shared {
        ctx::log(seat, &Msg::new(key!("sakiko_lead_rec")).seat("who", seat).seat("other", who));
    }
    // TODO(ABI): 「使你的下次主要移动结果对那些玩家一起执行，你先触发结算，此后其他玩家按
    //   行动顺序依次触发结算」 -- needs the Fx.MoveBefore / Fx.SettleAfter
    //   persistent hooks (C# `LeadFx.Mark` on the seat's next main move, then
    //   `LeadFx.Follow` walks each recorded player the same way with
    //   `H.ForceWalk(..., settle, payFactor: 0.5)` / `H.ForceTeleport` +
    //   `H.SettleAt`), and a place to keep the recorded seat list
    //   (`LeadFx.Who`).
    // TODO(ABI): 「触发结算时进行的支付价格减半」 -- needs the move `PayFactor`
    //   field (C# `LeadFx.Mark`: `m.PayFactor *= 0.5`) so settles on that move
    //   pay half.
}

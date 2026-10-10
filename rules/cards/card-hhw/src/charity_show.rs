//! `HHW:爱心义演` -- C# `CardCharityShow` (MatchHost.cs:4164-4177): this turn,
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:爱心义演`）:
//! > 爱心义演：
//! > 打出此卡的回合内，你若进行掷骰移动，每初次经过一个属于你的格子，使你的总移动数+2，本回合中向其他玩家支付时你的付款减半（向上取整10）（若弦卷心已将专属卡置于CiRCLE上，则CiRCLE也算作属于弦卷心的格子）
//!
//! +2 steps the first time you pass each of your tiles, and half pay to others.
//! C# arms `H._turnCtx.HalfPayToOthers` and `H.ExtraOf<CharityFx>(seat)` (a
//! player attachment, not a placed card). The hook surface only dispatches to
//! *placed* cards, so the play body places this card as the `CharityFx`
//! stand-in and files it to the discard pile at the owner's turn end (C#
//! `CharityFx.TurnEndAfter` -> `H.RemoveExtra(this)`).

use card_sdk::abi::{HookKind, MoveKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "HHW:爱心义演";
/// C# `CardKokoroCircle` -- the card that makes CiRCLE count as yours (rule:
/// 「若弦卷心已将专属卡置于CiRCLE上，则CiRCLE也算作属于弦卷心的格子」).
const KOKORO_ID: &str = "HHW:（kkr）前往笑容集结的地方！";

/// C# `H._turnCtx.HalfPayToOthers` / `CharityFx.Turn` -- the turn key the
/// half-pay is armed for (expires with the turn). Stored on the card instance
/// (`FieldCard::props`), not in the player's keyed-state map, so the write
/// stays inside this rule's scope.
const PROP_TURN: &str = "charity.turn";
/// Running count of the walk's `+2` steps this turn (instance prop).
const PROP_EXTRA: &str = "charity.extra";
/// Per-tile first-pass marker (instance prop `charity.seen.<tile>`).

pub const CHARITY_SHOW: CardDef = CardDef::new(
    "HHW:爱心义演",
    &[
        On::Play("", None, play),
        // `card.placed && actor == owner` is uniform across the three kinds;
        // per-kind clauses (the turn-key latch, the pay target, the walk shape,
        // the first-pass marker) stay in the arms below.
        On::Hook(
            &[HookKind::PayMul, HookKind::TurnEndAfter, HookKind::PassTile],
            "card.placed && actor == owner",
            None,
            hook,
        ),
    ],
);

fn play(player_id: i32) -> card_sdk::Asked {
    // C# `CardCharityShow.Play` arms the two turn-long effects and logs.
    // 规则书: 「打出此卡的回合内」 -- C# `H._turnCtx.HalfPayToOthers = true` and
    // `CharityFx.Turn = H.TurnKey` live on the turn, not on the card.
    // 规则书: 「打出此卡的回合内」 -- C# `H.ExtraOf<CharityFx>(seat)`. The hook
    // dispatch only runs on placed cards, so this placement stands in for the
    // player attachment (same pattern as `card-sumimi`'s 儿时玩伴的鼓励); the
    // `TurnEndAfter` hook below files it away at the owner's turn end.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("charity_show_note")));
    // Instance props (not player state): scoped to this placed card.
    ctx::set_prop(PROP_TURN, ctx::turn_key());
    ctx::set_prop(PROP_EXTRA, 0);
    ctx::log(
        player_id,
        &Msg::new(key!("charity_show_played")).player_id("who", player_id),
    );
    Ok(())
}

/// C# `CharityFx.Mine` (MatchHost.cs:4200-4210) -- is tile `t` "yours" for the
/// pass bonus? Your own land always is; CiRCLE counts when your
/// `CardKokoroCircle` sits on it (rule: 「若弦卷心已将专属卡置于CiRCLE上，则
/// CiRCLE也算作属于弦卷心的格子」).
fn mine(player_id: i32, t: i32) -> bool {
    // 规则书: 「属于你的格子」 -- C# `H.State.owners[t] != Seat` falls through to
    // the CiRCLE probe; owned land is yours.
    if ctx::tile_owner(t) == player_id {
        return true;
    }
    // 规则书: 「（若弦卷心已将专属卡置于CiRCLE上，则CiRCLE也算作属于弦卷心的格子）」
    // -- C# `H._placed.Any(p => p is CardKokoroCircle && p.Seat == Player && p.Tile == t)`.
    // `ctx::placed_tile` answers "is this id in play on that player, and on which
    // tile". `place_card` does not bind a tile yet (`Some(-1)`), so accept that
    // as "on CiRCLE" -- the only circle tile the rule names.
    if ctx::is_circle(t) {
        return match ctx::find_card(player_id, KOKORO_ID).and_then(ctx::tile_at) {
            Some(pt) => pt == t || pt < 0,
            None => false,
        };
    }
    false
}

/// C# `CharityFx.PassTile` / `CharityFx.PayMul` / `CharityFx.TurnEndAfter`
/// (MatchHost.cs:4211-4231, `H._turnCtx.HalfPayToOthers` applied in `SettleFactor`).
fn hook(player_id: i32) -> card_sdk::Asked {
    // `card.placed && actor == owner` is the pre.
    match trigger::kind() {
        // 规则书: 「本回合中向其他玩家支付时你的付款减半（向上取整10）」 -- C#
        // `SettleFactor`: `p.amount = CeilTo(p.amount / 2.0, 10)` for any
        // pay-to-other of the armed player. The `PayMul` pass (after `PayAdd`,
        // before `PayChoose`) rewrites the amount via `set_pay_amount`.
        TriggerKind::PayMul => {
            if ctx::prop(PROP_TURN) != ctx::turn_key() {
                return Ok(());
            }
            // C# `HalfPayToOthers` only halves payments *to other players*
            // (`p.to >= 0 && p.to != Player`); a bank payment is untouched.
            // (`actor == owner` is the pre.)
            let to = trigger::target();
            if to < 0 || to == player_id {
                return Ok(());
            }
            let amount = trigger::value();
            if amount <= 0 {
                return Ok(());
            }
            // `CeilTo(amount / 2.0, 10)` -- half, rounded up to a multiple of 10.
            let half = (amount as i64 + 1) / 2;
            let half = ((half + 9) / 10) * 10;
            let half = half.clamp(0, i32::MAX as i64) as i32;
            trigger::set_pay_amount(half);
            ctx::log(
                player_id,
                &Msg::new(key!("charity_show_pay"))
                    .player_id("who", player_id)
                    .n("from", amount as i64)
                    .n("to", half as i64),
            );
        }
        // 规则书: 「你若进行掷骰移动，每初次经过一个属于你的格子，使你的总移动数+2」
        // -- C# `CharityFx.PassTile` (MatchHost.cs:4211-4222).
        TriggerKind::PassTile => {
            if ctx::prop(PROP_TURN) != ctx::turn_key() {
                return Ok(());
            }
            // C# `m.Seat != Seat` -- `actor == owner` is the pre.
            // C# `m.Teleport || m.TeleportWalk` -- not a teleport.
            if trigger::move_kind() == Some(MoveKind::Teleport) {
                return Ok(());
            }
            // C# `m.Steps >= 0` (not a dice move) -- the rule scopes this to
            // 「掷骰移动」. `move_is_main()` is the turn's main roll-and-move.
            if !trigger::move_is_main() {
                return Ok(());
            }
            let t = trigger::tile();
            if t < 0 || !mine(player_id, t) {
                return Ok(());
            }
            // C# `Seen.Add(t)` -- only the first pass of each tile counts
            // (the prop holds the turn key that first saw it).
            let key = alloc::format!("charity.seen.{t}");
            if ctx::prop(&key) == ctx::turn_key() {
                return Ok(());
            }
            ctx::set_prop(&key, ctx::turn_key());
            ctx::log(
                player_id,
                &Msg::new(key!("charity_show_pass"))
                    .player_id("who", player_id)
                    .tile("tile", t),
            );
            // 规则书: 「使你的总移动数+2」 -- C# `m.ExtraSteps += 2` mid-walk.
            // The walk loop bound is `steps + m.ExtraSteps`, re-read each
            // step (play.rs). `set_extra_steps` assigns, so keep a running
            // count on the instance and write the sum.
            let mut extra = ctx::prop(PROP_EXTRA);
            if extra < 0 {
                extra = 0;
            }
            extra += 2;
            ctx::set_prop(PROP_EXTRA, extra);
            ctx::plan::set_extra_steps(extra);
            // TODO(规则书)[judgement]: the walk reads `m.extra_steps` off the in-flight
            //   the clause under-specifies -- see the note above it
            //   `Move` clone, while `set_extra_steps` writes
            //   `TurnCtx::plan.extra_steps` -- the two are not yet synced
            //   mid-walk, so the +2 may not extend an in-flight walk until the
            //   engine mirrors plan writes onto the live move.
        }
        // 规则书: 「打出此卡的回合内」 -- C# `CharityFx.TurnEndAfter` (`turn ==
        // Player` -> `H.RemoveExtra(this)`): both effects end with the turn.
        TriggerKind::TurnEndAfter => {
            ctx::set_prop(PROP_TURN, 0);
            ctx::set_dest(ctx::Dest::Graveyard);
        }
        _ => {}
    }
    Ok(())
}

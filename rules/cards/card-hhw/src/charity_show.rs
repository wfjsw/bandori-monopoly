//! `HHW:爱心义演` -- C# `CardCharityShow` (MatchHost.cs:4164-4177): this turn,
//! +2 steps the first time you pass each of your tiles, and half pay to others.
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:爱心义演`）:
//! > 爱心义演：
//! > 打出此卡的回合内，你若进行掷骰移动，每初次经过一个属于你的格子，使你的总移动数+2，本回合中向其他玩家支付时你的付款减半（向上取整10）（若弦卷心已将专属卡置于CiRCLE上，则CiRCLE也算作属于弦卷心的格子）
//!
//! C# arms `H._turnCtx.HalfPayToOthers` and `H.ExtraOf<CharityFx>(seat)` (a
//! player attachment, not a placed card). The hook surface only dispatches to
//! *placed* cards, so the play body places this card as the `CharityFx`
//! stand-in and files it to the discard pile at the owner's turn end (C#
//! `CharityFx.TurnEndAfter` -> `H.RemoveExtra(this)`).

use card_sdk::abi::TriggerKind;
use card_sdk::abi::MoveKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

const ID: &str = "HHW:爱心义演";
/// C# `CardKokoroCircle` -- the card that makes CiRCLE count as yours (rule:
/// 「若弦卷心已将专属卡置于CiRCLE上，则CiRCLE也算作属于弦卷心的格子」).
const KOKORO_ID: &str = "HHW:（kkr）前往笑容集结的地方！";

/// C# `H._turnCtx.HalfPayToOthers` / `CharityFx.Turn` -- the turn key the
/// half-pay is armed for (expires with the turn).
const SLOT_TURN: &str = "charity_show_turn";

pub const CHARITY_SHOW: CardDef = CardDef::new("HHW:爱心义演", &[
    On::Play(play),
    On::Hook(&[TriggerKind::PayMul, TriggerKind::TurnEndAfter, TriggerKind::PassTile], hook),
]);

fn play(player_id: i32) {
    // C# `CardCharityShow.Play` arms the two turn-long effects and logs.
    // 规则书: 「打出此卡的回合内」 -- C# `H._turnCtx.HalfPayToOthers = true` and
    // `CharityFx.Turn = H.TurnKey` live on the turn, not on the card.
    ctx::set_slot(player_id, SLOT_TURN, ctx::turn_key());
    // 规则书: 「打出此卡的回合内」 -- C# `H.ExtraOf<CharityFx>(seat)`. The hook
    // dispatch only runs on placed cards, so this placement stands in for the
    // player attachment (same pattern as `card-sumimi`'s 儿时玩伴的鼓励); the
    // `TurnEndAfter` hook below files it away at the owner's turn end.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("charity_show_note")));
    ctx::log(player_id, &Msg::new(key!("charity_show_played")).player_id("who", player_id));
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
        return match ctx::placed_tile(player_id, KOKORO_ID) {
            Some(pt) => pt == t || pt < 0,
            None => false,
        };
    }
    false
}

/// C# `CharityFx.PassTile` / `CharityFx.PayMul` / `CharityFx.TurnEndAfter`
/// (MatchHost.cs:4211-4231, `H._turnCtx.HalfPayToOthers` applied in `SettleFactor`).
fn hook(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    match trigger::kind() {
        // 规则书: 「本回合中向其他玩家支付时你的付款减半（向上取整10）」 -- C#
        // `SettleFactor`: `p.amount = CeilTo(p.amount / 2.0, 10)` for any
        // pay-to-other of the armed player. The `PayMul` pass (after `PayAdd`,
        // before `PayChoose`) rewrites the amount via `set_pay_amount`.
        TriggerKind::PayMul => {
            if ctx::slot(player_id, SLOT_TURN) != ctx::turn_key() {
                return;
            }
            // C# `HalfPayToOthers` only halves payments *to other players*
            // (`p.to >= 0 && p.to != Player`); a bank payment is untouched.
            let to = trigger::target();
            if to < 0 || to == player_id || trigger::player_id() != player_id {
                return;
            }
            let amount = trigger::value();
            if amount <= 0 {
                return;
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
            if ctx::slot(player_id, SLOT_TURN) != ctx::turn_key() {
                return;
            }
            // C# `m.Seat != Seat` -- only the owner's own walk.
            if trigger::player_id() != player_id {
                return;
            }
            // C# `m.Teleport || m.TeleportWalk` -- not a teleport.
            if trigger::move_kind() == Some(MoveKind::Teleport) {
                return;
            }
            // C# `m.Steps >= 0` (not a dice move) -- the rule scopes this to
            // 「掷骰移动」. `move_is_main()` is the turn's main roll-and-move.
            if !trigger::move_is_main() {
                return;
            }
            let t = trigger::tile();
            if t < 0 || !mine(player_id, t) {
                return;
            }
            // C# `Seen.Add(t)` -- only the first pass of each tile counts
            // (the slot holds the turn key that first saw it).
            let key = format!("charity_seen_{t}");
            if ctx::slot(player_id, &key) == ctx::turn_key() {
                return;
            }
            ctx::set_slot(player_id, &key, ctx::turn_key());
            ctx::log(
                player_id,
                &Msg::new(key!("charity_show_pass")).player_id("who", player_id).tile("tile", t),
            );
            // TODO(规则书): 「使你的总移动数+2」 -- C# `m.ExtraSteps += 2` mid-walk
            // (the walk loop bound is `steps + m.ExtraSteps`, re-read every
            // step). The plan's `set_extra_steps` assigns a pre-roll field the
            // walk loop does not read, so the +2 cannot extend an in-flight
            // walk yet.
        }
        // 规则书: 「打出此卡的回合内」 -- C# `CharityFx.TurnEndAfter` (`turn ==
        // Player` -> `H.RemoveExtra(this)`): both effects end with the turn.
        TriggerKind::TurnEndAfter => {
            if trigger::player_id() != player_id {
                return;
            }
            ctx::set_slot(player_id, SLOT_TURN, 0);
            ctx::unplace_card(player_id);
            ctx::to_discard(player_id, ID);
        }
        _ => {}
    }
}

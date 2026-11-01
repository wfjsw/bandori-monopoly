//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:人偶的箱庭`）:
//! > 人偶的箱庭：
//! >  使场上所有其他玩家选择其一执行：“进行一次移动掷骰并移动对应步数（不[触发结算]）”或“向你支付X*20资金，X为该玩家正常移动到你所在格子所需的移动数。”  无法移动的玩家只能选择向你支付。
//! >  然后，你强制移动其他玩家本次移动掷骰数之和。视为你本回合的主要移动。
//!
//! player either moves without settling or pays you X*20, then you move the sum.

use bandori_std::state_key;
use card_sdk::abi::{roll_source, RollPurpose};
use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const DOLL_GARDEN: CardDef = CardDef::new(
    "Mujica:人偶的箱庭",
    &[On::Play("", Some(cant_play), doll_garden)],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"人偶的箱庭"#)
        .raw_name(r#"人偶的箱庭"#)
        .tags(&[])
        .text(CardText::para(r#"使场上所有其他玩家选择其一执行：“进行一次移动掷骰并移动对应步数（不[触发结算]）”或“向你支付X*20资金，X为该玩家正常移动到你所在格子所需的移动数。” 无法移动的玩家只能选择向你支付。
        然后，你强制移动其他玩家本次移动掷骰数之和。视为你本回合的主要移动。"#))
        // sheet cell J12
        .fields(&[("group", r#"Ave Mujica"#), ("owner", "")]));

fn cant_play(player_id: i32) -> Option<Msg> {
    rules_status::move_blocked(player_id)
}

fn doll_garden(player_id: i32) -> card_sdk::Asked {
    let mut sum = 0;
    for p in ctx::others(player_id) {
        let x = ctx::tile_forward(ctx::player_pos(p), ctx::player_pos(player_id));
        // 规则书: 「无法移动的玩家只能选择向你支付」 -- the generic [无法移动]
        // gate (`stun` counts `stunStart` too: `MatchPlayer::stunned()` =
        // `stun + stunStart > 0`; [不可阻挡] lifts the stay and stun halves).
        let unstoppable = ctx::state::get(p, state_key::UNSTOPPABLE);
        let can_move = !rules_status::stay::blocks_move(rules_status::stay::get(p), unstoppable)
            && !rules_status::stun::blocks_move(
                rules_status::stun::get(p) + ctx::state::get(p, state_key::STUN_START),
                unstoppable,
            )
            && !rules_status::exile::blocks_move(rules_status::exile::get(p));
        if can_move {
            let pick = ctx::ask_choice(
                p,
                &Msg::new(key!("doll_garden_ask_title")),
                &Msg::new(key!("doll_garden_ask_text"))
                    .player_id("user", player_id)
                    .i("x", x as i64)
                    .card("card", "Mujica:人偶的箱庭"),
                &[
                    Msg::new(key!("doll_garden_move")),
                    Msg::new(key!("doll_garden_pay"))
                        .player_id("user", player_id)
                        .n("money", (x * 20) as i64),
                ],
            )?;
            if pick == 1 {
                // 规则书: 「向你支付X*20资金」
                ctx::transfer(p, player_id, x * 20, &Msg::new(key!("doll_garden_reason")))?;
                continue;
            }
        } else {
            // TODO(规则书)（「无法移动的玩家只能选择向你支付」）: a [晕眩]/[除外] victim is also 「无法收付款」 (rules.txt
            // 49/50) -- the pay gate refuses it (`log.blocked_pay`) and they wind up paying
            // 0.
            ctx::transfer(p, player_id, x * 20, &Msg::new(key!("doll_garden_reason")))?;
            continue;
        }
        // 规则书: 「进行一次移动掷骰并移动对应步数（不[触发结算]）」 -- a move
        // roll of the victim's own (non-main, so the walk stays card-driven
        // below); its Plan writes cannot fold outside a move frame and only the
        // face is used.
        let r = ctx::roll_move(p, roll_source::CARD)?.max(0);
        sum += r;
        // The MoveCtx shape maps onto `ctx::plan::*`.
        ctx::plan::set_steps(r);
        ctx::plan::set_settles(false);
        ctx::run_planned_move(p);
        ctx::log(
            player_id,
            &Msg::new(key!("doll_garden_walked"))
                .player_id("who", p)
                .i("n", r as i64),
        );
    }
    if sum <= 0 {
        // 不移动（人偶的箱庭）")`.
        ctx::log(
            player_id,
            &Msg::new(key!("doll_garden_no_move")).player_id("who", player_id),
        );
        // 规则书: 「视为你本回合的主要移动」 -- the sum walk is the main move even
        // when the sum is 0: run a 0-step walk so the main move is consumed and
        // the shared plan stops carrying the last victim's steps / `resolve`.
        ctx::plan::set_steps(0);
        ctx::plan::set_settles(true);
        ctx::run_planned_move(player_id);
        return Ok(());
    }
    // 规则书: 「你强制移动其他玩家本次移动掷骰数之和。视为你本回合的主要移动。」
    ctx::log(
        player_id,
        &Msg::new(key!("doll_garden_sum"))
            .player_id("who", player_id)
            .i("n", sum as i64),
    );
    // TODO(ABI)（强制移动）: TODO(ABI)（fresh MoveCtx）: Steps/Resolve map onto
    // `ctx::plan::*`; `forced` has no setter. no guest `plan::set_forced`, so
    // rules.txt 51's 「可选择…是否生效」 opt-out is unavailable (STA-02). the shared turn
    // plan is never reset -- a stale `stop_at` / `pay_factor` / `reverse` from an
    // earlier walk leaks in.
    ctx::plan::set_steps(sum);
    ctx::plan::set_settles(true);
    ctx::run_planned_move(player_id);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn doll_garden_kani(player_id: i32) -> card_sdk::Asked {
    doll_garden(player_id)
}

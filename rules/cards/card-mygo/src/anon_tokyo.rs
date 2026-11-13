//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:[千早爱音]Anon Tokyo`）:
//! > [千早爱音]Anon Tokyo：
//! > [限]：
//! > 自己所在的格子是[可购买格子]。
//! > [手]：
//! > [指定][使用者]所在当格的任一相邻的[可购买格子]，根据使用者所在格子和被[指定]格子的状态依次进行以下操作：
//! > 1. [使用者]同时拥有上述的两个格子则[消耗]其中地契购买价格中更高者的资金的一半并在上述的两个格子间放置1个[奇迹水晶]（上限1），被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的一半；
//! > 2. [使用者]不同时拥有上述的两个格子则进入移动阶段并将本回合的[主要移动]改为向前或后移动1格到被[指定]格子且[结算]。
//!
//! pick an adjacent buyable tile; if you own both tiles, pay half the dearer
//! deed to link them with a miracle crystal (cap 1), else step one tile over
//! and settle. A linked tile charges half of its partner's rent on top.

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{FilterKind, MarkFilter, TriggerKind};
use card_sdk::{key, CardDef, Filter, Msg, On, PayAddValue};
use card_sdk::{para, sec, steps, CardText};

pub const ANON_TOKYO: CardDef = CardDef::new(
    "MyGO:[千早爱音]Anon Tokyo",
    &[
        On::Play("", Some(cant_play), anon_tokyo),
        On::Filter(Filter::PaySplitAdd("card.placed && pay_is_rent && target == owner && value > 0", Some(link_guard), link_rent)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Anon Tokyo"#)
        .raw_name(r#"[千早爱音]Anon Tokyo"#)
        .tags(&[r#"手"#, r#"限"#])
        .text(CardText::of(&[
            sec(r#"[限]："#, &[
                para(r#"自己所在的格子是[可购买格子]。"#),
            ]),
            sec(r#"[手]："#, &[
                para(r#"[指定][使用者]所在当格的任一相邻的[可购买格子]，根据使用者所在格子和被[指定]格子的状态依次进行以下操作："#),
                steps(&[
                    r#"[使用者]同时拥有上述的两个格子则[消耗]其中地契购买价格中更高者的资金的一半并在上述的两个格子间放置1个[奇迹水晶]（上限1），被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的一半；"#,
                    r#"[使用者]不同时拥有上述的两个格子则进入移动阶段并将本回合的[主要移动]改为向前或后移动1格到被[指定]格子且[结算]。"#,
                ]),
            ]),
        ]))
        // sheet cell I8
        .fields(&[("group", r#"MyGO!!!!!"#), ("owner", r#"千早爱音"#)]));

/// The field stand-in the `PaySplitAdd` rider hangs on (see [`link_rent`]).
const ID: &str = "MyGO:[千早爱音]Anon Tokyo";

fn cant_play(player_id: i32) -> Option<Msg> {
    let n = ctx::tile_count();
    let here = ctx::player_pos(player_id);
    if here < 0 || n <= 0 || !ctx::is_buyable(here) {
        return Some(Msg::new(key!("anon_tokyo_blocked_here")));
    }
    // TODO(规则书)（自己所在的格子是[可购买格子]）:
    if adjacent(here, n).is_empty() {
        return Some(Msg::new(key!("anon_tokyo_blocked_adj")));
    }
    None
}

fn adjacent(here: i32, n: i32) -> Vec<i32> {
    let mut adj: Vec<i32> = Vec::new();
    for d in [1, -1] {
        let t = ((here + d) % n + n) % n;
        if ctx::is_buyable(t) && !adj.contains(&t) {
            adj.push(t);
        }
    }
    adj
}

fn anon_tokyo(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    let here = ctx::player_pos(player_id);
    if here < 0 || n <= 0 || !ctx::is_buyable(here) {
        return Ok(());
    }
    let adj = adjacent(here, n);
    if adj.is_empty() {
        return Ok(());
    }
    let t = ctx::ask_tile(
        player_id,
        &Msg::new(key!("anon_tokyo_title")),
        &Msg::new(key!("anon_tokyo_ask")),
        &adj,
    )?;
    // Fail = the card does nothing more.
    if !ctx::target_tile(t) {
        return Ok(());
    }
    if ctx::tile_owner(here) == player_id && ctx::tile_owner(t) == player_id {
        // TODO(规则书)（[使用者]同时拥有上述的两个格子则[消耗]其中地契购买价格中更高者的资金的一半）: `buy_price`).
        // The 消耗 and the 放置 are one body, but the pair is place-or-refuse
        // atomic at the cap: the charge runs only **after** the pair actually
        // placed, so a capped re-link costs nothing.
        // Place the stand-in **before** the crystal so the two rows bind to its instance
        // and die with it.
        if !ctx::is_placed() {
            ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
            ctx::place_card(player_id, ID, &Msg::new(key!("anon_tokyo_linked")));
        }
        // 规则书[手]1: 「并在上述的两个格子间放置1个[奇迹水晶]（上限1）」 -- the
        // cap is on **the pair** (「放置1个…（上限1）」). `place_mark_pair` is
        // place-or-refuse atomic: if either end already carries a mark of this
        // kind -- whoever linked it -- it places **neither** row (no half-link).
        // TODO(规则书) Class C: whether a tile may sit in two links at once; the
        // singular 「1个…（上限1）」 leans no, and `place_mark_pair`'s either-end
        // refusal is that reading.
        let placed = ctx::place_mark_pair(
            here,
            t,
            key!("anon_tokyo_mark"),
            "",
            player_id,
            ctx::self_uid(),
            1,
            &Msg::new(key!("anon_tokyo_mark_note")).tile("tile", t),
            card_sdk::abi::Stack::Fresh,
        );
        if placed {
            // 规则书[手]1: 「[消耗]其中地契购买价格中更高者的资金的一半」
            let amount = ctx::tile_price(here).max(ctx::tile_price(t)) / 2;
            if amount > 0 {
                ctx::pay(player_id, amount, &Msg::new(key!("anon_tokyo_pay")))?;
            }
            ctx::log(
                player_id,
                &Msg::new(key!("anon_tokyo_linked"))
                    .tile("a", here)
                    .tile("b", t)
                    .card("card", "MyGO:[千早爱音]Anon Tokyo"),
            );
        }
    // TODO(规则书)（[使用者]不同时拥有上述的两个格子则进入移动阶段并将本回合的[主要移动]改为向前或后移动1格到被[指定]格子且[结算]）:
    } else if rules_status::move_blocked(player_id).is_none() {
        ctx::plan::set_steps(1);
        // Forward(here, t) != 1 means the chosen tile is the step behind.
        ctx::plan::set_reverse(ctx::tile_forward(here, ctx::position_of_tile(t)) != 1);
        ctx::log(
            player_id,
            &Msg::new(key!("anon_tokyo_step")).tile("tile", t),
        );
        ctx::run_planned_move(player_id);
    }
    Ok(())
}

/// The rider runs once per player -- from the earliest active stand-in. Marks
/// are matched by kind (they outlive a hand-over), so every copy's stand-in
/// would otherwise add the same crystal's half again. A face-down instance
/// posts no hook, so it does not own the rider.
fn link_guard(player_id: i32) -> bool {
    if !ctx::is_placed() {
        return false;
    }
    let me = ctx::self_uid();
    ctx::field_instances(player_id)
        .into_iter()
        .filter(|(uid, id)| *id == ID && !ctx::is_face_down_at(*uid))
        .map(|(uid, _)| uid)
        .next()
        == Some(me)
}

fn link_rent(player_id: i32, v: PayAddValue) -> PayAddValue {
    let tile = trigger::tile();
    if tile < 0 {
        return v;
    }
    // The settled tile is one end of a link; the partner is the other end
    // (`TileMark.partner`, placed by `place_mark_pair`). Matched by kind only:
    // the crystal sits on the tiles and outlives a hand-over, so the linker is
    // not a stable key (and `target == owner` only narrows it to the owner).
    let other = ctx::mark_partner(tile, &MarkFilter::any().kind(key!("anon_tokyo_mark")));
    if other < 0 {
        return v;
    }
    if ctx::tile_owner(other) != player_id {
        return v;
    }
    // TODO(规则书)（被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的一半）:
    // the figure is the partner's real 收费 (quote: a fresh hypothetical landing
    // by the payer, C-Q13 mortgage and the pay pipeline included); `-1` only
    // when the payer could not settle it at all.
    let quoted = ctx::rent_quote(trigger::player_id(), other);
    if quoted < 0 {
        return v;
    }
    let bonus = quoted / 2;
    if bonus <= 0 {
        return v;
    }
    ctx::log(
        player_id,
        &Msg::new(key!("anon_tokyo_link_rent"))
            .tile("tile", other)
            .i("n", bonus as i64),
    );
    PayAddValue {
        value: v.value + bonus,
        ..v
    }
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn anon_tokyo_kani(player_id: i32) -> card_sdk::Asked {
    anon_tokyo(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn link_rent_kani(player_id: i32, v: card_sdk::PayAddValue) -> card_sdk::PayAddValue {
    link_rent(player_id, v)
}

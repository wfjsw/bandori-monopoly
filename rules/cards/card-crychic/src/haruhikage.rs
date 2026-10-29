//! `CRYCHIC:春日影` -- C# `CardHaruhikage` (MatchHost.cs:2584-2707): [反击] that
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:春日影`）:
//! > 春日影
//! >  ：
//! > （1）[特] 若抽到此卡时你的总资产大于等于20000且手牌不多于5张，可选择使其直接从抽牌堆打出，依次抽牌直至你的手牌数为6，若受到弃牌效果则中断此效果。
//! > （2）（此卡可作为[反击]打出）使用一次Crychic角色的技能
//!
//! runs a CRYCHIC character skill (reroll / step toward / cancel a rent).

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, ChainKind, TriggerKind};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

pub const HARUHIKAGE: CardDef = CardDef::new(
    "CRYCHIC:春日影",
    &[
        On::Play("", None, play),
        On::Counteract(
            &[
                ChainKind::Effect,
                ChainKind::DiceRollResult,
                ChainKind::SettleBefore,
            ],
            // Per-kind applicability as a disjunction. The SettleBefore arms are
            // 丰川祥子's two variants: (3) the owner's own main move with a
            // rival within 5 and a fire pot to spend, (2) another player's
            // settle within 5 of the owner (mover pays 300, steps toward).
            "(kind == DiceRollResult && actor == owner && move.roll != null) \
             || (kind == SettleBefore && actor == owner && move.main && tile.id >= 0 \
                 && others_within(owner, 5) > 0 && charge(owner) >= 1) \
             || (kind == SettleBefore && actor != owner && move.main && tile.id >= 0 \
                 && dist(pos(actor), pos(owner)) <= 5) \
             || (kind == Effect && pay_is_rent && actor != owner \
                 && target == owner && tile.owner == owner && tile.id >= 0 && value > 0)",
            None,
            counteract
        ).counter_reason(key!("haruhikage_when")),
        // 规则书（1）: 「若抽到此卡时你的总资产大于等于20000且手牌不多于5张，可选择…」 -- the
        // offer exists only at that asset *and* hand-size threshold. `assets_of`
        // is a derived list (money + land + houses), residual (docs/GUARDS.md
        // §4.2). `Drawn` fires with the card already in hand (`wasm_rules.rs`
        // 「it is still in hand」), so 「手牌」 counts this card.
        On::Action(Action::Drawn("trigger_card == card.id", Some(assets_ready), on_drawn)),
    ],
)
.props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"春日影"#)
        .raw_name(r#"春日影"#)
        .tags(&[r#"反击"#, r#"特"#])
        .text(CardText::items(&[
            it(r#"[特] 若抽到此卡时你的总资产大于等于20000且手牌不多于5张，可选择使其直接从抽牌堆打出，依次抽牌直至你的手牌数为6，若受到弃牌效果则中断此效果。"#),
            it(r#"（此卡可作为[反击]打出）使用一次Crychic角色的技能"#),
        ]))
        // sheet cell M3
        .fields(&[("group", r#"CRYCHIC"#), ("owner", "")]));

const ID: &str = "CRYCHIC:春日影";

/// `H.Within(seat, 5, includeSame: false)` -- other players within 5 tiles, not sharing yours.
fn within5(player_id: i32) -> Vec<i32> {
    let pos = ctx::player_pos(player_id);
    ctx::others(player_id)
        .into_iter()
        .filter(|&p| {
            let d = ctx::dist(pos, ctx::player_pos(p));
            d > 0 && d <= 5
        })
        .collect()
}

/// C# `H.AssetsOf` -- money + land (mortgaged at half) + houses.
fn assets_of(player_id: i32) -> i32 {
    let mut n = ctx::money_of(player_id);
    for t in ctx::owned_tiles(player_id) {
        n += if ctx::mortgaged_of(t) {
            ctx::tile_price(t) / 2
        } else {
            ctx::tile_price(t)
        };
        n += ctx::houses_of(t) * ctx::build_cost(t);
    }
    n
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- from hand the C# offers the
    // borrowed CRYCHIC skills (`H.CrychicChars` x `H.BorrowedActions`, then
    // `H.RunBorrowed`).
    ctx::log(
        player_id,
        &Msg::new(key!("haruhikage_play")).player_id("who", player_id),
    );
    use_skill(player_id)
}

/// 规则书（2）: 「使用一次Crychic角色的技能」 -- pick one of the CRYCHIC skills and
/// run it under its own id (`ctx::play_card`). Only skills with a live press
/// entry are offered: 椎名立希 / 长崎素世 / 丰川祥子 are timing skills with no
/// `On::Play`, and `play_card` is a no-op for those.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    const CRYCHIC: [&str; 5] = [
        "skill:高松灯（CRYCHIC）:跌跌撞撞...",
        "skill:椎名立希（CRYCHIC）:克服劣等感",
        "skill:长崎素世（CRYCHIC）:雨中祈晴",
        "skill:丰川祥子（CRYCHIC）:你愿意和我组建乐队吗？",
        "skill:若叶睦（CRYCHIC）:精致的人偶",
    ];
    let avail: alloc::vec::Vec<&str> = CRYCHIC
        .iter()
        .copied()
        .filter(|id| ctx::query_guard(player_id, id, ctx::entry::PRESS))
        .collect();
    if avail.is_empty() {
        // The two pressable skills can both be gated off (no fire pot, no
        // latched face); with nothing to run, skip the empty choice prompt.
        return Ok(());
    }
    let options: alloc::vec::Vec<Msg> = avail
        .iter()
        .map(|id| Msg::new(key!("haruhikage_option")).card("card", *id))
        .collect();
    let k = ctx::ask_pick(
        player_id,
        &Msg::new(key!("haruhikage_title")),
        &Msg::new(key!("haruhikage_ask")),
        &options,
    )?;
    if let Some(&id) = avail.get(k) {
        ctx::play_card(id, player_id)?;
    }
    Ok(())
}

/// Residual guard for [`on_drawn`]: 「若抽到此卡时你的总资产大于等于20000且手牌不多于5张」.
/// C# `H.AssetsOf(seat) >= 20000`. Derived scan (money + land + houses),
/// outside the CEL schema (docs/GUARDS.md §4.2). 「手牌不多于5张」 is the hand
/// size at the `Drawn` moment -- the card itself is already in hand.
fn assets_ready(player_id: i32) -> bool {
    assets_of(player_id) >= 20000 && ctx::hand_size(player_id) <= 5
}

/// 规则书（1）[特]: 「若抽到此卡时你的总资产大于等于20000且手牌不多于5张，可选择使其直接从抽牌堆打出，
/// 依次抽牌直至你的手牌数为6，若受到弃牌效果则中断此效果。」 -- C#
/// `CardHaruhikage.Drawn` -> `Special`.
/// Applicability is `assets_ready`; the ask is the 「可选择」 opt-in.
fn on_drawn(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「可选择使其直接从抽牌堆打出」 -- C# `H.AskYes(..., aiYes: true)`.
    let yes = ctx::ask_yes(
        player_id,
        &Msg::new(key!("haruhikage_title")),
        &Msg::new(key!("haruhikage_special_ask")).card("card", ID),
    )?;
    if !yes {
        return Ok(());
    }
    // The card was just drawn, so it is in hand (C# `hand.Remove(Id)`).
    if !ctx::take_card(player_id, ID, ctx::CardPile::Hand) {
        return Ok(());
    }
    ctx::log(
        player_id,
        &Msg::new(key!("haruhikage_special_play"))
            .player_id("who", player_id)
            .card("card", ID),
    );
    // 规则书（1）: 「依次抽牌直至你的手牌数为6，若受到弃牌效果则中断此效果。」 -- the
    // interrupt is the discard effect itself; a cancelled-and-nothing draw grows
    // no hand but is not a discard, so the loop keeps going (the 10-pass cap
    // bounds that).
    for _ in 0..10 {
        if ctx::hand_size(player_id) >= 6 || ctx::player_out(player_id) {
            break;
        }
        let discards = ctx::discard_size(player_id);
        ctx::draw(player_id, 1)?;
        if ctx::discard_size(player_id) > discards {
            break;
        }
    }
    // 规则书（2）: 「（此卡可作为[反击]打出）使用一次Crychic角色的技能」 -- a
    // 「直接从抽牌堆打出」 is a play, so (2) resolves on this route too.
    use_skill(player_id)?;
    // 规则书（1）: the [特] body finishes with the card in the discard pile.
    ctx::to_discard(player_id, ID);
    Ok(())
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    match trigger::kind() {
        TriggerKind::DiceRollResult => reroll_skill(player_id)?,
        TriggerKind::SettleBefore => step_toward_skill(player_id)?,
        // 规则书（2）[反击]: the payment window is the `effect` chain whose link
        // declares a `Pay` (`TriggerKind::Effect` -- `Pay` itself is the post-chain
        // settlement hook and never opens a [反击]). The entry's pre already
        // pinned it to the holder's own incoming rent (`pay_is_rent && actor !=
        // owner && target == owner`), matching 长崎素世（CRYCHIC）'s 「取消那次支付」.
        TriggerKind::Effect => cancel_pay_skill(player_id),
        _ => {}
    }
    Ok(())
}

/// 椎名立希（CRYCHIC）的技能 -- one reroll of the move dice.
fn reroll_skill(player_id: i32) -> card_sdk::Asked {
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- C# `Counteract` "diceRollValue" branch:
    // `int num = Math.Max(0, H.DoMoveRoll(move)); move.Roll = num;`.
    // C# rerolls with `H.DoMoveRoll`, which sums the move's whole dice table.
    let x = ctx::do_move_roll(player_id)?;
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- the new face becomes the move roll.
    trigger::set_move_roll(x);
    ctx::log(
        player_id,
        &Msg::new(key!("haruhikage_reroll"))
            .player_id("who", player_id)
            .i("n", x as i64),
    );
    Ok(())
}

/// 丰川祥子（CRYCHIC）的技能 -- one endpoint step toward a player within 5.
/// Two variants: (3) the owner's own main move (spends 1 fire pot), (2) another
/// player's settle (that player pays 300 and steps toward the owner).
fn step_toward_skill(player_id: i32) -> card_sdk::Asked {
    let actor = trigger::player_id();
    if actor != player_id {
        // 规则书（2）: 「可选择向你支付300资金并使自己的移动终点沿最短路径向你靠近1格」
        if !ctx::ask_yes(
            actor,
            &Msg::new(key!("haruhikage_toward_title")),
            &Msg::new(key!("haruhikage_pay_ask")).player_id("who", player_id),
        )? {
            return Ok(());
        }
        if ctx::transfer(
            actor,
            player_id,
            300,
            &Msg::new(key!("haruhikage_pay"))
                .player_id("who", player_id)
                .tile("tile", trigger::tile()),
        )? <= 0
        {
            return Ok(());
        }
        nudge_toward(actor, ctx::player_pos(player_id));
        return Ok(());
    }
    // 规则书（3）: 「你可消耗1火罐」 -- the pot is the cost of the self variant.
    if !bandori_std::fire::spend(player_id, 1, &Msg::new(key!("haruhikage_spend")))? {
        return Ok(());
    }
    let near = within5(player_id);
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- C# `Counteract` "settleBefore" branch
    // asks `终点向哪名玩家靠近 1 格？` over `H.Within(i, 5, includeSame: false)`.
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("haruhikage_toward_title")),
        &Msg::new(key!("haruhikage_toward_ask")),
        &near,
    )?;
    nudge_toward(player_id, ctx::player_pos(who));
    Ok(())
}

/// 「使自己的移动终点沿最短路径向…靠近1格」 -- one raw position write toward
/// `goal` (C# `H.StepToward`; the host's `teleport_to` is that same write, no
/// settle). A relocation inside `settleBefore` redirects the settle.
fn nudge_toward(who: i32, goal: i32) {
    let pos = ctx::player_pos(who);
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    let fwd = (goal - pos).rem_euclid(n);
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- the endpoint moves 1 tile closer.
    // The teleport lands on the live world and a replayed pass skips it, so log
    // the position actually reached, not the recomputed `to`. The log sits
    // outside the zero-distance early-out: a replay re-reads `pos` from the
    // already-landed world, collapsing `fwd` to 0 -- returning first would drop
    // the line the paused pass never got to post (it paused inside the
    // teleport). A genuine 0-distance target just logs the tile it is on.
    if fwd != 0 {
        let dir = if fwd <= n - fwd { 1 } else { -1 };
        let to = (pos + dir).rem_euclid(n);
        ctx::teleport_to(who, to);
    }
    ctx::log(
        who,
        &Msg::new(key!("haruhikage_step"))
            .player_id("who", who)
            .tile("tile", ctx::player_pos(who)),
    );
}

/// 长崎素世（CRYCHIC）的技能 -- cancel the rent being paid to you, and make the
/// next main move a teleport back to that tile.
fn cancel_pay_skill(player_id: i32) {
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- C# `Counteract` "pay" branch:
    // `trigger.Pay.cancel = true` plus `H.ExtraOf<SoyoTeleportFx>(i)`.
    let tile = trigger::tile();
    ctx::log(
        player_id,
        &Msg::new(key!("haruhikage_pay_note")).player_id("who", player_id),
    );
    // 规则书（2）: 「可取消那次支付」 -- C# `trigger.Pay.cancel = true`.
    trigger::set_pay_amount(0);
    // 规则书（2）: 「使你下回合的主要移动变为传送至触发此技能的那格」 -- the
    // skill's own armed latch: the attached 长崎素世 skill consumes these two
    // state keys in its `DiceRollPlan` / `MoveAfter` entries (soyo_crychic.rs).
    ctx::state::set(player_id, "skill.soyoCrychic.tile", tile);
    ctx::state::set(player_id, "skill.soyoCrychic.armed", 1);
    // TODO(规则书)（使你下回合的主要移动变为传送至触发此技能的那格）: the two state
    //   keys are private consts in soyo_crychic.rs -- keep the spellings in sync.
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

#[cfg(kani)]
#[inline(always)]
pub fn on_drawn_kani(player_id: i32) -> card_sdk::Asked {
    on_drawn(player_id)
}

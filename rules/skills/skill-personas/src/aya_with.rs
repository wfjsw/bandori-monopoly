//! `skill:丸山彩:With~`
//!
//! 规则书（skill sheet, 丸山彩）:
//! > （1）游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得丸山彩
//! > 的（2）技能
//! > （2）自己每次需要支付资金时可选择将自己Y个正面[P✽P粉丝]变反，此次支付的
//! > 分摊前资金减少Y×100（最少0）；如果自己是Pastel✽Palettes角色则将Y个其他
//! > 乐队玩家拥有的反面[P✽P粉丝]变正，否则将所有Pastel✽Palettes角色的1个反面
//! > [P✽P粉丝]变正
//!
//! Every Pastel✽Palettes character's (1) opens the same way -- 「游戏开始后获得
//! 5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得X的（2）技能」 -- and only
//! the granted skill differs. The fan half is here; the *grant* half is a skill
//! attachment (「获得X的（2）技能」), which is the `H.ReplaceSkill` shape and is
//! not built -- see the TODO below. Until it is, a non-P◇P player pressing this
//! card is pressing 丸山彩's own (2), which is the wrong clause.
//!
//! （2）'s 「分摊前」 is the figure *before* any split-pay divides it -- so the
//! cut applies to the whole, and the split happens afterwards.

use card_sdk::abi::HookKind;
use ruleset_core::ctx;
use card_sdk::{key, CardDef, Msg, On};

const FANS_UP: &str = "P✽P粉丝(正)";
const FANS_DOWN: &str = "P✽P粉丝(反)";

pub const AYA_WITH: CardDef = CardDef::new(
    "skill:丸山彩:With~",
    &[
        On::Action(
            &[HookKind::DeckAtGameStart],
            "group_is(owner, 'Pastel✽Palettes')",
            None,
            at_start,
        ),
        // 「此次支付的分摊前资金减少Y×100（最少0）」 -- the **pre-split** total
        // (「分摊前」), so this rides `payCombinedAddBefore` (PIPELINE-AUDIT Q2), not
        // the per-share `paySplitChoose`. A 「[分摊][支付]2000」 is cut to 1500 *before*
        // it divides, not 500 off each share.
        //
        // Rides `PayCombinedAddBefore` (v78), restoring the rulebook PayCombinedAdd
        // position: the interactive `ask_number` + `set_pay_amount` sits
        // immediately before the pure `PayCombinedAdd` filter stage (pre-split).
        On::Action(
            &[HookKind::PayCombinedAddBefore],
            "value > 0 && tok('P✽P粉丝(正)') >= 1",
            Some(mine),
            on_pay,
        ),
    ],
);

fn mine(player_id: i32) -> bool {
    // 「[持续]：[拥有者]不可使用任何Pastel✽Palettes角色的（2）技能」 (初次演出事故)
    // -- the `skillBlock:Pastel✽Palettes` token (one spelling, the band's real
    // name). `skill_blocked` covers the token, the global `skillBlock`, and a
    // `skillBlock` mark on the tile.
    if ruleset_core::ctx::skill_blocked(player_id, "Pastel✽Palettes") {
        return false;
    }
    ctx::trigger::player_id() == player_id
}

/// （1）「游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得丸山彩的
/// （2）技能」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    // （1） belongs to the skill's own Pastel✽Palettes character, not to
    // the grantees of the (2) below -- the `group_is(owner, …)` pre owns that.
    ctx::add_tok(player_id, FANS_UP, 5, i32::MAX)?;
    // 「所有非Pastel✽Palettes玩家获得丸山彩的（2）技能」 -- a grant is the skill
    // rule placed on the grantee's field. `bind_skills` already puts a player's
    // own two there; this adds a third. The body runs for whoever presses it,
    // which is what 「自己」 in （2） then means for the grantee.
    for p in 0..ctx::player_count() {
        if p == player_id || ctx::player_out(p) || ctx::in_group(p, "Pastel✽Palettes") {
            continue;
        }
        ctx::place_card(p, "skill:丸山彩:With~", &Msg::new(key!("aya_with_granted")));
        ctx::log(p, &Msg::new(key!("aya_with_grant")).player_id("who", p));
    }
    Ok(())
}

/// （2）「自己每次需要支付资金时可选择将自己Y个正面[P✽P粉丝]变反，此次支付的
/// 分摊前资金减少Y×100（最少0）」.
/// Rides `PayCombinedAddBefore` (v78): the prompt picks Y, which drives both the
/// amount rewrite (`set_pay_amount`) and the fan flips.
fn on_pay(player_id: i32) -> card_sdk::Asked {
    // 「不可使用」 -- same gate as `mine`, repeated in the body.
    if ruleset_core::ctx::skill_blocked(player_id, "Pastel✽Palettes") {
        return Ok(());
    }
    let amount = ctx::trigger::value();
    let up = ctx::tok(player_id, FANS_UP);
    let y = ctx::ask_number(
        player_id,
        &Msg::new(key!("aya_with_title")),
        &Msg::new(key!("aya_with_ask")).i("n", amount as i64),
        0,
        up.min((amount + 99) / 100),
    )?;
    if y < 1 {
        return Ok(());
    }
    // 「将自己Y个正面[P✽P粉丝]变反」
    ctx::add_tok(player_id, FANS_UP, -y, i32::MAX)?;
    ctx::add_tok(player_id, FANS_DOWN, y, i32::MAX)?;
    // 「此次支付的分摊前资金减少Y×100（最少0）」
    ctx::trigger::set_pay_amount((amount - y * 100).max(0));
    // 「如果自己是Pastel✽Palettes角色则将Y个其他乐队玩家拥有的反面[P✽P粉丝]
    // 变正，否则将所有Pastel✽Palettes角色的1个反面[P✽P粉丝]」变正」
    if ctx::in_group(player_id, "Pastel✽Palettes") {
        flip_others(player_id, y)?;
    } else {
        flip_all_pp()?;
    }
    ctx::log(player_id, &Msg::new(key!("aya_with_done")).i("n", y as i64));
    Ok(())
}

/// 「将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正」 -- spread Y flips across the
/// other players who hold face-down fans.
fn flip_others(player_id: i32, mut y: i32) -> card_sdk::Asked {
    for p in 0..ctx::player_count() {
        if y <= 0 {
            break;
        }
        if p == player_id || ctx::player_out(p) || ctx::in_group(p, "Pastel✽Palettes") {
            continue;
        }
        let down = ctx::tok(p, FANS_DOWN).min(y);
        if down > 0 {
            ctx::add_tok(p, FANS_DOWN, -down, i32::MAX)?;
            ctx::add_tok(p, FANS_UP, down, i32::MAX)?;
            y -= down;
        }
    }
    Ok(())
}

/// 「将所有Pastel✽Palettes角色的1个反面[P✽P粉丝]变正」.
fn flip_all_pp() -> card_sdk::Asked {
    for p in 0..ctx::player_count() {
        if ctx::player_out(p) || !ctx::in_group(p, "Pastel✽Palettes") {
            continue;
        }
        if ctx::tok(p, FANS_DOWN) > 0 {
            ctx::add_tok(p, FANS_DOWN, -1, i32::MAX)?;
            ctx::add_tok(p, FANS_UP, 1, i32::MAX)?;
        }
    }
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
/// cfg(kani) direct-call wrapper (ppcost body-id pattern).
#[cfg(kani)]
#[inline(always)]
pub fn at_start_kani(player_id: i32) -> card_sdk::Asked {
    at_start(player_id)
}

/// cfg(kani) direct-call wrapper (ppcost body-id pattern).
#[cfg(kani)]
#[inline(always)]
pub fn on_pay_kani(player_id: i32) -> card_sdk::Asked {
    on_pay(player_id)
}

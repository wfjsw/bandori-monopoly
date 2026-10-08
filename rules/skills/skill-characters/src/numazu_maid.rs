//! `skill:鳰原令王那:梦幻可爱♪女仆`
//!
//! 规则书（skill sheet, 鳰原令王那）:
//! > （1）游戏开始时如果场上有"Pastel✽Palettes"角色则额外获得1个正面[P✽P粉丝]
//! > （2）你拥有格子上的房屋总数增加时可选择失去1PAREO标记（初始1，上限3），
//! > 所有非自己的玩家分摊支付你"格子的房屋造价×格子上的房屋数"四分之一的资金
//! > （所有你拥有的格子中取最高值）
//!
//! （2）'s figure is 「格子的房屋造价×格子上的房屋数」 -- the build cost of one
//! house times how many stand there -- and 「所有你拥有的格子中取最高值」 picks
//! whichever of the player's deeds gives the largest such product before the
//! quarter is taken. 「分摊支付」 is the split-pay the vocabulary already has.

use card_sdk::abi::HookKind;
use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg, On};

const PAREO: &str = "PAREO标记";

pub const NUMAZU_MAID: CardDef = CardDef::new(
    "skill:鳰原令王那:梦幻可爱♪女仆",
    &[
        On::Hook(&[HookKind::DeckAtGameStart], None, at_start, ""),
        On::Hook(&[HookKind::HouseAdded], None, on_built, card_sdk::pre::MINE),
        // （2）'s offer is also a press, so a card can run it out of turn
        // (pareo_far's 「视为你的房屋总数增加」 -- C# `SkillPareo -> Offer()`).
        On::Play(None, offer, ""),
    ],
)
    .legacy(&[(1, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// （1）「游戏开始时如果场上有"Pastel✽Palettes"角色则额外获得1个正面[P✽P粉丝]」,
/// and （2）'s 「初始1」 -- the PAREO mark starts at 1 (cap 3).
fn at_start(player_id: i32) -> card_sdk::Asked {
    // 规则书（2）: 「失去1PAREO标记（初始1，上限3）」 -- the mark exists from
    // match start, one deep. Written before the (1) loop so the early return
    // there cannot skip it.
    ctx::add_tok(player_id, PAREO, 1, 3)?;
    for p in 0..ctx::player_count() {
        if p != player_id && !ctx::player_out(p) && ctx::in_band(p, "Pastel✽Palettes") {
            ctx::add_tok(player_id, "P✽P粉丝(正)", 1, i32::MAX)?;
            return Ok(());
        }
    }
    Ok(())
}

/// （2）「你拥有格子上的房屋总数增加时可选择失去1PAREO标记」 -- `houseAdded`
/// is the moment a house commits.
fn on_built(player_id: i32) -> card_sdk::Asked {
    offer(player_id)
}

/// （2）'s offer body: lose 1 PAREO mark, the others split-pay a quarter of the
/// largest `build_cost × houses` among the player's deeds. Reached two ways --
/// the `houseAdded` hook above, and the press entry (`On::Play`) a card runs
/// for 「视为你的房屋总数增加」 (pareo_far, C# `SkillPareo -> Offer()`).
fn offer(player_id: i32) -> card_sdk::Asked {
    if ctx::tok(player_id, PAREO) < 1 {
        return Ok(());
    }
    // 「所有你拥有的格子中取最高值」 -- the largest `build_cost × houses` over
    // the player's own deeds, which is 「格子的房屋造价×格子上的房屋数」.
    let mut best = 0;
    for t in ctx::owned_tiles(player_id) {
        let houses = ctx::houses_of(t);
        if houses <= 0 {
            continue;
        }
        let v = ctx::build_cost(t) * houses;
        if v > best {
            best = v;
        }
    }
    if best <= 0 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("numazu_maid_title")),
        &Msg::new(key!("numazu_maid_ask")).i("n", best as i64),
    )? {
        return Ok(());
    }
    ctx::add_tok(player_id, PAREO, -1, i32::MAX)?;
    // 「所有非自己的玩家分摊支付你…四分之一的资金」.
    let pot = best / 4;
    let others: alloc::vec::Vec<i32> = (0..ctx::player_count())
        .filter(|&p| p != player_id && !ctx::player_out(p))
        .collect();
    if others.is_empty() {
        return Ok(());
    }
    // `PIPELINE-AUDIT` Q2: the command-wide pre-split stage shapes `pot`
    // before it divides (the 「分摊前」 figure).
    let why = Msg::new(key!("numazu_maid_why"));
    let Some(shaped) = ctx::pay_total(others[0], player_id, pot, &why)? else {
        return Ok(());
    };
    let per = (shaped + others.len() as i32 - 1) / others.len() as i32;
    let share = ((per + 9) / 10) * 10;
    for p in others {
        ctx::pay_leg(p, player_id, share, &why)?;
    }
    ctx::log(
        player_id,
        &Msg::new(key!("numazu_maid_done")).i("n", share as i64),
    );
    Ok(())
}

//! `skill:大和麻弥:朝阳照耀的片刻`
//!
//! 规则书（skill sheet, 大和麻弥）:
//! > （1）游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得大和麻弥
//! > 的（2）技能
//! > （2）自己抽卡时时可选择将自己Y个正面[P✽P粉丝]变反，此次抽卡改为观看卡组顶端
//! > Y+1张卡（卡组数量不足Y+1则观看全部卡组），然后选择一张牌（不公开）加入手牌
//! > （此次加手视为抽卡动作），剩余观看的牌洗回卡组；如果自己是Pastel✽Palettes角色
//! > 则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正，否则将所有Pastel✽Palettes角色的
//! > 1个反面[P✽P粉丝]变正
//!
//! （1）'s grant is the skill rule placed on the grantee's field (see
//! [`super::aya_with`]).
//!
//! （2） 「此次抽卡改为观看卡组顶端Y+1张卡」 replaces the draw with a look: the
//! top `Y+1` are shown, one goes to hand as a draw, the rest go back to the
//! deck. 「剩余观看的牌洗回卡组」 is the return Ok(()), not a discard.

use alloc::string::String;

use card_sdk::abi::{CardPile, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const FANS_UP: &str = "P✽P粉丝(正)";
const FANS_DOWN: &str = "P✽P粉丝(反)";

pub const MAYA_DAWN: CardDef = CardDef::new(
    "skill:大和麻弥:朝阳照耀的片刻",
    &[
        On::Hook(&[HookKind::DeckAtGameStart], "", None, at_start),
        // （2） replaces the draw, so it runs at the per-card **before** point.
        On::Hook(&[HookKind::DrewBefore], card_sdk::pre::MINE, None, on_draw),
    ],
)
    .legacy(&[(1, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// （1）「游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得大和麻弥的
/// （2）技能」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    // （1） belongs to the skill's own Pastel✽Palettes character, not to
    // the grantees of the (2) below -- their copies must not re-fire it.
    if !ctx::in_band(player_id, "Pastel✽Palettes") {
        return Ok(());
    }
    ctx::add_tok(player_id, FANS_UP, 5, i32::MAX)?;
    for p in 0..ctx::player_count() {
        if p == player_id || ctx::player_out(p) || ctx::in_band(p, "Pastel✽Palettes") {
            continue;
        }
        ctx::place_card(
            p,
            "skill:大和麻弥:朝阳照耀的片刻",
            &Msg::new(key!("maya_dawn_granted")),
        );
    }
    Ok(())
}

/// （2）「自己抽卡时时可选择将自己Y个正面[P✽P粉丝]变反，此次抽卡改为观看卡组顶端
/// Y+1张卡…」.
fn on_draw(player_id: i32) -> card_sdk::Asked {
    let up = ctx::tok(player_id, FANS_UP);
    if up < 1 {
        return Ok(());
    }
    let y = ctx::ask_number(
        player_id,
        &Msg::new(key!("maya_dawn_title")),
        &Msg::new(key!("maya_dawn_ask")),
        0,
        up,
    )?;
    if y < 1 {
        return Ok(());
    }
    let mut y = y;
    ctx::add_tok(player_id, FANS_UP, -y, i32::MAX)?;
    ctx::add_tok(player_id, FANS_DOWN, y, i32::MAX)?;
    // 「观看卡组顶端Y+1张卡（卡组数量不足Y+1则观看全部卡组）」
    let deck = ctx::cards_in(player_id, CardPile::Deck);
    let n = deck.len().min((y + 1) as usize);
    let look: alloc::vec::Vec<String> = deck[..n].to_vec();
    if look.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("maya_dawn_pick_title")),
        &Msg::new(key!("maya_dawn_pick_text")),
        &look
            .iter()
            .map(|c| Msg::new(key!("maya_dawn_option")).card("card", c))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    // 「选择一张牌（不公开）加入手牌（此次加手视为抽卡动作）」 -- the pick is
    // the draw: it replaces the plain draw (`set_cancelled`) and the engine's
    // after-draw points fire for the card that entered the hand.
    let Some(keep) = look.get(pick).cloned() else {
        return Ok(());
    };
    if ctx::take_card(player_id, CardPile::Deck, &keep) {
        ctx::add_to_hand(player_id, &keep);
        trigger::set_cancelled();
    }
    // 「剩余观看的牌洗回卡组」
    for c in look.iter().filter(|c| **c != keep) {
        ctx::take_card(player_id, CardPile::Deck, c);
        ctx::add_to_deck(player_id, c, true);
    }
    // The fan tail, identical to the other three.
    if ctx::in_band(player_id, "Pastel✽Palettes") {
        for p in 0..ctx::player_count() {
            if y <= 0 {
                break;
            }
            if p == player_id || ctx::player_out(p) || ctx::in_band(p, "Pastel✽Palettes") {
                continue;
            }
            let down = ctx::tok(p, FANS_DOWN).min(y);
            if down > 0 {
                ctx::add_tok(p, FANS_DOWN, -down, i32::MAX)?;
                ctx::add_tok(p, FANS_UP, down, i32::MAX)?;
                y -= down;
            }
        }
    } else {
        for p in 0..ctx::player_count() {
            if ctx::player_out(p) || !ctx::in_band(p, "Pastel✽Palettes") {
                continue;
            }
            if ctx::tok(p, FANS_DOWN) > 0 {
                ctx::add_tok(p, FANS_DOWN, -1, i32::MAX)?;
                ctx::add_tok(p, FANS_UP, 1, i32::MAX)?;
            }
        }
    }
    ctx::log(
        player_id,
        &Msg::new(key!("maya_dawn_done")).i("n", y as i64),
    );
    Ok(())
}

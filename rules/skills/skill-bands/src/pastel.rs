//! `skill:Pastel✽Palettes:与偶像一起`
//!
//! 规则书（band sheet, Pastel✽Palettes）:
//! > （1）游戏开始时非Pastel✽Palettes角色获得1个反面[P✽P粉丝]（如有场上有多张
//! > Pastel✽Palettes乐队卡时此效果不重复发动）。（2）你因任意原因受到"将X个反面
//! > [P✽P粉丝]变正"的效果且X大于拥有的反面的[P✽P粉丝]时可为此卡添加等量溢出的
//! > [奇迹水晶]（最多10个）。（3）[经过]CiRCLE时不获得[CiRCLE奖励]。（4）回合开始时
//! > 此卡添加1个[奇迹水晶]，然后可选择移除此卡5个[奇迹水晶]并抽1张卡。
//!
//! 「反面[P✽P粉丝]」 is a player counter, held face-down by default; 「将X个…
//! 变正」 is the flip effect that consumes them. （3） is the plan's
//! no-circle-reward flag on the walk that passes a CiRCLE.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "skill:Pastel✽Palettes:与偶像一起";
/// 「反面[P✽P粉丝]」 -- a face-down fan counter (`P✽P粉丝(反)`, the same token
/// every other card reads and writes).
const FAN: &str = "P✽P粉丝(反)";

pub const PASTEL: CardDef = CardDef::new(
    "skill:Pastel✽Palettes:与偶像一起",
    &[
        On::Hook(&[HookKind::DeckAtGameStart], "", None, at_start),
        On::Hook(&[HookKind::TurnStartBefore], card_sdk::pre::MINE, None, at_turn_start),
        On::Play("", Some(can_flip), flip),
    ],
)
    .legacy(&[(1, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// （1）「游戏开始时非Pastel✽Palettes角色获得1个反面[P✽P粉丝]（如有场上有多张
/// Pastel✽Palettes乐队卡时此效果不重复发动）」 -- the band card's own game-start
/// clause; it hits every non-Pastel✽Palettes character on the board, not just
/// the card's owner.
fn at_start(player_id: i32) -> card_sdk::Asked {
    // 「如有场上有多张…乐队卡时此效果不重复发动」 -- only the first copy on
    // the board (lowest seat holding one) deals.
    let first = (0..ctx::player_count()).find(|&p| ctx::placed_cards(p).iter().any(|c| c == ID));
    if first != Some(player_id) {
        return Ok(());
    }
    for p in 0..ctx::player_count() {
        if ctx::player_out(p) || ctx::in_band(p, "Pastel✽Palettes") {
            continue;
        }
        ctx::add_tok(p, FAN, 1, i32::MAX)?;
    }
    Ok(())
}

/// （3）「[经过]CiRCLE时不获得[CiRCLE奖励]」 and （4）「回合开始时此卡添加1个
/// [奇迹水晶]…」. （3） is `prop::NO_REWARD` on this instance (`docs/TILES.md`)
/// -- the source owns the arming and the disarming, and the CiRCLE-reward step
/// reads it off the passing player's field. Set every turn start so the prop
/// tracks the card's presence.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    ctx::set_prop(card_sdk::abi::prop::NO_REWARD, 1);
    ctx::add_crystals(1, i32::MAX)?;
    if ctx::crystals() < 5 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("pastel_title")),
        &Msg::new(key!("pastel_draw")),
    )? {
        return Ok(());
    }
    ctx::add_crystals(-5, i32::MAX)?;
    ctx::draw(player_id, 1)?;
    Ok(())
}

/// （2）「将X个反面[P✽P粉丝]变正…X大于拥有的反面的[P✽P粉丝]时可为此卡添加
/// 等量溢出的[奇迹水晶]（最多10个）」 -- the press is the flip.
fn can_flip(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if ctx::tok(player_id, FAN) < 1 {
        return Some(Msg::new(key!("pastel_no_fan")));
    }
    None
}

fn flip(player_id: i32) -> card_sdk::Asked {
    crate::spend_copy_sticker(player_id)?;
    let have = ctx::tok(player_id, FAN);
    let want = ctx::ask_number(
        player_id,
        &Msg::new(key!("pastel_title")),
        &Msg::new(key!("pastel_how_many")),
        1,
        have.max(1),
    )?;
    if want <= 0 {
        return Ok(());
    }
    ctx::add_tok(player_id, FAN, -want, i32::MAX)?;
    // 「X大于拥有的反面的[P✽P粉丝]时可为此卡添加等量溢出的[奇迹水晶]（最多10个）」
    let over = (want - have).max(0);
    if over > 0 {
        ctx::add_crystals(over, 10)?;
        ctx::log(
            player_id,
            &Msg::new(key!("pastel_over")).i("n", over as i64),
        );
    }
    Ok(())
}

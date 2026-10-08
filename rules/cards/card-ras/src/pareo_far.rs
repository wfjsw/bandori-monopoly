//! `RAS:（PAREO）渐渐远去的你` -- C# `CardPareoFar`: up to 2 PAREO marks
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（PAREO）渐渐远去的你`）:
//! > （PAREO）渐渐远去的你：
//! > 获得2个PAREO标记，然后视为你的房屋总数增加且可选择移除任意你拥有的格子上的一层房屋
//!
//! (cap 3). 「视为你的房屋总数增加」 is the character-skill offer (C#
//! `SkillPareo -> Offer()`), reached through `ctx::character_skill` +
//! `ctx::invoke_skill`.

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg, On};

/// The shared mark name. `skill:鳰原令王那:梦幻可爱♪女仆` (2) reads and spends
/// 「PAREO标记」 by that literal, so the card must add under the same name --
/// a `key!`-namespaced stand-in would be a different mark and the
/// 「失去1PAREO标记」 offer would see 0. 规则书: 「获得2个PAREO标记」 /
/// 「失去1PAREO标记（初始1，上限3）」 name one mark.
const PAREO: &str = "PAREO标记";

pub const PAREO_FAR: CardDef =
    CardDef::new("RAS:（PAREO）渐渐远去的你", &[On::Play(None, pareo_far, "")]);

fn pareo_far(player_id: i32) -> card_sdk::Asked {
    let got = ctx::add_tok(player_id, PAREO, 2, 3)?;
    let total = ctx::tok(player_id, PAREO);
    ctx::log(
        player_id,
        &Msg::new(key!("pareo_far_got"))
            .player_id("who", player_id)
            .i("got", got as i64)
            .i("total", total as i64),
    );
    // 规则书: 「视为你的房屋总数增加」 -- C# `H._fx[i].skill is SkillPareo ->
    // Offer()`: reach the player's character skill and trigger its offer (the
    // 「失去1PAREO标记，所有非自己的玩家分摊支付…」 half of 鳰原令王那 (2)).
    // The skill exposes that offer as its press entry (`On::Play`), so the
    // invoke is `ctx::invoke_skill` on the id `ctx::character_skill` names.
    // Only when that skill *is* SkillPareo -- the C#'s `is SkillPareo` check;
    // some other character skill has no such offer.
    const SKILL_PAREO: &str = "skill:鳰原令王那:梦幻可爱♪女仆";
    if let Some(skill) = ctx::character_skill(player_id) {
        if skill == SKILL_PAREO {
            ctx::invoke_skill(player_id, &skill)?;
        }
    }
    // 规则书: 「可选择移除任意你拥有的格子上的一层房屋」 -- optional, any tile
    // this player owns, one layer. The ABI has no `ask_tile(allowNone: true)?`,
    // so the option is a yes/no first and the tile a second prompt; that reads
    // the same to the player and costs nothing extra when they decline.
    let with_houses: Vec<i32> = ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&t| ctx::houses_of(t) > 0)
        .collect();
    if !with_houses.is_empty() {
        let yes = ctx::ask_yes(
            player_id,
            &Msg::new(key!("pareo_far_ask_title")),
            &Msg::new(key!("pareo_far_ask_text")),
        )?;
        if yes {
            let t = ctx::ask_tile(
                player_id,
                &Msg::new(key!("pareo_far_pick_title")),
                &Msg::new(key!("pareo_far_pick_text")),
                &with_houses,
            )?;
            if t >= 0 {
                ctx::add_house(t, -1);
                ctx::log(
                    player_id,
                    &Msg::new(key!("pareo_far_removed")).tile("tile", t),
                );
            }
        }
    }
    Ok(())
}

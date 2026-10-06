//! `RAS:（PAREO）渐渐远去的你` -- C# `CardPareoFar`: up to 2 PAREO marks
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（PAREO）渐渐远去的你`）:
//! > （PAREO）渐渐远去的你：
//! > 获得2个PAREO标记，然后视为你的房屋总数增加且可选择移除任意你拥有的格子上的一层房屋
//!
//! (cap 3). The rest of the C# effect needs the character-skill hook
//! (TODO in source).

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const PAREO_FAR: CardDef =
    CardDef::new("RAS:（PAREO）渐渐远去的你", &[On::Play(None, pareo_far)]);

fn pareo_far(player_id: i32) -> card_sdk::Asked {
    let got = ctx::add_tok(player_id, key!("pareo_far_tok"), 2, 3);
    let total = ctx::tok(player_id, key!("pareo_far_tok"));
    ctx::log(
        player_id,
        &Msg::new(key!("pareo_far_got"))
            .player_id("who", player_id)
            .i("got", got as i64)
            .i("total", total as i64),
    );
    // TODO(ABI): H._fx[i].skill is SkillPareo -> Offer() -- needs the skill hook
    // and SplitPay; `ctx::houses_of` is ready for the house-count part.
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

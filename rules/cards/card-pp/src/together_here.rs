//! `PP:有你与我在这里共度` -- C# `CardTogetherHere`: flip X face-down fans up.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:有你与我在这里共度`）:
//! > 有你与我在这里共度：
//! > [手]：
//! > 将X个反面[P✽P粉丝]变正；X为5，如果[使用者]拥有的正面[P✽P粉丝]数量少于反面[P✽P粉丝]数量则X额外添加反面和正面[P✽P粉丝]数量的差的一半（向上取整），如果[共鸣]则X减少5并抽1张卡。
//!
//! X is 5, +ceil((down - up)/2) while down > up; [共鸣] cuts X by 5 and draws.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const TOGETHER_HERE: CardDef =
    CardDef::new("PP:有你与我在这里共度", &[On::Play(None, together_here)]);

/// The C# `H.FansUp` / `H.FansDown` token names (`P✽P粉丝` faces).
const FANS_UP: &str = "P✽P粉丝(正)";
const FANS_DOWN: &str = "P✽P粉丝(反)";

fn together_here(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「X为5，如果[使用者]拥有的正面[P✽P粉丝]数量少于反面[P✽P粉丝]数量则X额外添加反面和正面[P✽P粉丝]数量的差的一半（向上取整）」
    let up = ctx::tok(player_id, FANS_UP);
    let down = ctx::tok(player_id, FANS_DOWN);
    let mut x = 5i32;
    if up < down {
        x += (down - up + 1) / 2;
    }
    // 规则书[手]: 「如果[共鸣]则X减少5并抽1张卡」
    if crate::resonance::try_resonance(player_id)? {
        x -= 5;
        ctx::draw(player_id, 1);
    }
    // 规则书[手]: 「将X个反面[P✽P粉丝]变正」
    let flip = x.max(0).min(down);
    if flip > 0 {
        ctx::add_tok(player_id, FANS_DOWN, -flip, i32::MAX);
        ctx::add_tok(player_id, FANS_UP, flip, i32::MAX);
        ctx::log(
            player_id,
            &Msg::new(key!("together_here_up"))
                .player_id("who", player_id)
                .i("n", flip as i64),
        );
    }
    Ok(())
}

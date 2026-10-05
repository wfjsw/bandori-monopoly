//! `RAS:（PAREO）渐渐远去的你` -- C# `CardPareoFar`: up to 2 PAREO marks
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（PAREO）渐渐远去的你`）:
//! > （PAREO）渐渐远去的你：
//! > 获得2个PAREO标记，然后视为你的房屋总数增加且可选择移除任意你拥有的格子上的一层房屋
//!
//! (cap 3). The rest of the C# effect needs the character-skill hook
//! (TODO in source).

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const PAREO_FAR: CardDef = CardDef::new("RAS:（PAREO）渐渐远去的你", &[On::Play(pareo_far)]);

fn pareo_far(player_id: i32) {
    let got = ctx::add_tok(player_id, key!("pareo_far_tok"), 2, 3);
    let total = ctx::tok(player_id, key!("pareo_far_tok"));
    ctx::log(player_id, &Msg::new(key!("pareo_far_got")).player_id("who", player_id).i("got", got as i64).i("total", total as i64));
    // TODO(ABI): H._fx[i].skill is SkillPareo -> Offer() -- needs the skill hook
    // and SplitPay; `ctx::houses_of` is ready for the house-count part.
    // TODO(ABI): optional demolition of one house on an owned tile --
    // `ctx::houses_of` / `ctx::add_house` are ready; `ask_tile(allowNone: true)`
    // is still missing so the optional prompt cannot be expressed.
}

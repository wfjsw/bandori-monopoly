//! `PPP:Returns` -- C# `CardReturns` (MatchHost.cs:8403-8581): a [特] card that
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:Returns`）:
//! > Returns：
//! >  [特]：
//!
//! > （1）如果此卡被加入初始卡组则卡组卡数添加8。
//!
//! > （2）游戏开始时此卡从卡组放置到拥有此卡的玩家的[场地]上并获得一个其他存活玩家的团卡。
//! > [持续]：
//!
//! > （1）无效[拥有者]Poppin' Party团卡的
//! > （4）效果。
//!
//! > （2）[拥有者]每回合开始时选择一个其他存活玩家的团卡，如果和当前因此卡获得的团卡不一样则替换并移除上面的所有[奇迹水晶]。
//!
//! > （3）使用因此卡获得的团卡的主动效果时需要支付1星星贴纸。
//!
//! > （4）[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸。
//!
//! swells the starting deck, sits on the field from game start, and copies another
//! player's band card at a sticker price.
//! A pure [特] card (C# has no `Play`): deck setup and game-start placement sit on
//! the `DeckBeforeGame` / `DeckAtGameStart` hooks. The band-card copying and the
//! four [持续] clauses still need machinery the ABI does not carry (TODO below).

use alloc::string::String;
use alloc::vec::Vec;

use bandori_std::ctx;
use card_sdk::abi::{ActionKind, CardPile};
use card_sdk::{CardText, it, items, sec};
use card_sdk::{key, Action, CardDef, Msg, On};

const ID: &str = "PPP:Returns";
/// Index into `SKILLS` of the currently borrowed band, on the Returns instance
/// (`FieldCard::props`) -- scoped to this card, not the player's state map.
const PROP_BAND: &str = "returns.group";
/// Seat the borrowed band card was copied from, on the Returns instance -- the
/// board reads it to say whose card the owner holds. -1 = nothing borrowed.
const PROP_FROM: &str = "returns.from";

pub const RETURNS: CardDef = CardDef::new(
    "PPP:Returns",
    &[
        // [持续]（4）「[拥有者]的通用卡的[手]效果全部生效后」 -- `pre::MINE`
        // states the actor; the residual guard names the 通用 card. The body
        // used to re-check the actor and the prefix and return early -- a flash
        // per non-通用 play by the owner.
        On::Action(Action::CardPlayed(card_sdk::pre::MINE, Some(on_played_is_general), on_played)),
        // [持续]（2）「每回合开始时选择一个其他存活玩家的团卡」 -- the offer
        // exists only while Returns is placed and some other survivor has a
        // band card to borrow. Derived scans (pool + `is_placed`), residual.
        On::Action(Action::TurnStartBefore(card_sdk::pre::MINE, Some(choose_band_applies), choose_band)),
        // [特]（1）「如果此卡被加入初始卡组则卡组卡数添加8」 -- the +8 is
        // unconditional on the rulebook, but the pick prompt only exists when
        // the pool can feed eight draws. Empty pool = no offer, no flash.
        On::Action(Action::DeckBeforeGame("", Some(deck_pool_ready), deck_before_game)),
        On::Action(Action::DeckAtGameStart("trigger_card == card.id", None, deck_at_game_start)),
    ],
)
    .props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Returns"#)
        .raw_name(r#"Returns"#)
        .tags(&[r#"持续"#, r#"特"#])
        .text(CardText::of(&[
            sec(r#"[特]："#, &[
                items(&[
                    it(r#"如果此卡被加入初始卡组则卡组卡数添加8。"#),
                    it(r#"游戏开始时此卡从卡组放置到拥有此卡的玩家的[场地]上并获得一个其他存活玩家的团卡。"#),
                ]),
            ]),
            sec(r#"[持续]："#, &[
                items(&[
                    it(r#"无效[拥有者]Poppin' Party团卡的（4）效果。"#),
                    it(r#"[拥有者]每回合开始时选择一个其他存活玩家的团卡，如果和当前因此卡获得的团卡不一样则替换并移除上面的所有[奇迹水晶]。"#),
                    it(r#"使用因此卡获得的团卡的主动效果时需要支付1星星贴纸。"#),
                    it(r#"[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸。"#),
                ]),
            ]),
        ]))
        // sheet cell B5
        .fields(&[("group", r#"Poppin' Party"#), ("owner", "")]));

/// C# `DeckRules.Pool` for a Poppin' Party character, in pool order (exclusive
/// first, then band, then general) -- a snapshot of the roster; Returns is
/// a Poppin' Party band card so only that band's pool can hold it. The exclusive
/// entry is the player's own character card (`exclusive_of`).
const BAND_POOL: [&str; 10] = [
    "PPP:Popipa",
    "PPP:Returns",
    "PPP:Tomorrow's Door",
    "PPP:Bang Dream!",
    "PPP:STAR BEAT!",
    "PPP:抓到了",
    "PPP:迷宫般的仓库",
    "PPP:献给远方的你",
    "PPP:仓库里的Random Star",
    "PPP:向着未来的路标",
];
const GENERAL_POOL: [&str; 10] = [
    "通用:@Tsugu ycm",
    "通用:登上武道馆",
    "通用:GREAT",
    "通用:10次招募（1回限定）",
    "通用:该清CP了",
    "通用:雨啊，快点来吧",
    "通用:网络链接异常",
    "通用:尽力后的收获",
    "通用:CiRCLE THANKS PARTY!",
    "通用:安可",
];

/// C# `DeckRules.Pool`'s exclusive front (the player's own character card).
fn exclusive_of(player_id: i32) -> Option<&'static str> {
    if ctx::persona_is(player_id, "户山香澄") {
        Some("PPP:（香澄）大家我都喜欢哦")
    } else if ctx::persona_is(player_id, "花园多惠") {
        Some("PPP:（多惠）寻找更好的声音")
    } else if ctx::persona_is(player_id, "牛込里美") {
        Some("PPP:（里美）我的心就像巧克力螺")
    } else if ctx::persona_is(player_id, "山吹沙绫") {
        Some("PPP:（沙绫）总有一天要给这片天空命名")
    } else if ctx::persona_is(player_id, "市谷有咲") {
        Some("PPP:（有咲）等等等一下")
    } else {
        None
    }
}

/// C# `DeckRules.Pool(c)` ids minus anything already in the draw pile or hand.
fn pool_for(player_id: i32) -> Vec<String> {
    let mut have: Vec<String> = ctx::cards_in(player_id, CardPile::Deck);
    have.extend(ctx::cards_in(player_id, CardPile::Hand));
    let mut out: Vec<String> = Vec::new();
    let push = |id: &'static str, out: &mut Vec<String>| {
        if !have.iter().any(|h| h == id) && !out.iter().any(|o| o == id) {
            out.push(String::from(id));
        }
    };
    if let Some(ex) = exclusive_of(player_id) {
        push(ex, &mut out);
    }
    for id in BAND_POOL {
        push(id, &mut out);
    }
    for id in GENERAL_POOL {
        push(id, &mut out);
    }
    out
}

// 规则书[特]（1）: 「如果此卡被加入初始卡组则卡组卡数添加8。」
/// C# `CardReturns.DeckBeforeGame` -> `AddEight`: eight ids from `DeckRules.Pool`
/// go into the draw pile (prompted pick or random), then the pile is shuffled.
fn deck_before_game(player_id: i32) -> card_sdk::Asked {
    // `deck_pool_ready` is the residual guard; an empty pool never reaches here.
    let mut pool = pool_for(player_id);
    // C# `H.AskYes(..., "卡组里有「Returns」：卡组的卡数 +8。要自己从卡池里选 8 张
    // 加入抽卡区吗？（不选就随机）")`.
    let pick = ctx::ask_yes(
        player_id,
        &Msg::new(key!("returns_add_eight_title")),
        &Msg::new(key!("returns_add_eight_ask")).card("card", ID),
    )?;
    let mut added = 0;
    while added < 8 && !pool.is_empty() {
        let index = if pick {
            // C# `H.AskCard(seat, ..., "选一张加入抽卡区（n/8）", pool, ...)`.
            let refs: Vec<&str> = pool.iter().map(|s| s.as_str()).collect();
            let i = ctx::ask_card(
                player_id,
                &Msg::new(key!("returns_add_eight_title")),
                &Msg::new(key!("returns_add_eight_pick")).i("n", added as i64 + 1),
                &refs,
            )?;
            i.min(pool.len() - 1)
        } else {
            // C# `H._rng.Next(pool.Count)` -- a silent host rng; the only host-side
            // draw here is `ctx::roll`, which also logs the face.
            (ctx::roll(player_id, 1, pool.len() as i32) - 1).max(0) as usize % pool.len()
        };
        let id = pool.remove(index);
        // C# `h.draw.Add(pool[index])` then `H.Shuffle(h.draw)` at the end; each
        // insert here shuffles, which leaves the same shuffled pile.
        ctx::add_to_dest(player_id, &id, ctx::Dest::DeckRandom);
        added += 1;
    }
    ctx::log(
        player_id,
        &Msg::new(key!("returns_added"))
            .player_id("who", player_id)
            .i("n", added as i64),
    );
    Ok(())
}

// 规则书[特]（2）: 「游戏开始时此卡从卡组放置到拥有此卡的玩家的[场地]上并获得一个
// 其他存活玩家的团卡。」
/// C# `CardReturns.DeckAtGameStart` -> `Setup`: pull the id out of the draw pile
/// (or hand) and place it on the owner's field, then borrow a band.
fn deck_at_game_start(player_id: i32) -> card_sdk::Asked {
    // `trigger_card == card.id` owns "this card was opened".
    // C# `hidden.draw.Remove(Id) || hidden.hand.Remove(Id)` then `H.PlaceCard`.
    if !ctx::take_card(player_id, ID, CardPile::Deck)
        && !ctx::take_card(player_id, ID, CardPile::Hand)
    {
        return Ok(());
    }
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("returns_note")));
    // -1 = no borrow yet (the prop's defined default is 0, which is a valid
    // `SKILLS` index -- Poppin' Party -- so it cannot serve as the sentinel).
    ctx::set_prop(PROP_BAND, -1);
    ctx::set_prop(PROP_FROM, -1);
    // [持续]（1）「无效[拥有者]Poppin' Party团卡的（4）效果」 -- arm the token
    // `skill-groups::poppin` reads to skip its no-build-on-main-settle rule.
    ctx::set_tok(player_id, "returns.noBuild4", 1);
    ctx::log(
        player_id,
        &Msg::new(key!("returns_placed")).player_id("who", player_id),
    );
    // 规则书[特]（2）: 「并获得一个其他存活玩家的团卡」 -- the borrowed band
    // sits BESIDE the PPP one (ruling 2026-10-06). Same pick as the per-turn
    // [持续]（2） replacement.
    borrow_band(player_id)?;
    Ok(())
}

// [持续]（1）「无效[拥有者]Poppin' Party团卡的（4）效果。」 -- while Returns is
// in play the PPP band card's main-settle build restriction is lifted. Read by
// `skill-groups::poppin` off the owner's field (`find_card("PPP:Returns")`).
// [持续]（2）「[拥有者]每回合开始时选择一个其他存活玩家的团卡，如果和当前因此卡
// 获得的团卡不一样则替换并移除上面的所有[奇迹水晶]」 -- a `turnStart` press.
// The borrowed band sits beside the PPP one (ruling 2026-10-06); only a prior
// borrow is swapped out, and its crystals leave with it.
// [持续]（3）「使用因此卡获得的团卡的主动效果时需要支付1星星贴纸」 -- the copy
// is the instance `place_card` returned (`returns.borrow_uid`); the owner's own
// band card sits beside it and its presses are free.
// `skill_groups::copy_sticker_gate` is the activation cost (0 stickers → the
// press is refused) and `spend_copy_sticker` charges the 1 when it runs.
// [持续]（4）「[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸」 -- the
// `cardPlayed` hook is in; a card's band is the prefix of its id (`通用:`, `G:`),
// which is what `H.Db.Card(c.Id)?.group == "通用"` reads.

/// （4）'s applicability: the played card is a 通用 one (a card's band is the
/// prefix of its id -- `H.Db.Card(c.Id)?.group == "通用"`). The actor is
/// `pre::MINE`'s job and is not re-checked here (docs/GUARDS.md).
fn on_played_is_general(_player_id: i32) -> bool {
    ctx::trigger::cards()
        .into_iter()
        .next()
        .is_some_and(|id| id.starts_with("通用") || id.starts_with("G:"))
}

/// （4）「[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸」.
fn on_played(player_id: i32) -> card_sdk::Asked {
    ctx::add_tok(player_id, "星星贴纸", 1, i32::MAX)?;
    ctx::log(player_id, &Msg::new(key!("returns_sticker")));
    Ok(())
}

const GROUPS: [&str; 12] = [
    "Poppin' Party",
    "Afterglow",
    "Pastel✽Palettes",
    "Roselia",
    "Hello, Happy World!",
    "Morfonica",
    "RAISE A SUILEN",
    "MyGO!!!!!",
    "Ave Mujica",
    "Sumimi",
    "CRYCHIC",
    "CiRCLE",
];
const SKILLS: [&str; 12] = [
    "skill:Poppin' Party:星之鼓动",
    "skill:Afterglow:商店街的宠儿",
    "skill:Pastel✽Palettes:与偶像一起",
    "skill:Roselia:对音乐的纯粹",
    "skill:Hello, Happy World!:传播笑容",
    "skill:Morfonica:振翅高飞的练习曲",
    "skill:RAISE A SUILEN:UNSTOPPABLE",
    "skill:MyGO!!!!!:迷途之星",
    "skill:Ave Mujica:假面之下的真实",
    "skill:Sumimi:人气偶像组合",
    "skill:CRYCHIC:美好的往日幻影",
    "skill:CiRCLE:后勤人员的努力",
];

/// Which of the twelve bands `p` is in, or `""`.
fn band_of(p: i32) -> &'static str {
    for b in [
        "Poppin' Party",
        "Afterglow",
        "Pastel✽Palettes",
        "Roselia",
        "Hello, Happy World!",
        "Morfonica",
        "RAISE A SUILEN",
        "MyGO!!!!!",
        "Ave Mujica",
        "Sumimi",
        "CRYCHIC",
        "CiRCLE",
    ] {
        if ctx::in_group(p, b) {
            return b;
        }
    }
    ""
}

/// Other surviving players who hold a band card -- the borrow pool.
fn band_pool(player_id: i32) -> alloc::vec::Vec<i32> {
    ctx::others(player_id)
        .into_iter()
        .filter(|&p| !ctx::player_out(p) && !band_of(p).is_empty())
        .collect()
}

/// Residual guard for the per-turn borrow offer ([持续]（2）).
///
/// 规则书（docs/rulebook/cards.json, id `PPP:Returns`）:
/// > （2）[拥有者]每回合开始时选择一个其他存活玩家的团卡，如果和当前因此卡
/// > 获得的团卡不一样则替换并移除上面的所有[奇迹水晶]。
///
/// 「选择一个其他存活玩家的团卡」 names a non-empty set; with no other
/// survivor holding a band card there is nothing to choose, so the offer does
/// not exist (no prompt, no flash). `is_placed` is the card being in play.
fn choose_band_applies(player_id: i32) -> bool {
    ctx::is_placed() && !band_pool(player_id).is_empty()
}

/// Residual guard for the starting-deck +8 pick prompt ([特]（1）).
///
/// 规则书: 「如果此卡被加入初始卡组则卡组卡数添加8。」 -- the +8 draws from
/// the card pool; an empty pool cannot feed the prompt (or the eight draws).
fn deck_pool_ready(player_id: i32) -> bool {
    !pool_for(player_id).is_empty()
}

/// （2）「每回合开始时选择一个其他存活玩家的团卡…替换并移除上面的所有[奇迹水晶]」.
/// Applicability is `choose_band_applies`.
fn choose_band(player_id: i32) -> card_sdk::Asked {
    borrow_band(player_id)
}

/// Shared by the game-start borrow ([特]（2）) and the per-turn replacement
/// ([持续]（2）). The borrowed band sits **beside** the owner's own Poppin'
/// Party band card (ruling 2026-10-06): only a previously borrowed band is
/// replaced, never the PPP one.
///
/// The pool-empty early-out stays here: `deck_at_game_start` calls this after
/// the mandatory placement side effects (take/field/token), so lifting it to
/// that entry's residual would skip the placement when the pool is empty --
/// more than the offer's existence. `choose_band`'s entry is guarded instead.
fn borrow_band(player_id: i32) -> card_sdk::Asked {
    let pool = band_pool(player_id);
    if pool.is_empty() {
        return Ok(());
    }
    // 「复制哪名玩家的团卡？」 -- each option shows that player's 团卡 beside the
    // name (a prompt that asks about a card shows the card, not just its title).
    // The bound band skill is that 团卡's rule id; fall back to the band table
    // when nothing is bound yet.
    let cards: alloc::vec::Vec<String> = pool
        .iter()
        .map(|&p| {
            ctx::group_skill(p).unwrap_or_else(|| {
                let band = band_of(p);
                GROUPS
                    .iter()
                    .position(|b| *b == band)
                    .and_then(|i| SKILLS.get(i))
                    .map(|s| String::from(*s))
                    .unwrap_or_default()
            })
        })
        .collect();
    let refs: alloc::vec::Vec<&str> = cards.iter().map(|s| s.as_str()).collect();
    let who = ctx::ask_player_cards(
        player_id,
        &Msg::new(key!("returns_title")),
        &Msg::new(key!("returns_which_band")),
        &pool,
        &refs,
    )?;
    if who < 0 {
        return Ok(());
    }
    let band = band_of(who);
    let mut want = "";
    let mut idx = -1;
    for (i, b) in GROUPS.iter().enumerate() {
        if *b == band {
            idx = i as i32;
            want = SKILLS[i];
        }
    }
    if idx < 0 {
        return Ok(());
    }
    // 「如果和当前因此卡获得的团卡不一样则替换」
    let prev = ctx::prop(PROP_BAND);
    if prev == idx {
        return Ok(());
    }
    // Drop only the previously *borrowed* band skill (「替换并移除上面的所有
    // [奇迹水晶]」 -- its crystals live on its own instance and leave with it).
    // The owner's own Poppin' Party band card stays: ruling 2026-10-06 says the
    // borrowed card sits BESIDE it, and its skills (1)–(3) keep working.
    if prev >= 0 {
        if let Some(old) = SKILLS.get(prev as usize) {
            ctx::unplace_card_named(player_id, old);
        }
    }
    let borrowed = ctx::place_card(player_id, want, &Msg::new(key!("returns_band")));
    ctx::set_prop(PROP_BAND, idx);
    ctx::set_prop(PROP_FROM, who);
    // Which instance is 「因此卡获得的团卡」. The tax in `spend_copy_sticker` /
    // `copy_sticker_gate` keys on this uid, not on "a borrow exists" -- the
    // owner's own band card sits beside the copy and its presses are free.
    // (`returns.copied` was the old "any borrow" flag; nothing reads it now.)
    ctx::set_tok(player_id, "returns.borrow_uid", borrowed);
    ctx::log(
        player_id,
        &Msg::new(key!("returns_got_band")).card("card", want),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn on_played_kani(player_id: i32) -> card_sdk::Asked {
    on_played(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn choose_band_kani(player_id: i32) -> card_sdk::Asked {
    choose_band(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn deck_before_game_kani(player_id: i32) -> card_sdk::Asked {
    deck_before_game(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn deck_at_game_start_kani(player_id: i32) -> card_sdk::Asked {
    deck_at_game_start(player_id)
}

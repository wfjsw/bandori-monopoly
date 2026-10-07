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

use card_sdk::abi::{CardPile, HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "PPP:Returns";
/// Index into `SKILLS` of the currently borrowed band, on the Returns instance
/// (`FieldCard::props`) -- scoped to this card, not the player's state map.
const PROP_BAND: &str = "returns.band";

pub const RETURNS: CardDef = CardDef::new(
    "PPP:Returns",
    &[
        On::Hook(&[card_sdk::abi::HookKind::CardPlayed], mine, on_played),
        On::Hook(
            &[card_sdk::abi::HookKind::TurnStartBefore],
            mine,
            choose_band,
        ),
        On::Hook(&[HookKind::DeckBeforeGame], |_| true, deck_before_game),
        On::Hook(&[HookKind::DeckAtGameStart], |_| true, deck_at_game_start),
    ],
);

/// C# `DeckRules.Pool` for a Poppin' Party character, in pool order (exclusive
/// first, then band, then general) -- a snapshot of `data/cards.json`; Returns is
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
    if ctx::character_is(player_id, "户山香澄") {
        Some("PPP:（香澄）大家我都喜欢哦")
    } else if ctx::character_is(player_id, "花园多惠") {
        Some("PPP:（多惠）寻找更好的声音")
    } else if ctx::character_is(player_id, "牛込里美") {
        Some("PPP:（里美）我的心就像巧克力螺")
    } else if ctx::character_is(player_id, "山吹沙绫") {
        Some("PPP:（沙绫）总有一天要给这片天空命名")
    } else if ctx::character_is(player_id, "市谷有咲") {
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
    let mut pool = pool_for(player_id);
    if pool.is_empty() {
        return Ok(());
    }
    // C# `H.AskYes(..., "卡组里有「Returns」：卡组的卡数 +8。要自己从卡池里选 8 张
    // 加入抽卡区吗？（不选就随机）")`.
    let pick = ctx::ask_yes(
        player_id,
        &Msg::new(key!("returns_add_eight_title")),
        &Msg::new(key!("returns_add_eight_ask")),
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
        ctx::add_to_deck(player_id, &id, true);
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
    if trigger::kind() != TriggerKind::DeckAtGameStart || !trigger::card_is(ID) {
        return Ok(());
    }
    // C# `hidden.draw.Remove(Id) || hidden.hand.Remove(Id)` then `H.PlaceCard`.
    if !ctx::take_card(player_id, CardPile::Deck, ID)
        && !ctx::take_card(player_id, CardPile::Hand, ID)
    {
        return Ok(());
    }
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("returns_note")));
    // -1 = no borrow yet (the prop's defined default is 0, which is a valid
    // `SKILLS` index -- Poppin' Party -- so it cannot serve as the sentinel).
    ctx::set_prop(PROP_BAND, -1);
    // [持续]（1）「无效[拥有者]Poppin' Party团卡的（4）效果」 -- arm the token
    // `skill-bands::poppin` reads to skip its no-build-on-main-settle rule.
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
// `skill-bands::poppin` off the owner's field (`find_card("PPP:Returns")`).
// [持续]（2）「[拥有者]每回合开始时选择一个其他存活玩家的团卡，如果和当前因此卡
// 获得的团卡不一样则替换并移除上面的所有[奇迹水晶]」 -- a `turnStart` press.
// The borrowed band sits beside the PPP one (ruling 2026-10-06); only a prior
// borrow is swapped out, and its crystals leave with it.
// [持续]（3）「使用因此卡获得的团卡的主动效果时需要支付1星星贴纸」 -- the copy
// is marked with `returns.copied`, and `skill_bands::skill_ok` charges the
// sticker before a press.
// [持续]（4）「[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸」 -- the
// `cardPlayed` hook is in; a card's band is the prefix of its id (`通用:`, `G:`),
// which is what `H.Db.Card(c.Id)?.band == "通用"` reads.

/// （4）「[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸」.
fn on_played(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::player_id() != player_id {
        return Ok(());
    }
    let Some(id) = ctx::trigger::cards().into_iter().next() else {
        return Ok(());
    };
    if !id.starts_with("通用") && !id.starts_with("G:") {
        return Ok(());
    }
    ctx::add_tok(player_id, "星星贴纸", 1, i32::MAX);
    ctx::log(player_id, &Msg::new(key!("returns_sticker")));
    Ok(())
}

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

const BANDS: [&str; 12] = [
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
        if ctx::in_band(p, b) {
            return b;
        }
    }
    ""
}

/// （2）「每回合开始时选择一个其他存活玩家的团卡…替换并移除上面的所有[奇迹水晶]」.
fn choose_band(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_placed() {
        return Ok(());
    }
    borrow_band(player_id)
}

/// Shared by the game-start borrow ([特]（2）) and the per-turn replacement
/// ([持续]（2）). The borrowed band sits **beside** the owner's own Poppin'
/// Party band card (ruling 2026-10-06): only a previously borrowed band is
/// replaced, never the PPP one.
fn borrow_band(player_id: i32) -> card_sdk::Asked {
    let pool: alloc::vec::Vec<i32> = ctx::others(player_id)
        .into_iter()
        .filter(|&p| !ctx::player_out(p) && !band_of(p).is_empty())
        .collect();
    if pool.is_empty() {
        return Ok(());
    }
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("returns_title")),
        &Msg::new(key!("returns_which_band")),
        &pool,
    )?;
    if who < 0 {
        return Ok(());
    }
    let band = band_of(who);
    let mut want = "";
    let mut idx = -1;
    for (i, b) in BANDS.iter().enumerate() {
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
    ctx::place_card(player_id, want, &Msg::new(key!("returns_band")));
    ctx::set_prop(PROP_BAND, idx);
    ctx::set_tok(player_id, "returns.copied", 1);
    ctx::log(
        player_id,
        &Msg::new(key!("returns_got_band")).card("card", want),
    );
    Ok(())
}

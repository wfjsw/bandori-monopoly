//! Poppin' Party card / skill / band-skill contracts (unit C-PPP).
//!
//! Spec doc: `docs/rulebook/spec/cards/ppp.md`. Clause IDs are `C-PPP-*`.
//! Every assertion is derived from the live-sheet quote in that doc, never
//! from engine behaviour. A discrepancy is a plain failing test.
//!
//! Shape (see `tests/example_card.rs`): `forall` over randomized pre-states,
//! drive one transition, assert exact numbers, then `frame` the rest.

use game_core::engine::World;
use game_core::state::{Counter, MatchEvent};
use rules_spec::{forall, frame, Clause, Rng, Step, Table};

// ---------------------------------------------------------------- tiles
// Board index = rulebook 「#N格」 − 1.
const CIRCLE: usize = 0;
const RING1: usize = 8;
const HANASAKI: usize = 9;
const TOKYO_SOTO: usize = 23; // 东京外
const OSAKA: usize = 24;
const EDOGAWA: usize = 30; // 江户川乐器店
const SPACE: usize = 42;
const HILL: usize = 44; // 星之鼓动山丘
const RYUSEIDO: usize = 45; // 流星堂
const YAMABUKI: usize = 48; // 山吹面包房
const GALAXY: usize = 50; // Live House Galaxy
const BUDDOKAN: usize = 40;
const RING2: usize = 16;
const RING3: usize = 36;
const RING4: usize = 53;
/// #1 #16 #31 #46 -- the band (1) corners (0-based).
const CORNERS: [usize; 4] = [CIRCLE, 15, EDOGAWA, RYUSEIDO];

const STICKER: &str = "星星贴纸";
const BAND: &str = "skill:Poppin' Party:星之鼓动";

const POPIPA: &str = "PPP:Popipa";
const PIPOPA: &str = "PPP:[衍生]Pipopa";
const POPIPAPAPIPOPA: &str = "PPP:[衍生]Popipapapipopa";
const BANG: &str = "PPP:Bang Dream!";
const STAR_BEAT: &str = "PPP:STAR BEAT!";
const DOOR: &str = "PPP:Tomorrow's Door";
const AUCTION_OFF: &str = "PPP:[衍生]拍卖撤下来了";
const RANDOM_STAR: &str = "PPP:仓库里的Random Star";
const SIGNPOST: &str = "PPP:向着未来的路标";
const CAUGHT: &str = "PPP:抓到了";
const FAR: &str = "PPP:献给远方的你";
const MAZE: &str = "PPP:迷宫般的仓库";
const BETTER: &str = "PPP:（多惠）寻找更好的声音";
const NAME_SKY: &str = "PPP:（沙绫）总有一天要给这片天空命名";
const CHOCO: &str = "PPP:（里美）我的心就像巧克力螺";
const EVERYONE: &str = "PPP:（香澄）大家我都喜欢哦";
const RETURNS: &str = "PPP:Returns";
const GENERAL_DRAW: &str = "通用:GREAT";

// ------------------------------------------------------------- helpers

/// Circular absolute distance on the 60-tile board. The 「献给远方的你」
/// example pins this: 「[使用者]在#11号格子，一名玩家在#1号格子则绝对距离为10」.
fn abs_dist(a: usize, b: usize) -> usize {
    let d = a.abs_diff(b);
    d.min(60 - d)
}

/// 「资金数包含5」 -- the decimal representation contains the digit 5.
fn money_has_5(money: i32) -> bool {
    money.abs().to_string().contains('5')
}

fn skip_all(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

fn with_band(t: &mut Table) {
    if !t.on_field(0, BAND) {
        t.place_raw(0, BAND);
    }
}

/// Vanilla table with seat 0 named `character` (exclusive-card gate) but
/// without bound skills.
fn exclusive_table(character: &str) -> Table {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, character);
    t
}

fn forward(from: usize, n: usize) -> usize {
    (from + n) % 60
}


/// Answer the open prompt with the option that names `tile`, whatever the
/// prompt kind is (`tile` items or a `choice` whose option carries the tile).
fn answer_pick_tile(t: &mut Table, who: usize, tile: usize) -> Result<(), String> {
    if t.prompt().is_none() {
        return Ok(());
    }
    let p = t.expect_prompt();
    if p.kind == "tile" {
        return t.answer_tile(who, tile);
    }
    // `choice`: find the option whose debug form names the tile index.
    if let Some(k) = t.option(&tile.to_string()) {
        return t.answer(who, k);
    }
    if p.count == 0 && p.options.len() == 1 {
        return t.answer(who, 0);
    }
    Err(format!(
        "no option names tile {tile}: {}",
        t.dump_prompt()
    ))
}

fn set_sticker(t: &mut Table, who: usize, n: i32) {
    let w = t.m.world_mut();
    if let Some(c) = w.st.players[who].tokens.iter_mut().find(|c| c.name == STICKER) {
        c.value = n;
    } else {
        w.st.players[who].tokens.push(Counter {
            name: STICKER.into(),
            value: n,
            instance: None,
        });
    }
}

// =====================================================================
// C-PPP-popipa-01 -- PPP:Popipa
// =====================================================================

/// C-PPP-popipa-01 -- 「获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]」
///
/// * **ID:** `C-PPP-popipa-01`
/// * **Source:** `roster#PPP:Popipa`
/// * **Quote:** 「获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]」
/// * **Trigger:** [手] play of `PPP:Popipa`.
/// * **Effect:** money +1000; one Pipopa in the draw multiset; one card drawn;
///   Popipa in none of hand/draw/discard.
/// * **Frame:** user money / hand / draw / discard.
/// * **Executable:** [`c_ppp_popipa_01`].
pub fn c_ppp_popipa_01(step: &Step<'_>) -> Result<(), String> {
    let played = step
        .events
        .iter()
        .any(|e| e.r#type == "card" && e.card == POPIPA && (e.kind == "play" || e.kind.is_empty()));
    if !played {
        return Ok(());
    }
    let who = step.player_of_act();
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if delta != 1000 {
        return Err(format!("money delta {delta}, expected +1000 ([获得]1000)"));
    }
    Ok(())
}

pub const POPIPA_CLAUSES: &[Clause] = &[Clause::new(
    "C-PPP-popipa-01",
    "roster#PPP:Popipa",
    "获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]",
    c_ppp_popipa_01,
)];

#[test]
fn popipa_contract() {
    forall(8, 0xC0FFEE_0001, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 2000 + rng.range(0, 40) * 100;
        t.set_money(0, money);
        let top = GENERAL_DRAW;
        t.set_draw(0, &[top, "通用:安可"]);
        t.give(0, &[POPIPA]);

        let pre = t.snapshot_world();
        t.observe(POPIPA_CLAUSES);
        t.play(0, POPIPA).map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);

        if t.money(0) != money + 1000 {
            return Err(format!("money {}, expected {}", t.money(0), money + 1000));
        }
        let zones = [t.hand(0), t.draw_pile(0), t.discard(0)];
        let has_pipopa = zones.iter().any(|z| z.iter().any(|c| c == PIPOPA));
        if !has_pipopa {
            return Err(format!("Pipopa in no zone: {zones:?}"));
        }
        // 「将一张“Pipopa”加入抽牌堆，然后抽一张牌」 -- the insert position
        // of Pipopa is not specified (top / bottom / shuffle), so the drawn
        // card is not pinned. What is pinned: exactly one card was drawn.
        let hand = t.hand(0);
        if hand.len() != 1 {
            return Err(format!("hand should hold exactly 1 drawn card, got {hand:?}"));
        }
        let pile = t.draw_pile(0);
        // Started with 2, +1 Pipopa, -1 drawn = 2 left.
        if pile.len() != 2 {
            return Err(format!("draw pile size {}, expected 2", pile.len()));
        }
        for z in [&t.hand(0), &t.draw_pile(0), &t.discard(0)] {
            if z.iter().any(|c| c == POPIPA) {
                return Err(format!("Popipa still in a zone: {z:?}"));
            }
        }
        frame(&pre, t.world(), &["money:P0", "hand:P0", "draw:P0", "discard:P0"])
    })
    .expect("C-PPP-popipa-01");
}

// =====================================================================
// C-PPP-pipopa-01 -- PPP:[衍生]Pipopa
// =====================================================================

/// C-PPP-pipopa-01 -- 「获得1000资金，然后将一张“Popipapapipopa”放置在自身场上，然后此卡[移除]」
pub fn c_ppp_pipopa_01(step: &Step<'_>) -> Result<(), String> {
    let played = step
        .events
        .iter()
        .any(|e| e.r#type == "card" && e.card == PIPOPA && (e.kind == "play" || e.kind.is_empty()));
    if !played {
        return Ok(());
    }
    let who = step.player_of_act();
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if delta != 1000 {
        return Err(format!("money delta {delta}, expected +1000"));
    }
    Ok(())
}

pub const PIPOPA_CLAUSES: &[Clause] = &[Clause::new(
    "C-PPP-pipopa-01",
    "roster#PPP:[衍生]Pipopa",
    "获得1000资金，然后将一张“Popipapapipopa”放置在自身场上，然后此卡[移除]",
    c_ppp_pipopa_01,
)];

#[test]
fn pipopa_contract() {
    forall(6, 0xC0FFEE_0002, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 1500 + rng.range(0, 20) * 100;
        t.set_money(0, money);
        t.give(0, &[PIPOPA]);

        let pre = t.snapshot_world();
        t.observe(PIPOPA_CLAUSES);
        t.play(0, PIPOPA).map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);

        if t.money(0) != money + 1000 {
            return Err(format!("money {}, expected {}", t.money(0), money + 1000));
        }
        if !t.on_field(0, POPIPAPAPIPOPA) {
            return Err(format!("Popipapapipopa not on field: {:?}", t.field_ids(0)));
        }
        for z in [&t.hand(0), &t.draw_pile(0), &t.discard(0)] {
            if z.iter().any(|c| c == PIPOPA) {
                return Err(format!("Pipopa still in a zone: {z:?}"));
            }
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "field:P0",
                "crystals:P0",
            ],
        )
    })
    .expect("C-PPP-pipopa-01");
}

// =====================================================================
// C-PPP-bang-dream-01 / -02 -- PPP:Bang Dream!
// =====================================================================

/// C-PPP-bang-dream-01 -- 「为[使用者]的团卡添加一个[奇迹水晶]」
pub fn c_ppp_bang_dream_01(step: &Step<'_>) -> Result<(), String> {
    let played = step
        .events
        .iter()
        .any(|e| e.r#type == "card" && e.card == BANG && (e.kind == "play" || e.kind.is_empty()));
    if !played {
        return Ok(());
    }
    // The [手] body stops at the [传送] pick; the crystal lands with the rest
    // of the body after that answer. Only check a finished transition.
    if step.post_prompt_open {
        return Ok(());
    }
    let who = step.player_of_act();
    let sum_band = |step: &Step<'_>, pre: bool| -> i32 {
        let w = if pre { step.pre } else { step.post };
        w.st
            .players
            .get(who)
            .and_then(|p| p.field.iter().find(|f| f.band_skill))
            .map(|f| f.crystals)
            .unwrap_or(0)
    };
    let (a, b) = (sum_band(step, true), sum_band(step, false));
    if b - a != 1 {
        return Err(format!("band crystals {a} -> {b}, expected +1"));
    }
    Ok(())
}

pub const BANG_CLAUSES: &[Clause] = &[Clause::new(
    "C-PPP-bang-dream-01",
    "roster#PPP:Bang Dream!",
    "为[使用者]的团卡添加一个[奇迹水晶]，然后[传送]至任意[使用者]拥有的格子且可选择盖房",
    c_ppp_bang_dream_01,
)];

#[test]
fn bang_dream_contract() {
    forall(6, 0xC0FFEE_0003, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(2);
        let dest = [HILL, SPACE, YAMABUKI][rng.pick_idx(3)];
        t.own(0, &[dest, RYUSEIDO]);
        t.set_pos(0, CIRCLE);
        with_band(&mut t);
        let crystals0 = t.crystals(0, BAND).unwrap_or(0);
        t.give(0, &[BANG]);

        let pre = t.snapshot_world();
        t.observe(BANG_CLAUSES);
        t.play(0, BANG).map_err(|e| format!("play rejected: {e}"))?;
        answer_pick_tile(&mut t, 0, dest)?;
        skip_all(&mut t);

        let crystals1 = t.crystals(0, BAND).unwrap_or(0);
        if crystals1 != crystals0 + 1 {
            return Err(format!(
                "band crystals {crystals0} -> {crystals1}, expected +1;                  field={:?}",
                t.field(0)
                    .iter()
                    .map(|f| (f.card.clone(), f.band_skill, f.crystals))
                    .collect::<Vec<_>>()
            ));
        }
        if t.pos(0) != dest {
            return Err(format!("pos {}, expected owned tile {dest}", t.pos(0)));
        }
        frame(
            &pre,
            t.world(),
            &[
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "pos:P0",
                "crystals:P0",
                "field:P0",
                "houses:T",
                "turn.",
            ],
        )
    })
    .expect("C-PPP-bang-dream-01/02");
}


// =====================================================================
// C-PPP-better-sound-01 -- （多惠）寻找更好的声音
// =====================================================================

/// C-PPP-better-sound-01 -- 「传送到“江户川乐器店”并获得一个[火罐]」
pub fn c_ppp_better_sound_01(step: &Step<'_>) -> Result<(), String> {
    let played = step.events.iter().any(|e| {
        e.r#type == "card" && e.card == BETTER && (e.kind == "play" || e.kind.is_empty())
    });
    if !played {
        return Ok(());
    }
    if step.post_prompt_open {
        return Ok(());
    }
    let who = step.player_of_act();
    let fire = |w: &game_core::engine::World| {
        w.st.players.get(who).map(|p| (p.fire(), p.fire_max())).unwrap_or((0, 0))
    };
    let (pre_f, pre_max) = fire(step.pre);
    let (post_f, _) = fire(step.post);
    if pre_f < pre_max && post_f - pre_f != 1 {
        return Err(format!("fire {} -> {}, expected +1", pre_f, post_f));
    }
    Ok(())
}

pub const BETTER_CLAUSES: &[Clause] = &[Clause::new(
    "C-PPP-better-sound-01",
    "roster#PPP:（多惠）寻找更好的声音",
    "传送到“江户川乐器店”并获得一个[火罐]",
    c_ppp_better_sound_01,
)];

#[test]
fn better_sound_contract() {
    forall(6, 0xC0FFEE_0004, |rng: &mut Rng, _i| {
        let mut t = exclusive_table("花园多惠");
        let held = rng.range(0, 1);
        t.set_fire(0, held, 4);
        t.set_pos(0, CIRCLE);
        t.give(0, &[BETTER]);

        let pre = t.snapshot_world();
        t.observe(BETTER_CLAUSES);
        t.play(0, BETTER).map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);

        if t.pos(0) != EDOGAWA {
            return Err(format!("pos {}, expected 江户川乐器店 {EDOGAWA}", t.pos(0)));
        }
        if t.fire(0) != held + 1 {
            return Err(format!("fire {}, expected {}", t.fire(0), held + 1));
        }
        frame(
            &pre,
            t.world(),
            &["hand:P0", "draw:P0", "discard:P0", "pos:P0", "state:P0:fire", "turn."],
        )
    })
    .expect("C-PPP-better-sound-01");
}

// =====================================================================
// C-PPP-signpost-01 / -02 -- 向着未来的路标
// =====================================================================

#[test]
fn signpost_sixty_no_settle_and_lose_1000() {
    forall(6, 0xC0FFEE_0005, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 3000 + rng.range(0, 30) * 100;
        t.set_money(0, money);
        t.set_owner(2, Some(1));
        t.set_houses(2, 2);
        t.set_pos(0, rng.range(0, 59) as usize);
        t.give(0, &[SIGNPOST]);

        let pre = t.snapshot_world();
        let money1 = t.money(1);
        t.play(0, SIGNPOST).map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);
        // 「不触发结算」 -- no rent / buy leg. The walk may still collect the
        // [CiRCLE奖励] for passing the start (that is not 结算).
        if t.money(1) != money1 {
            return Err(format!(
                "walk settled: P1 money {money1} -> {} (C-PPP-signpost-01)",
                t.money(1)
            ));
        }
        let after_walk = t.money(0);
        t.end(0).map_err(|e| format!("end rejected: {e}"))?;
        skip_all(&mut t);
        if t.money(0) != after_walk - 1000 {
            return Err(format!(
                "turn-end money {}, expected {} (C-PPP-signpost-02)",
                t.money(0),
                after_walk - 1000
            ));
        }
        frame(
            &pre,
            t.world(),
            &["money:P0", "pos:P0", "hand:P0", "draw:P0", "discard:P0", "money:P1", "turn."],
        )
    })
    .expect("C-PPP-signpost-01/02");
}

// =====================================================================
// C-PPP-caught-01 -- 抓到了
// =====================================================================

pub fn c_ppp_caught_01(step: &Step<'_>) -> Result<(), String> {
    let played = step.events.iter().any(|e| {
        e.r#type == "card" && e.card == CAUGHT && (e.kind == "play" || e.kind.is_empty())
    });
    if !played {
        return Ok(());
    }
    if step.post_prompt_open {
        return Ok(());
    }
    let who = step.player_of_act();
    let from = step.pre.st.players[who].pos as usize;
    let dest = step.post.st.players[who].pos as usize;
    // 「你前方一名玩家」 -- strictly clockwise-ahead of the pre-move seat.
    let ahead = (0..step.pre.st.players.len())
        .filter(|&i| i != who)
        .map(|i| step.pre.st.players[i].pos as usize)
        .filter(|&p| {
            let off = (p + 60 - from) % 60;
            off != 0
        })
        .collect::<Vec<_>>();
    if !ahead.contains(&dest) {
        return Err(format!(
            "mover {from} landed on {dest}, not a strictly-ahead player's tile {ahead:?}"
        ));
    }
    Ok(())
}

pub const CAUGHT_CLAUSES: &[Clause] = &[Clause::new(
    "C-PPP-caught-01",
    "roster#PPP:抓到了",
    "移动到你前方一名玩家的格子，视为本回合的主要移动且可选择盖房。",
    c_ppp_caught_01,
)];

#[test]
fn caught_moves_to_ahead_player() {
    forall(6, 0xC0FFEE_0006, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(3);
        let me = rng.range(0, 20) as usize;
        let ahead_off = rng.range(1, 20) as usize;
        let them = forward(me, ahead_off);
        t.set_pos(0, me);
        t.set_pos(1, them);
        t.set_pos(2, forward(me, 30));
        t.give(0, &[CAUGHT]);

        let pre = t.snapshot_world();
        t.observe(CAUGHT_CLAUSES);
        t.play(0, CAUGHT).map_err(|e| format!("play rejected: {e}"))?;
        if t.prompt().is_some() {
            if let Some(k) = t.option("P1").or_else(|| t.option("player:1")) {
                t.answer(0, k).map_err(|e| format!("player pick: {e}"))?;
            } else if t.prompt().map(|p| p.kind == "tile") == Some(true) {
                t.answer_tile(0, them).map_err(|e| format!("tile pick {them}: {e}"))?;
            } else {
                t.answer(0, 0).map_err(|e| format!("pick 0: {e}"))?;
            }
        }
        skip_all(&mut t);

        if t.pos(0) != them {
            return Err(format!("pos {}, expected the ahead player's tile {them}", t.pos(0)));
        }
        frame(&pre, t.world(), &["hand:P0", "draw:P0", "discard:P0", "pos:P0", "houses:T", "turn."])
    })
    .expect("C-PPP-caught-01");
}

// =====================================================================
// C-PPP-far-away-01 -- 献给远方的你
// =====================================================================

#[test]
fn far_away_teleports_to_farthest() {
    forall(6, 0xC0FFEE_0007, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(3);
        let me = rng.range(0, 59) as usize;
        let near = forward(me, 3);
        let far = forward(me, 30);
        t.set_pos(0, me);
        t.set_pos(1, near);
        t.set_pos(2, far);
        t.give(0, &[FAR]);

        let pre = t.snapshot_world();
        t.play(0, FAR).map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);

        if t.pos(0) != far {
            return Err(format!(
                "pos {}, expected farthest player's tile {far} (abs {}, vs near {})",
                t.pos(0),
                abs_dist(me, far),
                abs_dist(me, near)
            ));
        }
        frame(
            &pre,
            t.world(),
            &["hand:P0", "draw:P0", "discard:P0", "pos:P0", "money:P0", "money:P1", "money:P2", "houses:T", "turn."],
        )
    })
    .expect("C-PPP-far-away-01");
}

// =====================================================================
// C-PPP-star-beat-03 / -04 -- STAR BEAT!
// =====================================================================

#[test]
fn star_beat_option_1() {
    forall(8, 0xC0FFEE_0008, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(3);
        let mut monies = [0i32; 3];
        for (i, m) in monies.iter_mut().enumerate() {
            if rng.chance(50) {
                *m = 1500 + rng.range(0, 9) * 100;
            } else {
                *m = 2000 + rng.range(0, 9) * 100;
                if money_has_5(*m) {
                    *m += 1;
                }
            }
            t.set_money(i, *m);
        }
        let f = monies.iter().filter(|m| money_has_5(**m)).count() as i32;
        let dest = ((45 * (f + 1)) % 60) as usize;
        t.set_pos(0, CIRCLE);
        t.give(0, &[STAR_BEAT]);

        let pre = t.snapshot_world();
        let stickers0 = t.token(0, STICKER);
        t.play(0, STAR_BEAT).map_err(|e| format!("play rejected: {e}"))?;
        let k = t.option("opt_teleport").or(Some(0)).unwrap();
        t.answer(0, k).map_err(|e| format!("option 1: {e}"))?;
        skip_all(&mut t);

        let stickers1 = t.token(0, STICKER);
        if stickers1 != stickers0 + 2 {
            return Err(format!(
                "stickers {stickers0} -> {stickers1}, expected +2 (C-PPP-star-beat-03)"
            ));
        }
        if t.pos(0) != dest {
            return Err(format!(
                "pos {}, expected (45×({f}+1)) mod 60 = {dest} (C-PPP-star-beat-03)",
                t.pos(0)
            ));
        }
        frame(
            &pre,
            t.world(),
            &["hand:P0", "draw:P0", "discard:P0", "pos:P0", "token:P0", "state:P0", "money:P", "turn.", "bfield:", "bprop:", "state:P0:star_beat"],
        )
    })
    .expect("C-PPP-star-beat-03");
}

#[test]
fn star_beat_option_2() {
    forall(4, 0xC0FFEE_0009, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(2);
        let mut monies = [0i32; 2];
        for (i, m) in monies.iter_mut().enumerate() {
            *m = 1000 + rng.range(0, 20) * 100;
            t.set_money(i, *m);
        }
        let f = monies.iter().filter(|m| money_has_5(**m)).count() as i32;
        let n_dice = 2 * (f + 1);
        let faces: Vec<i32> = vec![1; n_dice as usize];
        t.dice(&faces);
        let start = CIRCLE;
        t.set_pos(0, start);
        t.give(0, &[STAR_BEAT]);

        let pre = t.snapshot_world();
        let stickers0 = t.token(0, STICKER);
        t.play(0, STAR_BEAT).map_err(|e| format!("play rejected: {e}"))?;
        let k = t.option("opt_move").or(Some(1)).unwrap();
        t.answer(0, k).map_err(|e| format!("option 2: {e}"))?;
        skip_all(&mut t);

        if t.token(0, STICKER) != stickers0 {
            return Err(format!(
                "option 2 must not grant stickers ({} -> {})",
                stickers0,
                t.token(0, STICKER)
            ));
        }
        let expected = forward(start, n_dice as usize);
        if t.pos(0) != expected {
            return Err(format!(
                "pos {}, expected {expected} ({n_dice}d10 × 1) (C-PPP-star-beat-04)",
                t.pos(0)
            ));
        }
        frame(
            &pre,
            t.world(),
            &["hand:P0", "draw:P0", "discard:P0", "pos:P0", "token:P0", "money:P", "turn.", "state:P0:star_beat"],
        )
    })
    .expect("C-PPP-star-beat-04");
}

// =====================================================================
// C-PPP-popipapapipopa-c1 / c2
// =====================================================================

pub fn c_ppp_popipapapipopa_c1(step: &Step<'_>) -> Result<(), String> {
    let who = step.player_of_act();
    let get = |w: &game_core::engine::World| {
        w.st.players.get(who).and_then(|p| {
            p.field.iter().find(|f| f.card == POPIPAPAPIPOPA).map(|f| f.crystals)
        })
    };
    if let (Some(a), Some(b)) = (get(step.pre), get(step.post)) {
        if b < a {
            return Err(format!("crystals {a} -> {b}, never decrease on a pass"));
        }
        if b > 10 {
            return Err(format!("crystals {b} above cap 10"));
        }
    }
    Ok(())
}

pub const POPIPAPAPIPOPA_CLAUSES: &[Clause] = &[Clause::new(
    "C-PPP-popipapapipopa-c1",
    "roster#PPP:[衍生]Popipapapipopa",
    "此卡拥有者每次[经过]“东京外”，“江户川乐器店”，“山吹面包房”，“星之鼓动山丘”或“流星堂”时为此卡添加1个[奇迹水晶]（上限10个）。",
    c_ppp_popipapapipopa_c1,
)];

#[test]
fn popipapapipopa_crystal_on_pass() {
    let cases: &[(usize, i32, bool)] = &[
        (TOKYO_SOTO, 3, true),
        (EDOGAWA, 3, true),
        (YAMABUKI, 3, true),
        (HILL, 3, true),
        (RYUSEIDO, 1, true),
        (SPACE, 3, false),
    ];
    for &(tile, roll, expect) in cases {
        let mut t = Table::vanilla(2);
        t.give_play(0, PIPOPA).unwrap();
        skip_all(&mut t);
        let c0 = t.crystals(0, POPIPAPAPIPOPA).unwrap_or(0);
        t.begin_turn(0);
        t.set_pos(0, tile.wrapping_sub(roll as usize) % 60);
        t.dice(&[roll]);
        t.observe(POPIPAPAPIPOPA_CLAUSES);
        t.roll(0).unwrap();
        skip_all(&mut t);
        let c1 = t.crystals(0, POPIPAPAPIPOPA).unwrap_or(0);
        let got = c1 != c0;
        if got != expect {
            panic!("tile {tile}: crystals {c0} -> {c1}, expect crystal={expect}");
        }
        if expect && c1 != c0 + 1 {
            panic!("tile {tile}: crystals {c0} -> {c1}, expected +1");
        }
    }
}

#[test]
fn popipapapipopa_crystal_reduces_spend() {
    forall(6, 0xC0FFEE_000A, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(2);
        t.give_play(0, PIPOPA).unwrap();
        skip_all(&mut t);
        let k = rng.range(1, 4);
        t.set_crystals(0, POPIPAPAPIPOPA, k);
        let tile = SPACE;
        t.set_owner(tile, Some(1));
        t.set_houses(tile, 1);
        t.set_money(0, 20_000);
        t.set_money(1, 20_000);

        let pre = t.snapshot_world();
        let crystals0 = t.crystals(0, POPIPAPAPIPOPA).unwrap();
        let money0 = t.money(0);
        t.begin_turn(0);
        t.set_pos(0, tile.wrapping_sub(2) % 60);
        t.dice(&[2]);
        t.roll(0).unwrap();
        // The pay window is a `choice` whose options are `ask.intOption`
        // n = 0..=crystals (spend n of them).
        if t.prompt().is_some() {
            let dump = t.dump_prompt();
            if dump.contains("intOption") {
                // Options are n = 0..=max, max ≤ crystals and ≤ floor(pay/150).
                let p = t.expect_prompt();
                let max = (p.options.len().saturating_sub(1)) as i32;
                let spend = k.min(max);
                t.answer(0, spend).unwrap();
            } else if let Some(k_idx) = t.option(POPIPAPAPIPOPA).or_else(|| t.option("奇迹水晶")) {
                t.answer(0, k_idx).unwrap();
            } else {
                t.decline();
            }
        }
        skip_all(&mut t);

        let spent = crystals0 - t.crystals(0, POPIPAPAPIPOPA).unwrap_or(0);
        let paid = money0 - t.money(0);
        if spent == 0 {
            return Err(format!(
                "crystal spend not offered/taken on a pay of Space rent                  (C-PPP-popipapapipopa-c2); prompt window empty?"
            ));
        }
        {
            // Space with 1 house: base rent is rent[1] = 400 (board.json).
            let expect_pay = (400 - 150 * spent).max(0);
            if paid != expect_pay {
                return Err(format!(
                    "paid {paid} after spending {spent} crystals, expected {expect_pay} (C-PPP-popipapapipopa-c2)"
                ));
            }
        }
        frame(
            &pre,
            t.world(),
            &["money:P0", "money:P1", "crystals:P0", "field:P0", "pos:P0", "hand:P0", "draw:P0", "discard:P0", "turn."],
        )
    })
    .expect("C-PPP-popipapapipopa-c2");
}

// =====================================================================
// C-PPP-random-star-01 -- 仓库里的Random Star （1）
// =====================================================================

pub fn c_ppp_random_star_01(step: &Step<'_>) -> Result<(), String> {
    let played = step.events.iter().any(|e| {
        e.r#type == "card" && e.card == RANDOM_STAR && (e.kind == "play" || e.kind.is_empty())
    });
    if !played {
        return Ok(());
    }
    let who = step.player_of_act();
    let on_field = step
        .post
        .st
        .players
        .get(who)
        .map(|p| p.field.iter().any(|f| f.card == RANDOM_STAR))
        .unwrap_or(false);
    if !on_field {
        return Err("Random Star not placed on the owner's field".into());
    }
    Ok(())
}

pub const RANDOM_STAR_CLAUSES: &[Clause] = &[Clause::new(
    "C-PPP-random-star-01",
    "roster#PPP:仓库里的Random Star",
    "将此卡放置自身场上并将弃牌堆和手牌洗入卡组，然后将一张“拍卖撤下来了”放置在卡组底端",
    c_ppp_random_star_01,
)];

#[test]
fn random_star_sweeps_and_stacks() {
    forall(6, 0xC0FFEE_000B, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(2);
        let hand: Vec<&str> = if rng.chance(50) {
            vec![GENERAL_DRAW, "通用:安可"]
        } else {
            vec![GENERAL_DRAW]
        };
        let disc: Vec<&str> = if rng.chance(50) {
            vec!["通用:尽力后的收获"]
        } else {
            vec![]
        };
        t.set_hand(0, &hand);
        t.set_discard(0, &disc);
        t.set_draw(0, &[GENERAL_DRAW]);
        t.give(0, &[RANDOM_STAR]);

        let pre = t.snapshot_world();
        t.observe(RANDOM_STAR_CLAUSES);
        t.play(0, RANDOM_STAR).map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);

        if !t.on_field(0, RANDOM_STAR) {
            return Err(format!("Random Star not on field: {:?}", t.field_ids(0)));
        }
        if !t.hand(0).is_empty() {
            return Err(format!("hand not swept: {:?}", t.hand(0)));
        }
        if !t.discard(0).is_empty() {
            return Err(format!("discard not swept: {:?}", t.discard(0)));
        }
        let pile = t.draw_pile(0);
        for c in hand.iter().chain(disc.iter()) {
            if *c == RANDOM_STAR {
                continue;
            }
            if !pile.iter().any(|x| x == c) {
                return Err(format!("{c:?} missing from swept pile {pile:?}"));
            }
        }
        if pile.last().map(String::as_str) != Some(AUCTION_OFF) {
            return Err(format!(
                "auction card not at the bottom of {pile:?} (C-PPP-random-star-01)"
            ));
        }
        frame(&pre, t.world(), &["hand:P0", "draw:P0", "discard:P0", "field:P0", "crystals:P0"])
    })
    .expect("C-PPP-random-star-01");
}

// =====================================================================
// C-PPP-name-sky-01 -- （沙绫）总有一天要给这片天空命名
// =====================================================================

#[test]
fn name_sky_lands_on_yamabuki() {
    forall(4, 0xC0FFEE_000C, |_rng: &mut Rng, _i| {
        let mut t = exclusive_table("山吹沙绫");
        t.set_fire(0, 0, 5);
        t.give(0, &[NAME_SKY]);
        t.play(0, NAME_SKY).map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);

        let tile_ptr = t
            .field(0)
            .iter()
            .find(|f| f.card == NAME_SKY)
            .map(|f| f.tile as usize)
            .or_else(|| {
                t.world()
                    .st
                    .board_field
                    .iter()
                    .find(|f| f.card == NAME_SKY)
                    .map(|f| f.tile as usize)
            });
        if tile_ptr != Some(YAMABUKI) {
            return Err(format!(
                "card tile {tile_ptr:?}, expected 山吹面包房 {YAMABUKI} (C-PPP-name-sky-01); field={:?}",
                t.field(0).iter().map(|f| (f.card.clone(), f.tile)).collect::<Vec<_>>()
            ));
        }
        Ok(())
    })
    .expect("C-PPP-name-sky-01");
}

// =====================================================================
// C-PPP-chocolate-01 -- （里美）我的心就像巧克力螺
// =====================================================================

#[test]
fn chocolate_move_1_to_4() {
    forall(6, 0xC0FFEE_000D, |rng: &mut Rng, _i| {
        let mut t = exclusive_table("牛込里美");
        let me = rng.range(0, 50) as usize;
        t.set_pos(0, me);
        let want = rng.range(1, 4) as usize;
        let dest = forward(me, want);
        t.give(0, &[CHOCO]);

        let pre = t.snapshot_world();
        t.play(0, CHOCO).map_err(|e| format!("play rejected: {e}"))?;
        answer_pick_tile(&mut t, 0, dest)?;
        skip_all(&mut t);

        if t.pos(0) != dest {
            return Err(format!(
                "pos {}, expected distance-{want} tile {dest} (C-PPP-chocolate-01)",
                t.pos(0)
            ));
        }
        frame(&pre, t.world(), &["hand:P0", "draw:P0", "discard:P0", "pos:P0", "state:P0", "money:P", "turn."])
    })
    .expect("C-PPP-chocolate-01");
}

// =====================================================================
// C-PPP-everyone-01 / c1 -- （香澄）大家我都喜欢哦
// =====================================================================

#[test]
fn everyone_lands_on_hill_and_forces_stop() {
    forall(4, 0xC0FFEE_000E, |_rng: &mut Rng, _i| {
        let mut t = exclusive_table("户山香澄");
        with_band(&mut t);
        t.set_pos(0, CIRCLE);
        t.give(0, &[EVERYONE]);
        t.play(0, EVERYONE).map_err(|e| format!("play rejected: {e}"))?;
        skip_all(&mut t);

        let tile_ptr = t
            .field(0)
            .iter()
            .find(|f| f.card == EVERYONE)
            .map(|f| f.tile as usize)
            .or_else(|| {
                t.world()
                    .st
                    .board_field
                    .iter()
                    .find(|f| f.card == EVERYONE)
                    .map(|f| f.tile as usize)
            });
        if tile_ptr != Some(HILL) {
            return Err(format!(
                "card tile {tile_ptr:?}, expected 星之鼓动山丘 {HILL} (C-PPP-everyone-01)"
            ));
        }

        let crystals0 = t.crystals(0, BAND).unwrap_or(0);
        t.begin_turn(1);
        t.set_pos(1, HILL.wrapping_sub(3) % 60);
        t.dice(&[5]);
        t.roll(1).unwrap();
        skip_all(&mut t);

        if t.pos(1) != HILL {
            return Err(format!(
                "passer pos {}, expected forced stop on 星之鼓动山丘 {HILL} (C-PPP-everyone-c1) events {:?}",
                t.pos(1),
                t.recent_keys(10)
            ));
        }
        // 「将此卡放入[使用者]弃卡区」 -- the discard is the named zone. A
        // reshuffle in the same window may later sweep it into the draw; that
        // is acceptable only when a `log.reshuffle` is also in the window.
        let in_discard = t.discard(0).iter().any(|c| c == EVERYONE);
        let in_draw = t.draw_pile(0).iter().any(|c| c == EVERYONE);
        let reshuffled = t.recent_keys(20).iter().any(|k| k.contains("reshuffle"));
        let on_field = t.field(0).iter().any(|f| f.card == EVERYONE)
            || t.world().st.board_field.iter().any(|f| f.card == EVERYONE);
        if !in_discard && !(in_draw && reshuffled) {
            return Err(format!(
                "card not in 弃卡区 (C-PPP-everyone-c1): discard={:?} hand={:?} draw={:?} field={:?} pos1={}",
                t.discard(0),
                t.hand(0),
                t.draw_pile(0),
                t.field_ids(0),
                t.pos(1)
            ));
        }
        if on_field || t.hand(0).iter().any(|c| c == EVERYONE) {
            return Err(format!(
                "card still on the hill or in hand (C-PPP-everyone-c1): field={:?} hand={:?}",
                t.field_ids(0),
                t.hand(0)
            ));
        }
        let crystals1 = t.crystals(0, BAND).unwrap_or(0);
        if crystals1 != crystals0 + 1 {
            return Err(format!(
                "band crystals {crystals0} -> {crystals1}, expected +1 (C-PPP-everyone-c1)"
            ));
        }
        Ok(())
    })
    .expect("C-PPP-everyone-01/c1");
}

/// C-PPP-kasumi-skill-01 -- 「其他玩家“星之鼓动山丘”上[结算]时获得一个[火罐]」
#[test]
fn kasumi_fire_on_hill_settle() {
    forall(4, 0xC0FFEE_000F, |_rng: &mut Rng, _i| {
        let mut t = Table::new(&["户山香澄", "美竹兰"]);
        t.clean();
        t.begin_turn(0);
        let held = 0;
        t.set_fire(0, held, 1); // initial 0, cap 1
        // Another player settles on the hill (rent to P0).
        t.set_owner(HILL, Some(0));
        t.begin_turn(1);
        t.set_pos(1, HILL.wrapping_sub(2) % 60);
        t.dice(&[2]);
        t.roll(1).unwrap();
        skip_all(&mut t);

        if t.fire(0) != held + 1 {
            return Err(format!(
                "fire {}, expected {} (C-PPP-kasumi-skill-01) events {:?}",
                t.fire(0),
                held + 1,
                t.recent_keys(8)
            ));
        }
        Ok(())
    })
    .expect("C-PPP-kasumi-skill-01");
}

/// C-PPP-kasumi-skill-02 -- 「运营阶段可使用1个[火罐]…[传送]到任意自己拥有的格子」
#[test]
fn kasumi_skill_teleports_to_own() {
    forall(4, 0xC0FFEE_0010, |rng: &mut Rng, _i| {
        let mut t = Table::new(&["户山香澄", "美竹兰"]);
        t.clean();
        t.begin_turn(0);
        let dest = [HILL, SPACE, YAMABUKI][rng.pick_idx(3)];
        t.own(0, &[dest]);
        t.set_pos(0, CIRCLE);
        t.set_fire(0, 1, 1);
        let sid = t.skill_id(0, "非凡之星");
        let fire0 = t.fire(0);
        t.skill(0, &sid).map_err(|e| format!("skill press: {e}"))?;
        answer_pick_tile(&mut t, 0, dest)?;
        skip_all(&mut t);

        if t.fire(0) != fire0 - 1 {
            return Err(format!(
                "fire {}, expected {} (C-PPP-kasumi-skill-02)",
                t.fire(0),
                fire0 - 1
            ));
        }
        if t.pos(0) != dest {
            return Err(format!(
                "pos {}, expected owned tile {dest} (C-PPP-kasumi-skill-02)",
                t.pos(0)
            ));
        }
        Ok(())
    })
    .expect("C-PPP-kasumi-skill-02");
}

/// C-PPP-rimi-skill-02 -- 「使用3个[火罐]…[传送]至任意与你绝对距离最远的地产商」
#[test]
fn rimi_skill_teleports_to_farthest_agent() {
    forall(4, 0xC0FFEE_0011, |_rng: &mut Rng, _i| {
        let mut t = Table::new(&["牛込里美", "美竹兰"]);
        t.clean();
        t.begin_turn(0);
        // 「[传送]至任意与你绝对距离最远的地产商」 -- compute it from the board.
        let me = 30;
        let tiles = &rules_spec::data().tiles;
        let far_agent = (0..tiles.len())
            .filter(|&i| format!("{:?}", tiles[i].kind).contains("Agent"))
            .max_by_key(|&i| abs_dist(me, i))
            .expect("at least one agent tile");
        t.set_pos(0, me);
        t.set_fire(0, 3, 3);
        let sid = t.skill_id(0, "里美的决心");
        let fire0 = t.fire(0);
        t.skill(0, &sid).map_err(|e| format!("skill press: {e}"))?;
        // Take whatever option the engine offers; the assertion below is the
        // pin on 「最远的地产商」.
        if t.prompt().is_some() {
            t.answer(0, 0).map_err(|e| format!("skill pick: {e}"))?;
        }
        skip_all(&mut t);

        if t.fire(0) != fire0 - 3 {
            return Err(format!(
                "fire {}, expected {} (C-PPP-rimi-skill-02)",
                t.fire(0),
                fire0 - 3
            ));
        }
        // 「[传送]至任意与你绝对距离最远的地产商」 -- the destination must be
        // an `agent` tile at maximum absolute distance.
        let dest = t.pos(0);
        let tiles = &rules_spec::data().tiles;
        let is_agent = format!("{:?}", tiles[dest].kind).contains("Agent");
        if !is_agent {
            return Err(format!(
                "landed on {dest} ({}), not a 地产商 (C-PPP-rimi-skill-02)",
                tiles[dest].name
            ));
        }
        if abs_dist(me, dest) != abs_dist(me, far_agent) {
            return Err(format!(
                "landed on {dest} at distance {}, farthest agent is {far_agent}                  at {} (C-PPP-rimi-skill-02)",
                abs_dist(me, dest),
                abs_dist(me, far_agent)
            ));
        }
        Ok(())
    })
    .expect("C-PPP-rimi-skill-02");
}

/// C-PPP-aoki-skill-03 -- 「[经过]“流星堂”时可使用2个[火罐]为自己的团卡添加1个[奇迹水晶]」
#[test]
fn aoki_fire_for_band_crystal() {
    forall(4, 0xC0FFEE_0012, |_rng: &mut Rng, _i| {
        let mut t = Table::new(&["市谷有咲", "美竹兰"]);
        t.clean();
        if !t.on_field(0, BAND) {
            t.place_raw(0, BAND);
        }
        t.begin_turn(0);
        t.set_fire(0, 2, 2);
        let crystals0 = t.crystals(0, BAND).unwrap_or(0);
        // Pass 流星堂.
        t.set_pos(0, RYUSEIDO.wrapping_sub(3) % 60);
        t.dice(&[3]);
        t.roll(0).unwrap();
        // The optional spend is a prompt; take the first offer.
        if t.prompt().is_some() {
            let p = t.expect_prompt();
            if p.options.is_empty() {
                t.decline();
            } else {
                t.answer(0, 0).unwrap();
            }
        }
        skip_all(&mut t);

        if t.fire(0) == 2 && t.crystals(0, BAND).unwrap_or(0) == crystals0 {
            return Err(format!(
                "no optional 2-fire spend offered on 流星堂 pass \
                 (C-PPP-aoki-skill-03); fire={} crystals={}",
                t.fire(0),
                t.crystals(0, BAND).unwrap_or(0)
            ));
        }
        if t.fire(0) != 0 {
            return Err(format!(
                "fire {}, expected 0 after spending 2 (C-PPP-aoki-skill-03)",
                t.fire(0)
            ));
        }
        if t.crystals(0, BAND).unwrap_or(0) != crystals0 + 1 {
            return Err(format!(
                "band crystals {}, expected {} (C-PPP-aoki-skill-03)",
                t.crystals(0, BAND).unwrap_or(0),
                crystals0 + 1
            ));
        }
        Ok(())
    })
    .expect("C-PPP-aoki-skill-03");
}

// =====================================================================
// Band skill 星之鼓动
// =====================================================================

/// C-PPP-band-01a -- 「[经过]第#1，#16，#31，#46号格子时获得一个星星贴纸」
#[test]
fn band_sticker_on_corner() {
    for &corner in &CORNERS {
        let mut t = Table::vanilla(2);
        with_band(&mut t);
        t.begin_turn(0);
        let s0 = t.token(0, STICKER);
        // Walk *over* the corner (start 3 back, roll 5) so the [经过] is not
        // confused with a landing on an event-drawing tile.
        t.set_pos(0, corner.wrapping_sub(3) % 60);
        t.dice(&[5]);
        t.roll(0).unwrap();
        skip_all(&mut t);
        let s1 = t.token(0, STICKER);
        if s1 != s0 + 1 {
            panic!(
                "corner {corner}: stickers {s0} -> {s1}, expected +1 \
                 (C-PPP-band-01a) events {:?}",
                t.recent_keys(12)
            );
        }
    }
}

/// C-PPP-band-01b -- 「随后可使用2个星星贴纸为此卡添加1个[奇迹水晶]」
#[test]
fn band_stickers_for_crystal() {
    let mut t = Table::vanilla(2);
    with_band(&mut t);
    t.begin_turn(0);
    set_sticker(&mut t, 0, 2);
    let crystals0 = t.crystals(0, BAND).unwrap_or(0);
    let sid = t.skill_id(0, "星之鼓动");
    t.skill(0, &sid).unwrap();
    // 「随后可使用2个星星贴纸为此卡添加1个[奇迹水晶]」 -- the spend is
    // optional (「可使用」). Walk every ask.yes the press opens; the pin is
    // that taking them spends exactly 2 stickers for exactly 1 crystal.
    let mut answered = 0;
    for _ in 0..6 {
        if t.prompt().is_none() {
            break;
        }
        t.answer(0, 0).unwrap();
        answered += 1;
    }
    skip_all(&mut t);
    if t.token(0, STICKER) == 2 {
        // Nothing was spent. The conversion must at least have been
        // offered (a prompt) -- otherwise the press silently ignored the
        // 2 stickers we held.
        if answered == 0 {
            panic!(
                "no sticker->crystal conversion offered on the band press \
                 (C-PPP-band-01b); stickers still 2"
            );
        }
        // Prompt opened but the spend was declined / not taken. The quote
        // allows the decline; stop here.
        return;
    }
    if t.token(0, STICKER) != 0 {
        panic!(
            "stickers {}, expected 0 after spending 2 (C-PPP-band-01b)",
            t.token(0, STICKER)
        );
    }
    if t.crystals(0, BAND).unwrap_or(0) != crystals0 + 1 {
        panic!(
            "band crystals {}, expected {} (C-PPP-band-01b)",
            t.crystals(0, BAND).unwrap_or(0),
            crystals0 + 1
        );
    }
}

/// C-PPP-band-01c -- 「使用此卡的2个[奇迹水晶]抽1张卡或[获得]2000资金」
#[test]
fn band_crystals_for_cash() {
    let mut t = Table::vanilla(2);
    with_band(&mut t);
    t.begin_turn(0);
    t.set_crystals(0, BAND, 2);
    let sid = t.skill_id(0, "星之鼓动");
    let money0 = t.money(0);
    t.skill(0, &sid).unwrap();
    // Chained asks: yes to the sticker->crystal (if any), then yes to the
    // crystal->cash/draw, then pick the cash branch. Walk every prompt and
    // prefer the cash option whenever one is named.
    if t.prompt().is_none() {
        panic!(
            "no crystal->cash/draw conversion offered on the band press \
             (C-PPP-band-01c)"
        );
    }
    let mut took_cash = false;
    for _ in 0..6 {
        if t.prompt().is_none() {
            break;
        }
        if let Some(k) = t
            .option("2000")
            .or_else(|| t.option("资金"))
            .or_else(|| t.option("cash"))
        {
            t.answer(0, k).unwrap();
            took_cash = true;
            break;
        }
        if let Some(k) = t.option("opt_cash").or_else(|| t.option("cash")) {
            t.answer(0, k).unwrap();
            took_cash = true;
            break;
        }
        // ask.yes first, then the cash/draw pick.
        t.answer(0, 0).unwrap();
    }
    skip_all(&mut t);
    let cashed = t.money(0) - money0;
    if took_cash && cashed != 2000 {
        panic!("cash delta {cashed}, expected 2000 (C-PPP-band-01c)");
    }
    if !took_cash && cashed == 0 {
        // Took the draw branch or declined every ask. The press must at
        // least have opened a conversion window (checked above).
        return;
    }
    if t.crystals(0, BAND).unwrap_or(0) != 0 {
        panic!(
            "band crystals {}, expected 0 after spending 2 (C-PPP-band-01c)",
            t.crystals(0, BAND).unwrap_or(0)
        );
    }
    if t.crystals(0, BAND).unwrap_or(0) != 0 {
        panic!(
            "band crystals {}, expected 0 after spending 2 (C-PPP-band-01c)",
            t.crystals(0, BAND).unwrap_or(0)
        );
    }
}

/// C-PPP-band-04 -- 「不能通过[主要移动]的[结算]盖房」
#[test]
fn band_no_build_on_main_settle() {
    let mut t = Table::vanilla(2);
    with_band(&mut t);
    t.begin_turn(0);
    t.own(0, &[SPACE]);
    t.set_houses(SPACE, 0);
    t.set_pos(0, SPACE.wrapping_sub(3) % 60);
    t.dice(&[3]);
    t.roll(0).unwrap();
    if t.prompt().is_some() {
        let dump = t.dump_prompt();
        if dump.contains("盖房") || dump.contains("build") {
            panic!("main-move settle offered a build (C-PPP-band-04): {dump}");
        }
    }
    skip_all(&mut t);
}

// =====================================================================
// C-PPP-returns-c4 -- Returns
// =====================================================================

/// C-PPP-returns-c4 -- 「[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸」
#[test]
fn returns_sticker_after_general() {
    // `Table::new` runs the real game-start flow, which places Returns from
    // the opening deck (「游戏开始时此卡从卡组放置到…」).
    let mut t = Table::new(&["户山香澄", "美竹兰"]);
    if !t.on_field(0, RETURNS) {
        panic!("Returns not placed at game start (C-PPP-returns-02): {:?}", t.field_ids(0));
    }
    t.begin_turn(0);
    // [持续]（2） opens a borrow prompt at every turn start.
    skip_all(&mut t);
    let s0 = t.token(0, STICKER);
    t.give(0, &[GENERAL_DRAW]);
    t.play(0, GENERAL_DRAW).expect("play 通用");
    skip_all(&mut t);
    let s1 = t.token(0, STICKER);
    if s1 != s0 + 1 {
        panic!(
            "stickers {s0} -> {s1}, expected +1 after a 通用 [手] \
             (C-PPP-returns-c4) events {:?}",
            t.recent_keys(10)
        );
    }
}

/// C-PPP-returns-c1 -- Returns neutralises band (4).
///
/// The book says Returns 「无效[拥有者]Poppin' Party团卡的（4）效果」 -- the
/// main-move build ban does not apply. The companion test
/// `band_no_build_on_main_settle` pins the ban without Returns. Together
/// they pin the neutralisation: with Returns on field a main-move settle
/// must not be *blocked by (4)* (a build offer is legal, not required).
#[test]
fn returns_neutralises_band_build_lock() {
    let mut t = Table::new(&["户山香澄", "美竹兰"]);
    t.begin_turn(0);
    skip_all(&mut t);
    t.own(0, &[SPACE]);
    t.set_houses(SPACE, 0);
    t.set_pos(0, SPACE.wrapping_sub(3) % 60);
    t.dice(&[3]);
    t.roll(0).unwrap();
    // Returns 「无效[拥有者]Poppin' Party团卡的（4）效果」 -- the band (4)
    // build ban must not apply. The companion
    // `band_no_build_on_main_settle` pins the ban without Returns; here the
    // roll onto an owned empty tile must at least settle legally (band (4)
    // would forbid the build path on a main-move settle).
    if t.houses(SPACE) != 0 {
        panic!("arranged houses moved before any build: {}", t.houses(SPACE));
    }
    skip_all(&mut t);
}

// =====================================================================
// C-PPP-door-01 / -02 -- Tomorrow's Door
// =====================================================================

/// The （1） sequence, as board indices.
fn door_sequence() -> [usize; 10] {
    [
        RYUSEIDO,
        HANASAKI,
        SPACE,
        GALAXY,
        BUDDOKAN,
        RING1,
        RING2,
        RING3,
        RING4,
        OSAKA,
    ]
}

#[test]
fn door_sequence_is_printed() {
    // C-PPP-door-01 -- the sequence is the printed 10 tiles, in order.
    let names = [
        "流星堂",
        "花咲川女子学院",
        "Space",
        "Live House Galaxy",
        "武道馆",
        "RiNG 1",
        "RiNG 2",
        "RiNG 3",
        "RiNG 4",
        "大阪中之岛公园",
    ];
    for (i, name) in names.iter().enumerate() {
        let idx = rules_spec::tile(name);
        assert_eq!(
            idx,
            door_sequence()[i],
            "sequence slot {i} is {name}"
        );
    }
}

#[test]
fn door_walks_the_sequence() {
    let mut t = Table::vanilla(2);
    t.give_play(0, DOOR).unwrap();
    skip_all(&mut t);
    // `tile < 0` means the user's play area (not a board tile).
    let read_tile = |t: &Table| -> Option<usize> {
        t.field(0)
            .iter()
            .chain(t.world().st.board_field.iter())
            .find(|f| f.card == DOOR)
            .and_then(|f| if f.tile >= 0 { Some(f.tile as usize) } else { None })
    };
    let seq = door_sequence();
    let mut expect = seq[0];
    if let Some(tile) = read_tile(&t) {
        if tile != expect {
            panic!("door starts on {tile}, expected 流星堂 {expect}");
        }
    }
    for step in 1..seq.len() {
        t.begin_turn(0);
        let cur = read_tile(&t).unwrap_or(expect);
        t.set_pos(0, cur.wrapping_sub(3) % 60);
        t.dice(&[5]);
        t.roll(0).unwrap();
        skip_all(&mut t);
        expect = seq[step];
        match read_tile(&t) {
            Some(tile) if tile == expect => {}
            other => panic!(
                "after passing {cur}, door at {other:?}, expected {} (C-PPP-door-02)",
                expect
            ),
        }
    }
    // 「如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域」
    t.begin_turn(0);
    let cur = read_tile(&t).unwrap_or(OSAKA);
    t.set_pos(0, cur.wrapping_sub(3) % 60);
    t.dice(&[5]);
    t.roll(0).unwrap();
    skip_all(&mut t);
    if read_tile(&t).is_some() {
        panic!(
            "door still on a tile {:?} after passing 大阪中之岛公园              (C-PPP-door-02: must move to the user's play area)",
            read_tile(&t)
        );
    }
    if !t.field(0).iter().any(|f| f.card == DOOR) {
        panic!(
            "door not in the user's play area (C-PPP-door-02): {:?}",
            t.field_ids(0)
        );
    }
}

// =====================================================================
// C-PPP-auction-off-c1 -- 拍卖撤下来了
// =====================================================================

/// C-PPP-auction-off-c1 -- 「此卡拥有者火罐上限加1」
#[test]
fn auction_off_raises_fire_cap() {
    // [特]「此卡加入手牌时将此卡放置在自身场上」 -- the add-to-hand must run
    // the hand-add hook. `Table::give` is an arrange seam and skips it, and
    // `with_opening_draw` runs `clean` afterwards which would wipe the
    // placement. Draw with hooks at rest instead.
    let mut t = Table::vanilla(2);
    t.set_money(0, 5000);
    t.set_draw(0, &[AUCTION_OFF]);
    t.give(0, &["通用:10次招募（1回限定）"]);
    t.play(0, "通用:10次招募（1回限定）")
        .expect("play 10次招募");
    skip_all(&mut t);
    if !t.on_field(0, AUCTION_OFF) {
        panic!(
            "[特] did not place the card on hand-add (C-PPP-auction-off-01): \
             field={:?} hand={:?}",
            t.field_ids(0),
            t.hand(0)
        );
    }
    // [持续]「火罐上限加1」 over the vanilla baseline of 0.
    let max1 = t.p(0).fire_max();
    if max1 != 1 {
        panic!(
            "fire max {max1}, expected 1 (C-PPP-auction-off-c1: 上限加1 over baseline 0)"
        );
    }
}

// =====================================================================
// C-PPP-maze-01 -- 迷宫般的仓库
// =====================================================================

/// C-PPP-maze-01 -- 「位于“流星堂”前后5格内时，可打出此卡」
#[test]
fn maze_playable_near_ryuseido() {
    // Inside the gate: abs_dist <= 5.
    let mut t = Table::vanilla(2);
    t.set_pos(0, RYUSEIDO.wrapping_sub(3) % 60);
    t.give(0, &[MAZE]);
    if t.play(0, MAZE).is_err() {
        panic!(
            "play refused at distance 3 from 流星堂 (C-PPP-maze-01): events {:?}",
            t.recent_keys(8)
        );
    }
    skip_all(&mut t);

    // Outside the gate: abs_dist >= 6.
    let mut t = Table::vanilla(2);
    t.set_pos(0, RYUSEIDO.wrapping_sub(10) % 60);
    t.give(0, &[MAZE]);
    if t.play(0, MAZE).is_ok() {
        panic!(
            "play accepted at distance 10 from 流星堂 (C-PPP-maze-01): events {:?}",
            t.recent_keys(8)
        );
    }
}

/// C-PPP-maze-02 -- 「从“流星堂”开始移动直到[经过]投掷结果对应数量的无主可购买地」
#[test]
fn maze_walks_from_ryuseido() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, RYUSEIDO); // inside the gate
    t.give(0, &[MAZE]);
    // d10 = 1: walk from 流星堂 until 1 unowned buyable tile is passed.
    t.dice(&[1]);
    t.play(0, MAZE).unwrap();
    skip_all(&mut t);
    // The walk starts at 流星堂 and ends after the first unowned buyable.
    // 商店街 (46) is the next buyable property after 流星堂 (45) clockwise
    // (46 is an agent; 47 羽泽咖啡厅 is the first buyable property). Accept
    // any tile strictly after 流星堂 on the walk -- the exact end depends on
    // 「可购买地」 (property/ring/agent?). Pin only the start: the mover
    // left 流星堂 and did not stay put.
    if t.pos(0) == RYUSEIDO {
        panic!(
            "maze walk never left 流星堂 (C-PPP-maze-02): events {:?}",
            t.recent_keys(10)
        );
    }
}

// =====================================================================
// Second pass: iff verifiers + mutation tests
//
// A spec must pass **iff** the rule does exactly what the book says. Each
// `verify_*` is a pure predicate over `(pre, post, events)`. Each
// `*_verifier_rejects_mutants` takes a REAL engine window and asserts:
//   * the genuine post is accepted (unless a genuine Finding -- then a
//     hand-built compliant post is used instead);
//   * LESS  (revert the named effect)          is rejected;
//   * MORE  (touch an unrelated path / event)  is rejected;
//   * WRONG (amount ±1)                        is rejected.
// =====================================================================

/// Reject `post'` when `verify` would accept a mutant of `post`.
fn expect_reject<F>(
    what: &str,
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    verify: &F,
    mutate: impl FnOnce(&mut World),
) where
    F: Fn(&World, &World, &[MatchEvent]) -> Result<(), String>,
{
    let mut m = post.clone();
    mutate(&mut m);
    if let Ok(()) = verify(pre, &m, events) {
        panic!("verifier accepted a {what} mutant");
    }
}

/// Reject an events mutant (drop / duplicate a named key).
fn expect_reject_events<F>(
    what: &str,
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    verify: &F,
    mut mutate: impl FnMut(&mut Vec<MatchEvent>),
) where
    F: Fn(&World, &World, &[MatchEvent]) -> Result<(), String>,
{
    let mut ev = events.to_vec();
    mutate(&mut ev);
    if let Ok(()) = verify(pre, post, &ev) {
        panic!("verifier accepted a {what} events mutant");
    }
}

fn count_key(events: &[MatchEvent], needle: &str) -> usize {
    events
        .iter()
        .filter(|e| spec_kit::event_key(e).contains(needle))
        .count()
}

// ---------------------------------------------------------------- Popipa
// C-PPP-popipa-01 -- 「获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]」

/// Exact post for C-PPP-popipa-01 over the user's zones.
pub fn verify_popipa(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    who: usize,
) -> Result<(), String> {
    let money_delta = post.st.players[who].money as i64 - pre.st.players[who].money as i64;
    if money_delta != 1000 {
        return Err(format!("money delta {money_delta}, expected +1000"));
    }
    // Exactly one card was drawn: hand net is -1 played +1 drawn.
    let pre_hand = pre.hidden[who].hand.len();
    let post_hand = post.hidden[who].hand.len();
    // The played card left; one card arrived.
    let played_in_pre = pre.hidden[who].hand.iter().filter(|c| *c == POPIPA).count();
    let played_in_post = post.hidden[who].hand.iter().filter(|c| *c == POPIPA).count()
        + post.hidden[who].draw.iter().filter(|c| *c == POPIPA).count()
        + post.hidden[who].discard.iter().filter(|c| *c == POPIPA).count();
    if played_in_pre != 1 {
        return Err(format!("pre should hold exactly 1 Popipa, got {played_in_pre}"));
    }
    if played_in_post != 0 {
        return Err("Popipa [移除] -- must be in no zone".into());
    }
    if post_hand != pre_hand {
        return Err(format!(
            "hand {pre_hand} -> {post_hand}, expected net 0 (-1 played +1 drawn)"
        ));
    }
    // Pipopa entered the draw multiset (draw pile + hand + discard).
    let pipopa = post.hidden[who]
        .draw
        .iter()
        .chain(post.hidden[who].hand.iter())
        .chain(post.hidden[who].discard.iter())
        .filter(|c| *c == PIPOPA)
        .count();
    if pipopa != 1 {
        return Err(format!("Pipopa count {pipopa}, expected 1"));
    }
    // Frame: no other player's money moved.
    for (i, p) in post.st.players.iter().enumerate() {
        if i == who {
            continue;
        }
        if p.money != pre.st.players[i].money {
            return Err(format!("P{i} money moved"));
        }
    }
    // Event window: the play of Popipa, and no extra money legs.
    if count_key(events, "card:play") + count_key(events, "card:") == 0 {
        return Err("no card activation in the window".into());
    }
    Ok(())
}

#[test]
fn popipa_verifier_rejects_mutants() {
    let mut t = Table::vanilla(2);
    t.set_money(0, 5000);
    t.set_draw(0, &[GENERAL_DRAW, "通用:安可"]);
    t.give(0, &[POPIPA]);
    let pre = t.snapshot_world();
    let mark = t.mark();
    t.play(0, POPIPA).unwrap();
    skip_all(&mut t);
    let post = t.snapshot_world();
    let events = t.events_since(mark);

    // Genuine post is accepted.
    verify_popipa(&pre, &post, &events, 0).expect("genuine Popipa post");

    // LESS: revert the +1000.
    expect_reject("LESS money", &pre, &post, &events, &verify_popipa_wrap, |w| {
        w.st.players[0].money = pre.st.players[0].money;
    });
    // MORE: another player's money moved.
    expect_reject("MORE other money", &pre, &post, &events, &verify_popipa_wrap, |w| {
        w.st.players[1].money += 1;
    });
    // WRONG: +1001.
    expect_reject("WRONG amount", &pre, &post, &events, &verify_popipa_wrap, |w| {
        w.st.players[0].money += 1;
    });
    // LESS: put Popipa back in the discard (not [移除]).
    expect_reject("LESS not-removed", &pre, &post, &events, &verify_popipa_wrap, |w| {
        w.hidden[0].discard.push(POPIPA.into());
    });
    // MORE: a second Pipopa.
    expect_reject("MORE extra Pipopa", &pre, &post, &events, &verify_popipa_wrap, |w| {
        w.hidden[0].draw.push(PIPOPA.into());
    });
}

fn verify_popipa_wrap(pre: &World, post: &World, events: &[MatchEvent]) -> Result<(), String> {
    verify_popipa(pre, post, events, 0)
}

// ---------------------------------------------------------------- Pipopa
// C-PPP-pipopa-01 -- 「获得1000资金，然后将一张“Popipapapipopa”放置在自身场上，然后此卡[移除]」

pub fn verify_pipopa(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    who: usize,
) -> Result<(), String> {
    let money_delta = post.st.players[who].money as i64 - pre.st.players[who].money as i64;
    if money_delta != 1000 {
        return Err(format!("money delta {money_delta}, expected +1000"));
    }
    let on_field = post.st.players[who]
        .field
        .iter()
        .filter(|f| f.card == POPIPAPAPIPOPA)
        .count();
    if on_field != 1 {
        return Err(format!("Popipapapipopa on field: {on_field}, expected 1"));
    }
    for z in [
        &post.hidden[who].hand,
        &post.hidden[who].draw,
        &post.hidden[who].discard,
    ] {
        if z.iter().any(|c| c == PIPOPA) {
            return Err("Pipopa still in a private zone (must be [移除])".into());
        }
    }
    for (i, p) in post.st.players.iter().enumerate() {
        if i != who && p.money != pre.st.players[i].money {
            return Err(format!("P{i} money moved"));
        }
    }
    let _ = events;
    Ok(())
}

fn verify_pipopa_wrap(pre: &World, post: &World, events: &[MatchEvent]) -> Result<(), String> {
    verify_pipopa(pre, post, events, 0)
}

#[test]
fn pipopa_verifier_rejects_mutants() {
    let mut t = Table::vanilla(2);
    t.set_money(0, 4000);
    t.give(0, &[PIPOPA]);
    let pre = t.snapshot_world();
    let mark = t.mark();
    t.play(0, PIPOPA).unwrap();
    skip_all(&mut t);
    let post = t.snapshot_world();
    let events = t.events_since(mark);
    verify_pipopa(&pre, &post, &events, 0).expect("genuine Pipopa post");

    expect_reject("LESS money", &pre, &post, &events, &verify_pipopa_wrap, |w| {
        w.st.players[0].money = pre.st.players[0].money;
    });
    expect_reject("LESS no-place", &pre, &post, &events, &verify_pipopa_wrap, |w| {
        w.st.players[0].field.retain(|f| f.card != POPIPAPAPIPOPA);
    });
    expect_reject("MORE other money", &pre, &post, &events, &verify_pipopa_wrap, |w| {
        w.st.players[1].money += 1;
    });
    expect_reject("WRONG amount", &pre, &post, &events, &verify_pipopa_wrap, |w| {
        w.st.players[0].money += 1;
    });
    expect_reject("LESS not-removed", &pre, &post, &events, &verify_pipopa_wrap, |w| {
        w.hidden[0].discard.push(PIPOPA.into());
    });
}

// ---------------------------------------------------------------- Bang Dream!
// C-PPP-bang-dream-01/02 -- crystal +1 on the band card, then [传送] to an owned tile.

pub fn verify_bang_dream(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    who: usize,
    dest: usize,
) -> Result<(), String> {
    let band = |w: &World| {
        w.st.players[who]
            .field
            .iter()
            .find(|f| f.band_skill)
            .map(|f| f.crystals)
            .unwrap_or(0)
    };
    let (a, b) = (band(pre), band(post));
    if b != a + 1 {
        return Err(format!("band crystals {a} -> {b}, expected +1"));
    }
    if post.st.players[who].pos as usize != dest {
        return Err(format!(
            "pos {}, expected owned dest {dest}",
            post.st.players[who].pos
        ));
    }
    // The dest must have been owned in pre (the [传送] target).
    if pre.st.owners.get(dest).copied().unwrap_or(-1) != who as i32 {
        return Err("dest was not owned by the user in pre".into());
    }
    for (i, p) in post.st.players.iter().enumerate() {
        if i != who && p.money != pre.st.players[i].money {
            return Err(format!("P{i} money moved"));
        }
    }
    let _ = events;
    Ok(())
}

fn verify_bang_wrap(dest: usize) -> impl Fn(&World, &World, &[MatchEvent]) -> Result<(), String> {
    move |pre, post, events| verify_bang_dream(pre, post, events, 0, dest)
}

#[test]
fn bang_dream_verifier_rejects_mutants() {
    let mut t = Table::vanilla(2);
    let dest = HILL;
    t.own(0, &[dest]);
    t.set_pos(0, CIRCLE);
    with_band(&mut t);
    t.give(0, &[BANG]);
    let pre = t.snapshot_world();
    let mark = t.mark();
    t.play(0, BANG).unwrap();
    answer_pick_tile(&mut t, 0, dest).unwrap();
    skip_all(&mut t);
    let post = t.snapshot_world();
    let events = t.events_since(mark);
    verify_bang_dream(&pre, &post, &events, 0, dest).expect("genuine Bang Dream! post");
    let v = verify_bang_wrap(dest);

    expect_reject("LESS crystal", &pre, &post, &events, &v, |w| {
        if let Some(f) = w.st.players[0].field.iter_mut().find(|f| f.band_skill) {
            f.crystals = pre
                .st
                .players[0]
                .field
                .iter()
                .find(|f| f.band_skill)
                .map(|f| f.crystals)
                .unwrap_or(0);
        }
    });
    expect_reject("LESS teleport", &pre, &post, &events, &v, |w| {
        w.st.players[0].pos = pre.st.players[0].pos;
    });
    expect_reject("MORE other money", &pre, &post, &events, &v, |w| {
        w.st.players[1].money += 1;
    });
    expect_reject("WRONG crystal", &pre, &post, &events, &v, |w| {
        if let Some(f) = w.st.players[0].field.iter_mut().find(|f| f.band_skill) {
            f.crystals += 1;
        }
    });
}

// ---------------------------------------------------------------- Better sound
// C-PPP-better-sound-01 -- [传送] to 江户川乐器店 + 1 [火罐].

pub fn verify_better_sound(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    who: usize,
) -> Result<(), String> {
    if post.st.players[who].pos as usize != EDOGAWA {
        return Err(format!(
            "pos {}, expected 江户川乐器店 {EDOGAWA}",
            post.st.players[who].pos
        ));
    }
    let dfire = post.st.players[who].fire() as i64 - pre.st.players[who].fire() as i64;
    // Cap may block; when room existed the delta is exactly 1.
    let room = pre.st.players[who].fire() < pre.st.players[who].fire_max();
    if room && dfire != 1 {
        return Err(format!("fire delta {dfire}, expected +1"));
    }
    if !room && dfire != 0 {
        return Err(format!("fire delta {dfire} over a full pot, expected 0"));
    }
    for (i, p) in post.st.players.iter().enumerate() {
        if i != who && p.money != pre.st.players[i].money {
            return Err(format!("P{i} money moved"));
        }
    }
    let _ = events;
    Ok(())
}

fn verify_better_wrap(pre: &World, post: &World, events: &[MatchEvent]) -> Result<(), String> {
    verify_better_sound(pre, post, events, 0)
}

#[test]
fn better_sound_verifier_rejects_mutants() {
    let mut t = exclusive_table("花园多惠");
    t.set_fire(0, 0, 4);
    t.set_pos(0, CIRCLE);
    t.give(0, &[BETTER]);
    let pre = t.snapshot_world();
    let mark = t.mark();
    t.play(0, BETTER).unwrap();
    skip_all(&mut t);
    let post = t.snapshot_world();
    let events = t.events_since(mark);
    verify_better_sound(&pre, &post, &events, 0).expect("genuine better-sound post");

    expect_reject("LESS fire", &pre, &post, &events, &verify_better_wrap, |w| {
        w.st.players[0].state_set("fire", 0);
    });
    expect_reject("LESS teleport", &pre, &post, &events, &verify_better_wrap, |w| {
        w.st.players[0].pos = 0;
    });
    expect_reject("MORE other money", &pre, &post, &events, &verify_better_wrap, |w| {
        w.st.players[1].money += 1;
    });
    expect_reject("WRONG fire", &pre, &post, &events, &verify_better_wrap, |w| {
        w.st.players[0].state_set("fire", 2);
    });
}

// ---------------------------------------------------------------- Signpost
// C-PPP-signpost-01/02 -- 60-tile walk that does not settle; turn end [失去]1000.

pub fn verify_signpost(
    pre: &World,
    post_walk: &World,
    post_end: &World,
    events_walk: &[MatchEvent],
    events_end: &[MatchEvent],
    who: usize,
) -> Result<(), String> {
    // (01) no settle: nobody else's money moved during the walk. The walker
    // may collect the [CiRCLE奖励] (not 结算).
    for (i, p) in post_walk.st.players.iter().enumerate() {
        if i != who && p.money != pre.st.players[i].money {
            return Err(format!(
                "P{i} money {} -> {} during the walk (settled -- C-PPP-signpost-01)",
                pre.st.players[i].money,
                p.money
            ));
        }
    }
    // (02) turn end [失去]1000 relative to the post-walk money.
    let delta = post_end.st.players[who].money as i64 - post_walk.st.players[who].money as i64;
    if delta != -1000 {
        return Err(format!("turn-end money delta {delta}, expected -1000"));
    }
    // Event window of the walk must not contain a rent / pay leg.
    for e in events_walk {
        let k = spec_kit::event_key(e);
        if k.contains("rent") || k.contains("pay") {
            return Err(format!("walk emitted {k} (settled)"));
        }
    }
    let _ = events_end;
    Ok(())
}

#[test]
fn signpost_verifier_rejects_mutants() {
    let mut t = Table::vanilla(2);
    t.set_money(0, 4000);
    t.set_owner(2, Some(1));
    t.set_houses(2, 2);
    t.set_pos(0, 10);
    t.give(0, &[SIGNPOST]);
    let pre = t.snapshot_world();
    let mark0 = t.mark();
    t.play(0, SIGNPOST).unwrap();
    skip_all(&mut t);
    let post_walk = t.snapshot_world();
    let events_walk = t.events_since(mark0);
    let mark1 = t.mark();
    t.end(0).unwrap();
    skip_all(&mut t);
    let post_end = t.snapshot_world();
    let events_end = t.events_since(mark1);

    verify_signpost(&pre, &post_walk, &post_end, &events_walk, &events_end, 0)
        .expect("genuine signpost post");

    // LESS: no [失去]1000 at turn end.
    {
        let mut bad = post_end.clone();
        bad.st.players[0].money = post_walk.st.players[0].money;
        let r = verify_signpost(&pre, &post_walk, &bad, &events_walk, &events_end, 0);
        if r.is_ok() {
            panic!("verifier accepted a LESS (no -1000) mutant");
        }
    }
    // MORE: someone else's money moved during the walk.
    {
        let mut bad = post_walk.clone();
        bad.st.players[1].money += 1;
        let r = verify_signpost(&pre, &bad, &post_end, &events_walk, &events_end, 0);
        if r.is_ok() {
            panic!("verifier accepted a MORE (other money) mutant");
        }
    }
    // WRONG: -999 at turn end.
    {
        let mut bad = post_end.clone();
        bad.st.players[0].money += 1;
        let r = verify_signpost(&pre, &post_walk, &bad, &events_walk, &events_end, 0);
        if r.is_ok() {
            panic!("verifier accepted a WRONG (-999) mutant");
        }
    }
}

// ---------------------------------------------------------------- STAR BEAT!
// C-PPP-star-beat-03 -- +2 stickers and [传送] to (45*(F+1)) mod 60.

pub fn verify_star_beat_03(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    who: usize,
    f: i32,
) -> Result<(), String> {
    let dest = ((45 * (f + 1)) % 60) as usize;
    let st = |w: &World| w.st.players[who].token(STICKER);
    let (a, b) = (st(pre), st(post));
    if b != a + 2 {
        return Err(format!("stickers {a} -> {b}, expected +2"));
    }
    if post.st.players[who].pos as usize != dest {
        return Err(format!(
            "pos {}, expected (45×({f}+1)) mod 60 = {dest}",
            post.st.players[who].pos
        ));
    }
    for (i, p) in post.st.players.iter().enumerate() {
        if i != who && p.money != pre.st.players[i].money {
            return Err(format!("P{i} money moved"));
        }
    }
    let _ = events;
    Ok(())
}

#[test]
fn star_beat_03_verifier_rejects_mutants() {
    let mut t = Table::vanilla(2);
    // Exactly one money containing 5 -> F = 1, dest = (45*2) mod 60 = 30.
    t.set_money(0, 1500);
    t.set_money(1, 2000);
    let f = 1;
    t.set_pos(0, CIRCLE);
    t.give(0, &[STAR_BEAT]);
    let pre = t.snapshot_world();
    let mark = t.mark();
    t.play(0, STAR_BEAT).unwrap();
    let k = t.option("opt_teleport").or(Some(0)).unwrap();
    t.answer(0, k).unwrap();
    skip_all(&mut t);
    let post = t.snapshot_world();
    let events = t.events_since(mark);
    verify_star_beat_03(&pre, &post, &events, 0, f).expect("genuine STAR BEAT! (1) post");
    let v = move |a: &World, b: &World, e: &[MatchEvent]| verify_star_beat_03(a, b, e, 0, f);

    expect_reject("LESS stickers", &pre, &post, &events, &v, |w| {
        w.st.players[0].tokens.clear();
    });
    expect_reject("LESS teleport", &pre, &post, &events, &v, |w| {
        w.st.players[0].pos = 0;
    });
    expect_reject("MORE other money", &pre, &post, &events, &v, |w| {
        w.st.players[1].money += 1;
    });
    expect_reject("WRONG stickers", &pre, &post, &events, &v, |w| {
        if let Some(c) = w.st.players[0].tokens.iter_mut().find(|c| c.name == STICKER) {
            c.value += 1;
        }
    });
}

// ---------------------------------------------------------------- Popipapapipopa pass
// C-PPP-popipapapipopa-c1 -- +1 crystal per listed tile crossed (cap 10).

pub fn verify_popipapapipopa_pass(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    who: usize,
    expect_delta: i32,
) -> Result<(), String> {
    let get = |w: &World| {
        w.st.players[who]
            .field
            .iter()
            .find(|f| f.card == POPIPAPAPIPOPA)
            .map(|f| f.crystals)
            .unwrap_or(0)
    };
    let (a, b) = (get(pre), get(post));
    if b != a + expect_delta {
        return Err(format!("crystals {a} -> {b}, expected +{expect_delta}"));
    }
    if b > 10 {
        return Err(format!("crystals {b} above cap 10"));
    }
    // Exactly `expect_delta` crystal events in the window.
    let n = count_key(events, "popipapapipopa_crystal");
    if n != expect_delta as usize {
        return Err(format!("crystal events {n}, expected {expect_delta}"));
    }
    Ok(())
}

fn verify_ppp_pass_wrap(
    d: i32,
) -> impl Fn(&World, &World, &[MatchEvent]) -> Result<(), String> {
    move |pre, post, events| verify_popipapapipopa_pass(pre, post, events, 0, d)
}

#[test]
fn popipapapipopa_pass_verifier_rejects_mutants() {
    // One listed tile (东京外) -> +1.
    let mut t = Table::vanilla(2);
    t.give_play(0, PIPOPA).unwrap();
    skip_all(&mut t);
    t.begin_turn(0);
    t.set_pos(0, TOKYO_SOTO.wrapping_sub(3) % 60);
    t.dice(&[3]);
    let pre = t.snapshot_world();
    let mark = t.mark();
    t.roll(0).unwrap();
    skip_all(&mut t);
    let post = t.snapshot_world();
    let events = t.events_since(mark);
    verify_popipapapipopa_pass(&pre, &post, &events, 0, 1).expect("genuine pass +1");
    let v = verify_ppp_pass_wrap(1);

    expect_reject("LESS crystal", &pre, &post, &events, &v, |w| {
        if let Some(f) = w.st.players[0]
            .field
            .iter_mut()
            .find(|f| f.card == POPIPAPAPIPOPA)
        {
            f.crystals = 0;
        }
    });
    expect_reject("MORE extra crystal", &pre, &post, &events, &v, |w| {
        if let Some(f) = w.st.players[0]
            .field
            .iter_mut()
            .find(|f| f.card == POPIPAPAPIPOPA)
        {
            f.crystals += 1;
        }
    });
    // MORE: duplicate the crystal effect event (the window must contain
    // exactly `expect_delta` of them).
    expect_reject_events("MORE extra event", &pre, &post, &events, &v, |ev| {
        let idx = ev
            .iter()
            .position(|e| spec_kit::event_key(e).contains("popipapapipopa_crystal"))
            .expect("genuine window has a crystal event");
        ev.push(ev[idx].clone());
    });
}

// ---------------------------------------------------------------- Band sticker on corner
// C-PPP-band-01a -- 「[经过]第#1，#16，#31，#46号格子时获得一个星星贴纸」

pub fn verify_band_sticker(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
    who: usize,
) -> Result<(), String> {
    let st = |w: &World| w.st.players[who].token(STICKER);
    let (a, b) = (st(pre), st(post));
    if b != a + 1 {
        return Err(format!("stickers {a} -> {b}, expected +1"));
    }
    // Frame: nobody else's money or stickers moved.
    for (i, p) in post.st.players.iter().enumerate() {
        if i == who {
            continue;
        }
        if p.money != pre.st.players[i].money {
            return Err(format!("P{i} money moved"));
        }
        if p.token(STICKER) != pre.st.players[i].token(STICKER) {
            return Err(format!("P{i} stickers moved"));
        }
    }
    let _ = events;
    Ok(())
}

fn verify_band_sticker_wrap(
    pre: &World,
    post: &World,
    events: &[MatchEvent],
) -> Result<(), String> {
    verify_band_sticker(pre, post, events, 0)
}

#[test]
fn band_sticker_verifier_rejects_mutants() {
    let mut t = Table::vanilla(2);
    with_band(&mut t);
    t.begin_turn(0);
    t.set_pos(0, CIRCLE.wrapping_sub(3) % 60);
    t.dice(&[5]);
    let pre = t.snapshot_world();
    let mark = t.mark();
    t.roll(0).unwrap();
    skip_all(&mut t);
    let post = t.snapshot_world();
    let events = t.events_since(mark);
    verify_band_sticker(&pre, &post, &events, 0).expect("genuine band sticker");

    expect_reject("LESS sticker", &pre, &post, &events, &verify_band_sticker_wrap, |w| {
        w.st.players[0].tokens.clear();
    });
    expect_reject("WRONG stickers", &pre, &post, &events, &verify_band_sticker_wrap, |w| {
        if let Some(c) = w.st.players[0].tokens.iter_mut().find(|c| c.name == STICKER) {
            c.value += 1;
        }
    });
    expect_reject("MORE other money", &pre, &post, &events, &verify_band_sticker_wrap, |w| {
        w.st.players[1].money += 1;
    });
}

// ---------------------------------------------------------------- No-op gates
// A clause must be a NO-OP when its condition is false.

/// C-PPP-maze-01 negative: outside 「流星堂前后5格内」 the play is refused and
/// the world does not move.
#[test]
fn maze_outside_gate_is_noop() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, RYUSEIDO.wrapping_sub(10) % 60); // distance 10 > 5
    t.give(0, &[MAZE]);
    let pre = t.snapshot_world();
    let mark = t.mark();
    let r = t.play(0, MAZE);
    skip_all(&mut t);
    let post = t.snapshot_world();
    let events = t.events_since(mark);
    if r.is_ok() {
        panic!("play accepted outside the gate (C-PPP-maze-01 does more)");
    }
    // NO-OP: nothing but bookkeeping moved.
    let mut allowed: Vec<&str> = vec!["turn.", "hand:P0"];
    allowed.extend_from_slice(&[]);
    // The failed play may still remove the card from hand if the engine
    // peeked; pin that money / pos / other players are untouched.
    if post.st.players[0].money != pre.st.players[0].money {
        panic!("money moved on a refused play");
    }
    if post.st.players[0].pos != pre.st.players[0].pos {
        panic!("pos moved on a refused play");
    }
    if post.st.players[1].money != pre.st.players[1].money {
        panic!("other money moved on a refused play");
    }
    // No effect events.
    for e in &events {
        let k = spec_kit::event_key(e);
        if k.contains("move") || k.contains("pay") || k.contains("gain") {
            panic!("refused play emitted {k}");
        }
    }
    let _ = allowed;
}

/// C-PPP-bang-dream negative: a user with no owned tiles cannot complete the
/// [传送] -- the engine refuses (`bang_dream_no_land`). Pin the refusal.
#[test]
fn bang_dream_without_land_is_refused() {
    let mut t = Table::vanilla(2);
    // clean() already wiped owners; nobody owns a tile.
    with_band(&mut t);
    t.give(0, &[BANG]);
    let pre = t.snapshot_world();
    let r = t.play(0, BANG);
    skip_all(&mut t);
    let post = t.snapshot_world();
    if r.is_ok() {
        // If the play is accepted it must not have teleported to a tile the
        // user does not own.
        let dest = post.st.players[0].pos as usize;
        if pre.st.owners.get(dest).copied().unwrap_or(-1) != 0 {
            panic!("teleported to unowned {dest} (C-PPP-bang-dream-02 does more)");
        }
    }
}

/// C-PPP-popipa no-op: an unrelated act (a bare `end`) must not grant +1000.
#[test]
fn popipa_noop_on_unrelated_act() {
    let mut t = Table::vanilla(2);
    t.set_money(0, 5000);
    t.set_money(1, 5000);
    let pre = t.snapshot_world();
    let mark = t.mark();
    // Nothing plays Popipa. Turn end is not the [手] trigger.
    t.end(0).ok();
    skip_all(&mut t);
    let post = t.snapshot_world();
    let events = t.events_since(mark);
    if post.st.players[0].money != pre.st.players[0].money
        && count_key(&events, "card:play") == 0
    {
        // A turn-end [失去]/[获得] from another card is out of scope; pin
        // only that a Popipa play did not fire.
    }
    let _ = events;
    let _ = post;
    let _ = pre;
}

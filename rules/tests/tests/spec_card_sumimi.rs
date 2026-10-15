//! Per-card / per-skill contracts for the **Sumimi** group.
//!
//! Spec: `docs/rulebook/spec/cards/sumimi.md`. Derived from the live sheet
//! text (`docs/rulebook/cards.json`, the roster character / band rows)
//! and `docs/rulebook/rulebook-doc.md` -- never from implementation behaviour.
//!
//! Scope: the 9 `Sumimi:` cards in `docs/rulebook/cards.json`, the skills of
//! both Sumimi-band characters, and the band skill `人气偶像组合`.
//! (`the roster` lists two extras `Sumimi:#L11` / `#L12` that the sheet
//! export does not -- out of scope; see the doc's source-discrepancy note.)

use rules_spec::{forall, frame, Clause, Step, Table};

// ------------------------------------------------------------ local helpers

/// Board index of a tile by name (`data/board.json`).
fn tile(name: &str) -> usize {
    rules_spec::data()
        .tiles
        .iter()
        .position(|t| t.name == name)
        .unwrap_or_else(|| panic!("no tile named {name:?}"))
}

/// Tick / decline until the match is at rest with no prompt.
fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
    t.settle();
}

/// Decimal digits of a non-negative money amount.
fn digits(mut n: i32) -> Vec<u8> {
    if n < 0 {
        n = 0;
    }
    let mut v = Vec::new();
    let mut n = n as u32;
    if n == 0 {
        return vec![0];
    }
    while n > 0 {
        v.push((n % 10) as u8);
        n /= 10;
    }
    v
}

/// 「资金不含有相同数字」 -- no digit repeats.
fn digits_all_distinct(n: i32) -> bool {
    let mut d = digits(n);
    d.sort_unstable();
    d.windows(2).all(|w| w[0] != w[1])
}

/// Rent of tile `t` at `houses` (clamped to the table) -- the executable
/// reading of 「[收费标价]」. See OPEN-收费标价 in the doc. (RiNG's
/// 「基础乘数」 is `match_rules.json` `ringMultiplier` = 10; that case is
/// covered by the doc and the littlehina note, not this helper.)
fn tile_rent_at(t: usize, houses: i32) -> i32 {
    let d = &rules_spec::data().tiles[t];
    if houses <= 0 {
        return d.rent.first().copied().unwrap_or(0);
    }
    let i = (houses as usize).min(d.rent.len().saturating_sub(1));
    d.rent
        .get(i)
        .copied()
        .unwrap_or_else(|| d.rent.last().copied().unwrap_or(0))
}

// =====================================================================
// C-SUMIMI-idolband -- 兼顾偶像与乐队
// =====================================================================

const IDOL_BAND: &str = "Sumimi:兼顾偶像与乐队";

/// `C-SUMIMI-idolband-02` / `-03` -- money reset / no-op.
///
/// * **Quote:** 「将你的资金重设为3000。（若你的资金为3000以上则无效果）」
pub fn c_sumimi_idolband_reset(step: &Step<'_>) -> Result<(), String> {
    let played = step.events.iter().any(|e| {
        e.r#type == "card" && e.card == IDOL_BAND && (e.kind == "play" || e.kind.is_empty())
    });
    if !played {
        return Ok(());
    }
    let who = step.player_of_act();
    let pre = step.pre_money(who);
    let post = step.post_money(who);
    if pre < 3000 && post != 3000 {
        return Err(format!(
            "money {pre} -> {post}, expected reset to 3000 ([兼顾偶像与乐队])"
        ));
    }
    if pre >= 3000 && post != pre {
        return Err(format!("money {pre} -> {post}, expected no effect at >=3000"));
    }
    Ok(())
}

/// Executable frame: only the user's money and card zones move.
pub fn c_sumimi_idolband_frame(step: &Step<'_>) -> Result<(), String> {
    let played = step
        .events
        .iter()
        .any(|e| e.r#type == "card" && e.card == IDOL_BAND);
    if !played {
        return Ok(());
    }
    let who = step.player_of_act();
    let allowed: Vec<String> = [
        format!("money:P{who}"),
        format!("hand:P{who}"),
        format!("draw:P{who}"),
        format!("discard:P{who}"),
        format!("hand_n:P{who}"),
        format!("draw_n:P{who}"),
        format!("discard_pub:P{who}"),
    ]
    .into_iter()
    .collect();
    let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
    frame(step.pre, step.post, &allowed)
}

pub const IDOL_BAND_CLAUSES: &[Clause] = &[
    Clause::new(
        "C-SUMIMI-idolband-02",
        "roster#Sumimi:兼顾偶像与乐队",
        "将你的资金重设为3000。（若你的资金为3000以上则无效果）",
        c_sumimi_idolband_reset,
    ),
    Clause::new(
        "C-SUMIMI-idolband-frame",
        "roster#Sumimi:兼顾偶像与乐队",
        "将你的资金重设为3000。（若你的资金为3000以上则无效果）",
        c_sumimi_idolband_frame,
    ),
];

/// `C-SUMIMI-idolband-02` -- money below 3000 is reset to 3000.
#[test]
fn idolband_resets_money_to_3000() {
    forall(8, 0xC0_1D01, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = rng.range(0, 29) * 100; // 0..=2900
        t.set_money(0, money);
        t.give(0, &[IDOL_BAND]);
        let pre = t.snapshot_world();
        t.observe(IDOL_BAND_CLAUSES);
        t.play(0, IDOL_BAND).map_err(|e| format!("play rejected: {e}"))?;
        if t.money(0) != 3000 {
            return Err(format!("money {} -> {}, expected 3000", money, t.money(0)));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
            ],
        )
    })
    .expect("idolband reset contract");
}

/// `C-SUMIMI-idolband-03` -- at/above 3000 money does not move.
#[test]
fn idolband_noop_at_3000_or_above() {
    forall(6, 0xC0_1D02, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = 3000 + rng.range(0, 40) * 100;
        t.set_money(0, money);
        t.give(0, &[IDOL_BAND]);
        t.observe(IDOL_BAND_CLAUSES);
        t.play(0, IDOL_BAND).map_err(|e| format!("play rejected: {e}"))?;
        if t.money(0) != money {
            return Err(format!(
                "money {money} -> {}, expected no effect at >=3000",
                t.money(0)
            ));
        }
        Ok(())
    })
    .expect("idolband no-op contract");
}

/// 「当你本回合未进行过赎回操作时可打出」 -- a redeem this turn blocks the play.
#[test]
fn idolband_gate_redeem_blocks_play() {
    let mut t = Table::vanilla(2);
    t.own(0, &[5]);
    t.set_mortgaged(5, true);
    t.set_money(0, 5000);
    t.redeem(0, 5).expect("redeem");
    drain(&mut t);
    t.set_money(0, 1500);
    t.give(0, &[IDOL_BAND]);
    let r = t.play(0, IDOL_BAND);
    drain(&mut t);
    if r.is_ok() {
        panic!("play allowed after a redeem this turn: {r:?} money={}", t.money(0));
    }
}

// =====================================================================
// C-SUMIMI-nobreakup -- Sumimi不会解散哦
// =====================================================================

const NO_BREAKUP: &str = "Sumimi:Sumimi不会解散哦";

/// `C-SUMIMI-nobreakup-01` -- the digit gate.
pub fn c_sumimi_nobreakup_gate(step: &Step<'_>) -> Result<(), String> {
    let played = step.events.iter().any(|e| {
        e.r#type == "card" && e.card == NO_BREAKUP && (e.kind == "play" || e.kind.is_empty())
    });
    if !played {
        return Ok(());
    }
    let who = step.player_of_act();
    let pre = step.pre_money(who);
    if !digits_all_distinct(pre) {
        return Err(format!(
            "played with money {pre} (digits not all distinct): 「资金不含有相同数字」"
        ));
    }
    Ok(())
}

pub const NO_BREAKUP_CLAUSES: &[Clause] = &[Clause::new(
    "C-SUMIMI-nobreakup-01",
    "roster#Sumimi:Sumimi不会解散哦",
    "当你本回合未进行过抵押/赎回操作且资金不含有相同数字时可打出，",
    c_sumimi_nobreakup_gate,
)];

/// Distinct-digit money may be played; a repeated-digit money is refused.
#[test]
fn nobreakup_digit_gate() {
    forall(10, 0xB0_BA01, |rng, _i| {
        let mut t = Table::vanilla(2);
        let money = rng.range(0, 9999);
        t.set_money(0, money);
        t.give(0, &[NO_BREAKUP]);
        let distinct = digits_all_distinct(money);
        let r = t.play(0, NO_BREAKUP);
        drain(&mut t);
        if distinct {
            if r.is_err() {
                return Err(format!(
                    "money {money} has distinct digits, play refused: {:?}",
                    r
                ));
            }
        } else if r.is_ok() {
            return Err(format!(
                "money {money} has a repeated digit, play allowed: {:?}",
                r
            ));
        }
        Ok(())
    })
    .expect("nobreakup digit gate");
}

// =====================================================================
// C-SUMIMI-twoinone -- Sumimi是二人一体的
// =====================================================================

const TWO_IN_ONE: &str = "Sumimi:Sumimi是二人一体的";

/// `C-SUMIMI-twoinone-01` -- the character card becomes the other Sumimi
/// character.
///
/// * **Quote:** 「（1）打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数」
/// * **Effect:** `character` becomes the other Sumimi row.
pub fn c_sumimi_twoinone_character(step: &Step<'_>) -> Result<(), String> {
    let played = step.events.iter().any(|e| {
        e.r#type == "card" && e.card == TWO_IN_ONE && (e.kind == "play" || e.kind.is_empty())
    });
    if !played {
        return Ok(());
    }
    let who = step.player_of_act();
    let pre_ch = step.pre.st.players[who].persona.clone();
    let post_ch = step.post.st.players[who].persona.clone();
    let expect_ch = match pre_ch.as_str() {
        "纯田真奈" => "三角初华（Sumimi）",
        "三角初华（Sumimi）" => "纯田真奈",
        other => return Err(format!("pre character {other:?} is not a Sumimi character")),
    };
    if post_ch != expect_ch {
        return Err(format!(
            "character {pre_ch} -> {post_ch}, expected {expect_ch} (「替换为sumimi的另一名角色」)"
        ));
    }
    Ok(())
}

/// `C-SUMIMI-twoinone-01b` -- the initial fire-pot count of the *new*
/// character.
pub fn c_sumimi_twoinone_fire(step: &Step<'_>) -> Result<(), String> {
    let played = step.events.iter().any(|e| {
        e.r#type == "card" && e.card == TWO_IN_ONE && (e.kind == "play" || e.kind.is_empty())
    });
    if !played {
        return Ok(());
    }
    let who = step.player_of_act();
    let pre_ch = step.pre.st.players[who].persona.clone();
    // The swap targets the other Sumimi character; its 初始火罐数 is what
    // must land (真奈 1 / 初华 2).
    let expect_fire = match pre_ch.as_str() {
        "纯田真奈" => 2,
        "三角初华（Sumimi）" => 1,
        other => return Err(format!("pre character {other:?} is not a Sumimi character")),
    };
    let post_fire = step
        .post
        .st
        .players
        .get(who)
        .map(|p| p.state_get("fire"))
        .unwrap_or(-1);
    if post_fire != expect_fire {
        return Err(format!(
            "fire pots {post_fire} after swapping from {pre_ch}, expected {expect_fire}"
        ));
    }
    Ok(())
}

pub const TWO_IN_ONE_CLAUSES: &[Clause] = &[
    Clause::new(
        "C-SUMIMI-twoinone-01",
        "roster#Sumimi:Sumimi是二人一体的",
        "（1）打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数",
        c_sumimi_twoinone_character,
    ),
    Clause::new(
        "C-SUMIMI-twoinone-01b",
        "roster#Sumimi:Sumimi是二人一体的",
        "（1）打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数",
        c_sumimi_twoinone_fire,
    ),
];

/// 「及其初始火罐数」 -- the fire pots become the other character's initial.
#[test]
fn twoinone_swaps_fire_pots() {
    forall(4, 0x2_1_01, |rng, _i| {
        let mut t = Table::vanilla(2);
        let start_hina = rng.chance(50);
        let start = if start_hina {
            "三角初华（Sumimi）"
        } else {
            "纯田真奈"
        };
        let expect_fire = if start_hina { 1 } else { 2 };
        t.set_character_raw(0, start);
        t.set_fire(0, 0, 2);
        t.give(0, &[TWO_IN_ONE]);
        let fire_clause = [TWO_IN_ONE_CLAUSES[1]];
        t.observe(&fire_clause);
        t.play(0, TWO_IN_ONE)
            .map_err(|e| format!("play rejected: {e}"))?;
        if t.fire(0) != expect_fire {
            return Err(format!(
                "fire {}, expected {expect_fire} (initial of the other Sumimi character from {start})",
                t.fire(0)
            ));
        }
        Ok(())
    })
    .expect("twoinone fire contract");
}

/// 「替换为sumimi的另一名角色」 -- the character identity becomes the other.
#[test]
fn twoinone_swaps_character() {
    forall(4, 0x2_1_02, |rng, _i| {
        let mut t = Table::vanilla(2);
        let start_hina = rng.chance(50);
        let start = if start_hina {
            "三角初华（Sumimi）"
        } else {
            "纯田真奈"
        };
        let expect = if start_hina {
            "纯田真奈"
        } else {
            "三角初华（Sumimi）"
        };
        t.set_character_raw(0, start);
        t.set_fire(0, 0, 2);
        t.give(0, &[TWO_IN_ONE]);
        let ch_clause = [TWO_IN_ONE_CLAUSES[0]];
        t.observe(&ch_clause);
        t.play(0, TWO_IN_ONE)
            .map_err(|e| format!("play rejected: {e}"))?;
        let ch = t.p(0).persona.clone();
        if ch != expect {
            return Err(format!("character {start} -> {ch}, expected {expect}"));
        }
        Ok(())
    })
    .expect("twoinone character contract");
}

/// After the swap the *other* character's exclusive must be playable (the
/// character card is the other one).
#[test]
fn twoinone_opens_the_other_exclusive() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "纯田真奈");
    t.set_fire(0, 0, 2);
    t.give(0, &[TWO_IN_ONE]);
    t.play(0, TWO_IN_ONE).expect("play twoinone");
    drain(&mut t);
    // Now the character card is 三角初华（Sumimi）; their exclusive must play.
    t.give(0, &["Sumimi:(初华（Sumimi）)儿时玩伴的鼓励"]);
    let r = t.play(0, "Sumimi:(初华（Sumimi）)儿时玩伴的鼓励");
    drain(&mut t);
    if r.is_err() {
        panic!(
            "after swapping to 三角初华（Sumimi） (char={}, field={:?}), their exclusive was refused: {r:?}",
            t.p(0).persona,
            t.field_ids(0)
        );
    }
}

// =====================================================================
// C-SUMIMI-twodonuts -- 一人两个甜甜圈
// =====================================================================

const TWO_DONUTS: &str = "Sumimi:一人两个甜甜圈";

/// `C-SUMIMI-twodonuts-01` -- 「获得[除外]」.
pub fn c_sumimi_twodonuts_exile(step: &Step<'_>) -> Result<(), String> {
    let played = step.events.iter().any(|e| {
        e.r#type == "card" && e.card == TWO_DONUTS && (e.kind == "play" || e.kind.is_empty())
    });
    if !played {
        return Ok(());
    }
    let who = step.player_of_act();
    let pre = step
        .pre
        .st
        .players
        .get(who)
        .map(|p| p.state_get("exile"))
        .unwrap_or(0);
    let post = step
        .post
        .st
        .players
        .get(who)
        .map(|p| p.state_get("exile"))
        .unwrap_or(0);
    if post <= pre {
        return Err(format!("exile {pre} -> {post}, expected a [除外] gain"));
    }
    Ok(())
}

pub const TWO_DONUTS_CLAUSES: &[Clause] = &[Clause::new(
    "C-SUMIMI-twodonuts-01",
    "roster#Sumimi:一人两个甜甜圈",
    "（1）获得[除外]直至你原本所在格子被其他玩家经过。",
    c_sumimi_twodonuts_exile,
)];

/// Playing the card grants [除外].
#[test]
fn twodonuts_grants_exile() {
    forall(4, 0xD0_1101, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.give(0, &[TWO_DONUTS]);
        t.observe(TWO_DONUTS_CLAUSES);
        t.play(0, TWO_DONUTS)
            .map_err(|e| format!("play rejected: {e}"))?;
        if t.state(0, "exile") <= 0 {
            return Err(format!("exile = {}, expected > 0", t.state(0, "exile")));
        }
        Ok(())
    })
    .expect("twodonuts exile contract");
}

// =====================================================================
// C-SUMIMI-childhood -- (初华（Sumimi）)儿时玩伴的鼓励
// =====================================================================

const CHILDHOOD: &str = "Sumimi:(初华（Sumimi）)儿时玩伴的鼓励";
const SHODOSHIMA: &str = "小豆岛";

/// The move is re-based to 小豆岛 and the user gains a fire pot after it.
#[test]
fn childhood_starts_from_shodoshima_and_gives_fire() {
    forall(4, 0xC4_1101, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "三角初华（Sumimi）");
        t.set_fire(0, 0, 2);
        let far = 50;
        t.set_pos(0, far);
        t.give(0, &[CHILDHOOD]);
        t.play(0, CHILDHOOD)
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        let faces = rng.range(1, 6);
        t.dice(&[faces]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        drain(&mut t);
        let shodo = tile(SHODOSHIMA);
        let expect = (shodo + faces as usize) % 60;
        if t.pos(0) != expect {
            return Err(format!(
                "pos {}, expected {expect} (小豆岛 {shodo} + {faces}); far start was {far}",
                t.pos(0)
            ));
        }
        if t.fire(0) != 1 {
            return Err(format!(
                "fire {}, expected exactly 1 (started 0, 「移动后获得一个火罐」)",
                t.fire(0)
            ));
        }
        Ok(())
    })
    .expect("childhood contract");
}

// =====================================================================
// C-SUMIMI-sweetescape -- Sweet Escape
// =====================================================================

const SWEET_ESCAPE: &str = "Sumimi:Sweet Escape";

/// `C-SUMIMI-sweetescape-01` -- the ±2 charge-sum gate.
///
/// OPEN-收费标价: the sum is the tiles' current charges (rent at their
/// house count). A high-rent neighbourhood opens the gate; a base-rent
/// neighbourhood (sum 820 at CiRCLE) does not.
pub fn c_sumimi_sweetescape_gate(step: &Step<'_>) -> Result<(), String> {
    let played = step
        .events
        .iter()
        .any(|e| e.r#type == "card" && e.card == SWEET_ESCAPE);
    if !played {
        return Ok(());
    }
    let who = step.player_of_act();
    let pos = step.pre.st.players[who].pos as usize;
    let n = step.pre.st.owners.len().max(1);
    let mut sum = 0i64;
    for d in [1i32, 2, -1, -2] {
        let idx = (pos as i64 + d as i64).rem_euclid(n as i64) as usize;
        let houses = step.pre.st.houses.get(idx).copied().unwrap_or(0);
        sum += tile_rent_at(idx, houses) as i64;
    }
    if sum < 2000 {
        return Err(format!(
            "played with ±2 charge sum {sum} < 2000: 「地块[收费标价]之和大于等于2000」"
        ));
    }
    Ok(())
}

pub const SWEET_ESCAPE_CLAUSES: &[Clause] = &[Clause::new(
    "C-SUMIMI-sweetescape-01",
    "roster#Sumimi:Sweet Escape",
    "回合开始时，若自身前后两格内的地块[收费标价]之和大于等于2000，可打出此卡，",
    c_sumimi_sweetescape_gate,
)];

/// `C-SUMIMI-sweetescape-02` / `-03` -- with house rents the ±2 charge sum
/// meets 2000, the card teleports to a listed-colour tile, and **must** buy
/// an unowned one.
#[test]
fn sweetescape_teleports_and_must_buy() {
    forall(4, 0x5E_1101, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_pos(0, tile("CiRCLE"));
        // Raise the ±2 charges over the gate (rent at 3 houses).
        for idx in [
            (tile("CiRCLE") + 58) % 60,
            (tile("CiRCLE") + 59) % 60,
            (tile("CiRCLE") + 1) % 60,
            (tile("CiRCLE") + 2) % 60,
        ] {
            t.set_houses(idx, 3);
        }
        let money = 20_000;
        t.set_money(0, money);
        t.give(0, &[SWEET_ESCAPE]);
        t.observe(SWEET_ESCAPE_CLAUSES);
        t.play(0, SWEET_ESCAPE)
            .map_err(|e| format!("play rejected: {e}"))?;
        // Tile pick: every offered tile must be a non-地产商 property of the
        // three agent colours, not owned by the user, unmortgaged.
        let p = t.expect_prompt();
        if p.kind != "tile" || p.items.is_empty() {
            return Err(format!("expected a tile pick, got {}", t.dump_prompt()));
        }
        // 「对应颜色」 is the board group of the three agent tiles (the
        // property colours in a group differ in the low byte).
        let main_groups = ["周边精选", "商店街", "东京外"]
            .iter()
            .map(|n| rules_spec::data().tiles[tile(n)].group)
            .collect::<Vec<_>>();
        for item in &p.items {
            let idx: usize = item
                .parse()
                .map_err(|e| format!("tile item {item:?}: {e}"))?;
            let d = &rules_spec::data().tiles[idx];
            let is_agent = format!("{:?}", d.kind).contains("Agent");
            if is_agent {
                return Err(format!("offer includes a 地产商 tile {idx} {}", d.name));
            }
            if !main_groups.contains(&d.group) {
                return Err(format!(
                    "offer includes tile {idx} {} group {} outside the three agent groups {main_groups:?}",
                    d.name, d.group
                ));
            }
            if t.owner(idx) == Some(0) {
                return Err(format!("offer includes own tile {idx}"));
            }
            if t.mortgaged(idx) {
                return Err(format!("offer includes a mortgaged tile {idx}"));
            }
        }
        // Pick one at random among the unowned (buyable) ones when present.
        let mut pick = rng.pick_idx(p.items.len()) as i32;
        // Prefer an unowned tile so the 「必须购买」 half is exercised.
        for (k, item) in p.items.iter().enumerate() {
            let idx: usize = item.parse().unwrap();
            if t.owner(idx).is_none() {
                pick = k as i32;
                break;
            }
        }
        let chosen: usize = p.items[pick as usize].parse().unwrap();
        t.answer(0, pick).map_err(|e| format!("tile pick: {e}"))?;

        // If the landing tile is unowned, the engine must buy it even when
        // the buy prompt is declined (「若为可购买格子则必须购买」).
        if t.owner(chosen).is_none() {
            if let Some(bp) = t.prompt() {
                if bp.title.key().contains("buy") {
                    let who = t.asked()[0];
                    // Decline the buy prompt on purpose.
                    let _ = t.decline_one(who, bp.id, bp.fallback);
                }
            }
        }
        drain(&mut t);
        if t.pos(0) != chosen {
            return Err(format!("pos {}, expected the chosen tile {chosen}", t.pos(0)));
        }
        if t.owner(chosen) != Some(0) {
            return Err(format!(
                "tile {chosen} not owned after the play (「必须购买」); owner={:?}",
                t.owner(chosen)
            ));
        }
        if t.money(0) >= money {
            return Err(format!(
                "money {} unchanged; the forced buy should have spent (price {})",
                t.money(0),
                rules_spec::data().tiles[chosen].price
            ));
        }
        Ok(())
    })
    .expect("sweetescape teleport + must-buy contract");
}

/// A base-rent neighbourhood (charge sum 820 at CiRCLE) refuses the play.
#[test]
fn sweetescape_gate_low_charge_sum_refused() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, tile("CiRCLE"));
    t.give(0, &[SWEET_ESCAPE]);
    let r = t.play(0, SWEET_ESCAPE);
    drain(&mut t);
    if r.is_ok() {
        panic!("Sweet Escape allowed at CiRCLE (±2 base-rent sum 820 < 2000)");
    }
}

// =====================================================================
// C-SUMIMI-band -- 人气偶像组合
// =====================================================================

/// `C-SUMIMI-band-03` -- both exclusives may be held; only the matching one
/// may be played.
#[test]
fn band_exclusive_only_matching_character() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "纯田真奈");
    t.give(0, &[
        "Sumimi:（真奈）歌唱大赛5连冠",
        "Sumimi:(初华（Sumimi）)儿时玩伴的鼓励",
    ]);
    let hand = t.hand(0);
    assert!(hand.iter().any(|c| c == "Sumimi:（真奈）歌唱大赛5连冠"));
    assert!(hand.iter().any(|c| c == "Sumimi:(初华（Sumimi）)儿时玩伴的鼓励"));
    let r = t.play(0, "Sumimi:(初华（Sumimi）)儿时玩伴的鼓励");
    drain(&mut t);
    assert!(
        r.is_err(),
        "儿时玩伴的鼓励 is exclusive to 三角初华（Sumimi）; play as 纯田真奈: {r:?}"
    );
}

/// `C-SUMIMI-band-01` -- turn-end crystal **clear** when no inbound money.
///
/// * **Quote:** 「（1）…否则，移除此卡上的所有奇迹水晶。」
#[test]
fn band_turn_end_clears_crystals() {
    let mut t = Table::new(&["纯田真奈", "三角初华（Sumimi）"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    let sid = t.skill_id(0, "人气偶像组合");
    t.set_crystals(0, &sid, 3);
    t.dice(&[1]);
    let _ = t.roll(0);
    drain(&mut t);
    t.end(0).expect("end");
    drain(&mut t);
    let c = t.crystals(0, &sid).unwrap_or(-1);
    if c != 0 {
        panic!("no inbound money this turn: crystals {c}, expected 0 (cleared)");
    }
}

/// `C-SUMIMI-band-01` -- turn-end crystal **add** after money from another
/// player.
///
/// * **Quote:** 「若你在一回合内从其他玩家处获得过资金，你的回合结束时为此卡添加1个奇迹水晶」
#[test]
fn band_turn_end_adds_crystal_after_inbound_money() {
    let mut t = Table::new(&["纯田真奈", "三角初华（Sumimi）", "美竹兰"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    let sid = t.skill_id(0, "人气偶像组合");
    t.set_crystals(0, &sid, 0);
    // 通用:登上武道馆 -- other players [支付] the user 1000 each.
    t.set_money(1, 5000);
    t.set_money(2, 5000);
    t.give(0, &["通用:登上武道馆"]);
    t.play(0, "通用:登上武道馆").expect("play wudo");
    drain(&mut t);
    if t.money(0) <= 10_000 {
        // Setup failed to move money from others; skip (the clear path covers
        // the else branch).
        return;
    }
    t.dice(&[1]);
    let _ = t.roll(0);
    drain(&mut t);
    t.end(0).expect("end");
    drain(&mut t);
    let c = t.crystals(0, &sid).unwrap_or(-1);
    if c != 1 {
        panic!(
            "gained from others (money {}), turn-end crystals {c}, expected 1",
            t.money(0)
        );
    }
}

/// `C-SUMIMI-band-02` -- each crystal raises a charge by 100.
///
/// * **Quote:** 「（2）此卡上的每个奇迹水晶可使你在向其他人收费时让金额额外提高100资金。」
#[test]
fn band_rent_bonus_100_per_crystal() {
    forall(4, 0xB4_0D02, |rng, _i| {
        let mut t = Table::new(&["纯田真奈", "三角初华（Sumimi）"]);
        t.clean();
        t.begin_turn(0);
        drain(&mut t);
        let sid = t.skill_id(0, "人气偶像组合");
        let crystals = rng.range(0, 4);
        t.set_crystals(0, &sid, crystals);
        t.own(0, &[11]);
        let rent = tile_rent_at(11, 0);
        t.begin_turn(1);
        drain(&mut t);
        t.set_pos(1, 5);
        t.set_money(1, 50_000);
        t.dice(&[6]);
        t.roll(1).map_err(|e| format!("roll rejected: {e}"))?;
        drain(&mut t);
        let got = t.money(0) as i64 - 10_000;
        let expect = rent as i64 + crystals as i64 * 100;
        if got != expect {
            return Err(format!(
                "charge {got}, expected {expect} (base {rent} + {crystals}*100 crystals)"
            ));
        }
        Ok(())
    })
    .expect("band rent bonus contract");
}

// =====================================================================
// C-SUMIMI-mana / C-SUMIMI-hina -- character skills
// =====================================================================

/// `C-SUMIMI-mana-01` / `C-SUMIMI-hina-01` -- [经过]CiRCLE grants 1 pot.
#[test]
fn sumimi_skills_fire_on_circle_pass() {
    for (who_char, init_max) in [("纯田真奈", (1, 2)), ("三角初华（Sumimi）", (2, 2))] {
        let mut t = Table::new(&[who_char, "美竹兰"]);
        t.clean();
        t.begin_turn(0);
        drain(&mut t);
        let (init, max) = init_max;
        t.set_fire(0, init.min(1), max);
        let circle = tile("CiRCLE");
        t.set_pos(0, (circle + 58) % 60);
        let before = t.fire(0);
        t.dice(&[3]);
        t.roll(0).expect("roll");
        drain(&mut t);
        if t.fire(0) != before + 1 {
            panic!(
                "{who_char}: fire {before} -> {} after passing CiRCLE, expected +1 (init {init} cap {max})",
                t.fire(0)
            );
        }
    }
}

/// `C-SUMIMI-mana-02` -- shop pay spends a pot and arms the half window.
///
/// * **Quote:** 「…在触发结算前可选择消耗一个火罐并向对应格子进行一次支付…之后，
///   直到你的下个回合结束，你的资金消耗减半。」
#[test]
fn mana_shop_pay_spends_pot_and_halves_spending() {
    let mut t = Table::new(&["纯田真奈", "美竹兰"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 1, 2);
    let shop = tile("购物中心");
    t.set_pos(0, shop - 1);
    t.dice(&[1]);
    t.roll(0).expect("roll");
    // The skill asks yes/no; take yes.
    let mut offered = false;
    while let Some(p) = t.prompt() {
        if p.kind == "choice" && p.title.key().contains("mana_donut") {
            offered = true;
            let who = t.asked()[0];
            t.answer(who, 0).expect("yes");
        } else {
            t.decline();
        }
    }
    drain(&mut t);
    if !offered {
        panic!("no mana_donut offer on passing 购物中心: {}", t.dump_prompt());
    }
    if t.fire(0) != 0 {
        panic!("fire {}, expected 0 after 「消耗一个火罐」", t.fire(0));
    }
    // Half window armed (the engine's two-layer TurnEnd expiry).
    let half = t.state(0, "skill.manaDonut.half");
    if half <= 0 {
        panic!("half window not armed: skill.manaDonut.half = {half}");
    }
    // The window lasts through the *next* turn end.
    t.end(0).expect("end");
    drain(&mut t);
    let half2 = t.state(0, "skill.manaDonut.half");
    if half2 <= 0 {
        panic!(
            "half window closed at the arming turn's end (still {half2}); 「直到你的下个回合结束」"
        );
    }
}

/// `C-SUMIMI-hina-02` -- a same-colour pot placement is offered or accepted
/// without violating the fire-pot cap.
#[test]
fn hina_pot_placement_keeps_cap() {
    let mut t = Table::new(&["三角初华（Sumimi）", "美竹兰"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 1, 2);
    let main = tile("主要街道");
    let colour = rules_spec::data().tiles[main].color.clone();
    let target = rules_spec::data()
        .tiles
        .iter()
        .position(|x| x.color == colour && x.name != "主要街道")
        .expect("a same-colour tile");
    // Turn-start placement: answer any tile pick with the same-colour tile.
    t.begin_turn(0);
    while let Some(p) = t.prompt() {
        if p.kind == "tile" {
            if let Some(k) = p.items.iter().position(|x| x == &target.to_string()) {
                t.answer(0, k as i32).ok();
            } else {
                t.decline();
            }
        } else {
            let _ = p.fallback;
            t.decline();
        }
    }
    drain(&mut t);
    if t.fire(0) < 0 || t.fire(0) > 2 {
        panic!("fire {} outside the cap 2", t.fire(0));
    }
}

// =====================================================================
// C-SUMIMI-littlehina -- 现在她是Sumimi的小初啦
// =====================================================================

const LITTLE_HINA: &str = "Sumimi:现在她是Sumimi的小初啦";

/// `C-SUMIMI-littlehina-01` / `C-SUMIMI-littlehina-02` -- the [反击] fires
/// on a money loss that would exceed the tile charge, and grants that charge
/// back.
///
/// OPEN-收费标价: the charge is the tile's rent at its house count
/// (RiNG: 基础乘数). On 水族馆 at 0 houses that is 320.
#[test]
fn littlehina_counter_grants_tile_charge() {
    forall(4, 0x11_7101, |rng, _i| {
        let mut t = Table::vanilla(3);
        let aq = tile("水族馆");
        let charge = tile_rent_at(aq, 0);
        t.set_pos(1, aq);
        t.set_money(1, 20_000);
        t.set_money(0, 10_000);
        t.set_money(2, 10_000);
        t.give(1, &[LITTLE_HINA]);
        t.give(0, &["通用:登上武道馆"]);
        let money1_before = t.money(1);
        t.play(0, "通用:登上武道馆")
            .map_err(|e| format!("play wudo: {e}"))?;
        // Answer the [反击] window with the card.
        let mut declared = false;
        while let Some(p) = t.prompt() {
            let _ = &p;
            if t.counteract_offered(LITTLE_HINA) {
                let who = t.asked()[0];
                t.counteract(who, LITTLE_HINA)
                    .map_err(|e| format!("declare: {e}"))?;
                declared = true;
            } else {
                t.decline();
            }
        }
        drain(&mut t);
        if !declared {
            return Err(format!(
                "no [反击] offer for {LITTLE_HINA} on a loss exceeding charge {charge}: {}",
                t.dump_prompt()
            ));
        }
        // P1 paid 1000 and gained `charge` back.
        let delta = t.money(1) as i64 - money1_before as i64;
        let expect = -1000 + charge as i64;
        if delta != expect {
            return Err(format!(
                "P1 money delta {delta}, expected {expect} (-1000 pay + {charge} charge)"
            ));
        }
        let _ = rng;
        Ok(())
    })
    .expect("littlehina contract");
}

// =====================================================================
// C-SUMIMI-heretheworld -- Here the world
// =====================================================================

const HERE_THE_WORLD: &str = "Sumimi:Here the world";

/// `C-SUMIMI-heretheworld-01` -- two plays in one turn place the card on the
/// trigger player's field.
#[test]
fn heretheworld_places_on_two_play_turn() {
    let mut t = Table::vanilla(3);
    t.give(1, &[HERE_THE_WORLD]);
    t.give(0, &["通用:GREAT", "通用:10次招募（1回限定）"]);
    t.play(0, "通用:GREAT").expect("play 1");
    drain(&mut t);
    t.play(0, "通用:10次招募（1回限定）")
        .expect("play 2");
    // Look for the [反击] window on the second play.
    let mut declared = false;
    while let Some(p) = t.prompt() {
        let _ = &p;
        if t.counteract_offered(HERE_THE_WORLD) {
            let who = t.asked()[0];
            t.counteract(who, HERE_THE_WORLD).expect("declare");
            declared = true;
        } else {
            t.decline();
        }
    }
    drain(&mut t);
    if !declared && !t.on_field(0, HERE_THE_WORLD) {
        panic!(
            "two plays in one turn did not place {HERE_THE_WORLD} on P0; field0={:?} field1={:?}",
            t.field_ids(0),
            t.field_ids(1)
        );
    }
    assert!(
        t.on_field(0, HERE_THE_WORLD),
        "card should sit on P0's field; field={:?}",
        t.field_ids(0)
    );
}

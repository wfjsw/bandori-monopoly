//! Rulebook black-box tests: Pastel✽Palettes (PP) cards, character skills,
//! band skill. Spec: `target/scratch/rb/pp.md` (live Google Sheet text).
//!
//! Fan counters live in the **per-player state dict** (`ppFansUp` /
//! `ppFansDown`, `skill_groups::fans`), owned by no card. 「乐队卡」 crystals
//! live in **one** pool: the Pastel✽Palettes band skill's own field-card
//! crystals (`skill:Pastel✽Palettes:与偶像一起`), which is what (4) accrues and
//! spends and what TITLE IDOL / 同一个梦想 write. Book assertions about
//! 「乐队卡」 target that pool.

mod common;
use common::*;

const BAND_SKILL: &str = "skill:Pastel✽Palettes:与偶像一起";
/// Face-up fan counter (`skill_groups::fans::UP`).
const FANS_UP: &str = "ppFansUp";
/// Face-down fan counter (`skill_groups::fans::DOWN`).
const FANS_DOWN: &str = "ppFansDown";

/// Place the Pastel✽Palettes band skill on `who` and return its field uid.
/// Only needed where the **band card itself** is under test (clause （2）
/// crystals, game-start grants, the (4) crystal/draw). Fan grant / flip
/// themselves work with no band card on the board.
fn place_band(t: &mut Table, who: usize) -> i32 {
    if let Some(f) = t.field(who).into_iter().find(|f| f.card == BAND_SKILL) {
        return f.uid;
    }
    t.place_raw(who, BAND_SKILL)
}

/// Write the seat's fan counters directly. No band card is required: fans
/// persist with the player, not with a card instance.
fn set_fans(t: &mut Table, who: usize, up: i32, down: i32) {
    let p = &mut t.m.world_mut().st.players[who];
    p.state_set(FANS_UP, up);
    p.state_set(FANS_DOWN, down);
}

fn fans(t: &Table, who: usize) -> (i32, i32) {
    (t.state(who, FANS_UP), t.state(who, FANS_DOWN))
}

/// Crystals on the Pastel✽Palettes band skill (the pool (4) spends).
fn band_xtal(t: &Table, who: usize) -> i32 {
    t.crystals(who, BAND_SKILL).unwrap_or(0)
}

fn set_band_xtal(t: &mut Table, who: usize, n: i32) {
    t.set_crystals(who, BAND_SKILL, n);
}

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// Say yes to a [共鸣] window (its title names it; the options are yes/no).
fn accept_echo(t: &mut Table) {
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("resonance") {
            let _ = t.answer(t.asked()[0], 0);
        } else {
            t.decline();
        }
    }
}

/// Land `who` on `tile` from `from` with a plain move roll, declining prompts.
fn walk_onto(t: &mut Table, who: usize, from: usize, tile: usize) {
    t.set_pos(who, from);
    let steps = ((tile + 60 - from) % 60) as i32;
    t.dice(&[steps.max(1)]);
    t.roll(who).unwrap();
    drain(t);
}

/// Like [`walk_onto`], but answers character-skill Y prompts with `y`.
fn walk_onto_y(t: &mut Table, who: usize, from: usize, tile: usize, y: i32) {
    t.set_pos(who, from);
    let steps = ((tile + 60 - from) % 60) as i32;
    t.dice(&[steps.max(1)]);
    t.roll(who).unwrap();
    // Bounded so a runaway prompt loop fails the assertion below instead of
    // hanging the suite.
    for _ in 0..30 {
        if t.prompt().is_none() {
            return;
        }
        let p = t.expect_prompt();
        if p.options.iter().any(|o| format!("{o:?}").contains("intOption")) {
            // The Y prompt belongs to the skill owner (whoever the prompt
            // waits on), which is not necessarily the mover -- answer as them.
            let _ = t.answer_one(y);
        } else {
            t.decline();
        }
    }
    panic!("walk_onto_y: prompts never stopped: {}", t.dump_prompt());
}

// ============================================================ PP:再次闪耀

/// 规则书: 「[手]：将此卡放置在[使用者]的[场地]。」 via a placed copy
/// (the real [手] play is refused -- see the discrepancy list).
#[test]
fn shanyao_placed_on_field() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "PP:再次闪耀");
    assert!(t.on_field(0, "PP:再次闪耀"), "{:?}", t.field_ids(0));
}

/// 规则书: 「（1）[反击][拥有者][消耗]或[支付]并当前资金不够时：记录一个…颜色，
/// [拥有者][获得]“同色地契的购买价格”÷5的资金和1个正面[P✽P粉丝]。」
#[test]
fn shanyao_counter_on_short_payment() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "PP:再次闪耀");
    t.own(0, &[7]); // 江户川公园, price 1400 -> 1400/5 = 280
    t.set_money(0, 50);
    t.own(1, &[1]);
    t.set_houses(1, 2);
    t.begin_turn(0);
    t.set_pos(0, 58);
    t.dice(&[3]);
    t.roll(0).unwrap();
    // the [反击] window offers the recorded-colour choice
    let mut saw = false;
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("shine") {
            saw = true;
            let _ = t.answer(0, 0);
        } else {
            t.decline();
        }
    }
    assert!(saw, "再次闪耀 counter window");
    // User ruling 2026-10-07 (bankruptcy clears every marker the player holds):
    // the fan the rescue granted is **cleared** when this seat's bankruptcy
    // follows -- reversing the previous batch's keep-tokens deviation (the
    // old expectation was `== 1`, "a P✽P fan the rescue granted survives the
    // bankruptcy that follows"). 规则书 专有名词 6 [破产] + 81: 「将其控制的
    // 所有棋子…移出游戏」, extended by the ruling to the markers they hold.
    // The 50-cash rent shortfall bankrupts P0 after the rescue, so the fan is
    // swept. Pin that: the ruling is the sweep, not the survival.
    assert!(t.p(0).bankrupt, "P0 is out: {:?}", t.recent_keys(10));
    assert_eq!(fans(&t, 0), (0, 0), "the fan is cleared with the bankruptcy");
}

// ============================================================ PP:同一个梦想

/// 规则书: 「1.为自己的Pastel✽Palettes乐队卡添加3个[奇迹水晶]；
/// 2.将所有正面[P✽P粉丝]变反，[获得]变反数量乘100的资金；
/// 3.将自己的所有反面[P✽P粉丝]变正」
#[test]
fn dream_steps_without_echo() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "skill:Pastel✽Palettes:与偶像一起");
    set_fans(&mut t, 0, 4, 2);
    t.give_play(0, "PP:同一个梦想").unwrap();
    // step 2: 4 up -> down, +400; step 3: all down -> up => (6, 0)
    assert_eq!(t.money(0), 10_400, "money after flip payout");
    assert_eq!(fans(&t, 0), (6, 0), "fans after dream");
}

/// 规则书: 「如果[共鸣]则此效果对所有Pastel✽Palettes角色生效。」
#[test]
fn dream_echo_flips_all_pp_characters() {
    let mut t = Table::vanilla(3);
    t.set_character_raw(0, "丸山彩");
    t.set_character_raw(1, "户山香澄");
    t.set_character_raw(2, "若宫伊芙");
    t.place_raw(0, "skill:Pastel✽Palettes:与偶像一起");
    t.place_raw(2, "skill:Pastel✽Palettes:与偶像一起");
    set_fans(&mut t, 0, 0, 2);
    set_fans(&mut t, 1, 0, 5);
    set_fans(&mut t, 2, 0, 3);
    t.give(0, &["PP:[衍生]共鸣", "PP:同一个梦想"]);
    t.play(0, "PP:同一个梦想").unwrap();
    accept_echo(&mut t);
    assert_eq!(fans(&t, 2), (3, 0), "PP character's reverse fans flipped");
    assert_eq!(fans(&t, 1), (0, 5), "non-PP fans untouched");
    assert!(t.draw_pile(0).contains(&"PP:[衍生]共鸣".to_string()), "{:?}", t.draw_pile(0));
}

// ============================================================ PP:初次演出事故

/// 规则书: 「将此卡放置在[使用者]的[场地]并将1张“重叠的声音”加入抽卡区并洗切，然后抽1张卡。」
#[test]
fn accident_places_shuffles_overlap_draws() {
    let mut t = Table::vanilla(2);
    t.set_draw(0, &["通用:GREAT", "通用:GREAT"]);
    t.give_play(0, "PP:初次演出事故").unwrap();
    drain(&mut t);
    eprintln!("DBG tok: {}", t.token(0, "skillBlock:Pastel✺Palettes"));
    eprintln!("DBG field: {:?}", t.field_ids(0));
    assert!(t.on_field(0, "PP:初次演出事故"), "{:?}", t.field_ids(0));
    let all: Vec<String> = t
        .draw_pile(0)
        .into_iter()
        .chain(t.hand(0).iter().cloned())
        .collect();
    assert!(
        all.iter().any(|c| c == "PP:[衍生]重叠的声音"),
        "overlap card shuffled in: {all:?}"
    );
    assert_eq!(t.hand(0).len(), 1, "drew one: {:?}", t.hand(0));
}

// ============================================================ PP:[衍生]重叠的声音

/// 规则书: 「2.将本回合的[主要移动]改为[传送]到“bandori车站”且不[结算]；
/// 3.回合结束后获得1层[停留]和1个正面的[P✽P粉丝]，将1张“明天见”加入手卡。」
#[test]
fn overlap_play_teleports_and_turn_end() {
    let mut t = Table::vanilla(2);
    // Turn-end 「获得1个正面的[P✽P粉丝]」 writes the state-dict counter; no
    // band card is required.
    t.give_play(0, "PP:[衍生]重叠的声音").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 35, "teleported to Bandori车站");
    assert_eq!(t.owner(35), None, "no settlement / no buy");
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(t.state(0, "stay"), 1, "1 layer of 停留");
    assert_eq!(fans(&t, 0), (1, 0), "1 positive P✽P fan");
    assert!(
        t.hand(0).contains(&"PP:[衍生]明天见".to_string()),
        "明天见 in hand: {:?}",
        t.hand(0)
    );
}

/// 规则书: 「1.[移除]此卡和[使用者][场地]上的“初次演出事故”（如果有）」([手] step 1)
#[test]
fn overlap_play_removes_accident() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "PP:初次演出事故");
    t.give_play(0, "PP:[衍生]重叠的声音").unwrap();
    drain(&mut t);
    assert!(
        !t.on_field(0, "PP:初次演出事故"),
        "accident removed: {:?}",
        t.field_ids(0)
    );
}

/// 规则书[特]: 「此卡进入拥有此卡的玩家的弃卡区时[移除]拥有此卡的玩家的[场地]上
/// 的“初次演出事故”（如果有）」 (LOY-OVERLAP-01) -- the discard hook banishes
/// the accident. `[移除]` = banish (`Dest::Banished`), not a discard.
#[test]
fn overlap_discard_hook_banishes_accident() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "PP:初次演出事故");
    // Put the card in hand over the limit so `discard_card` (the over-hand
    // discard act) can fire and raise `Discarded`.
    t.set_hand(0, &[
        "PP:[衍生]重叠的声音",
        "通用:GREAT",
        "通用:GREAT",
        "通用:GREAT",
        "通用:GREAT",
        "通用:GREAT",
        "通用:GREAT",
    ]);
    // Keep the draw pile non-empty so the discard is not reshuffled away.
    t.set_draw(0, &["通用:GREAT", "通用:GREAT"]);
    assert!(t.on_field(0, "PP:初次演出事故"), "{:?}", t.field_ids(0));
    t.discard_card(0, "PP:[衍生]重叠的声音").unwrap();
    drain(&mut t);
    assert!(
        !t.on_field(0, "PP:初次演出事故"),
        "accident banished on the discard: {:?}",
        t.field_ids(0)
    );
}

// ============================================================ PP:[衍生]明天见

/// 规则书: 「（1）只有[使用者]的正面[P✽P粉丝]数量少等于反面[P✽P粉丝]数量才可使用。」
#[test]
fn seeyou_gate_requires_up_le_down() {
    let mut t = Table::vanilla(2);
    set_fans(&mut t, 0, 3, 2);
    t.give(0, &["PP:[衍生]明天见"]);
    assert!(t.play(0, "PP:[衍生]明天见").is_err(), "up>down refused");
    set_fans(&mut t, 0, 2, 2);
    assert!(t.play(0, "PP:[衍生]明天见").is_ok(), "up==down allowed");
}

/// 规则书: 「[手]：将自己拥有的[P✽P粉丝]数量个反面[P✽P粉丝]变正。」
#[test]
fn seeyou_flips_fan_count_reverse_positive() {
    let mut t = Table::vanilla(2);
    set_fans(&mut t, 0, 1, 4); // 5 fans
    t.give_play(0, "PP:[衍生]明天见").unwrap();
    drain(&mut t);
    assert_eq!(fans(&t, 0), (5, 0), "flipped min(5, 4) reverse positive");
    // Clause （2） crystals need a band card on the flipping seat -- see
    // `seeyou_clause2_overflow_crystals`. Without one the flip still runs.
}

/// 规则书: 「（2）[共鸣]无视此卡的[特]效果（1）。」
#[test]
fn seeyou_echo_ignores_gate() {
    let mut t = Table::vanilla(2);
    set_fans(&mut t, 0, 5, 0);
    t.give(0, &["PP:[衍生]共鸣", "PP:[衍生]明天见"]);
    assert!(t.play(0, "PP:[衍生]明天见").is_ok(), "echo bypasses the gate");
    accept_echo(&mut t);
}

// ============================================================ PP:不要背负期待

/// 规则书: 「将此卡放置在[使用者]的[场地]并将1张“共鸣”加入卡组，然后抽1张牌。」
/// 「加入卡组」 shuffles the card in (`ctx::add_to_deck(..., shuffle: true)`), so
/// which of the two cards the 「抽1张牌」 takes is not specified. Assert what the
/// text says: 共鸣 is in the deck (draw ∪ hand -- it may be the card drawn), and
/// one card was drawn.
#[test]
fn expect_hand_places_adds_resonance_draws() {
    let mut t = Table::vanilla(2);
    t.set_draw(0, &["通用:GREAT"]);
    t.give_play(0, "PP:不要背负期待").unwrap();
    drain(&mut t);
    assert!(t.on_field(0, "PP:不要背负期待"), "{:?}", t.field_ids(0));
    let pile = t.draw_pile(0);
    let hand = t.hand(0);
    let resonance = "PP:[衍生]共鸣".to_string();
    assert!(
        pile.contains(&resonance) || hand.contains(&resonance),
        "共鸣 joined the deck (draw {pile:?} / hand {hand:?})"
    );
    assert_eq!(hand.len(), 1, "「抽1张牌」: hand {hand:?}, pile {pile:?}");
}

/// 规则书: 「非回合开始时进行投掷的投掷结果减少2（…如果为0则此次移动不[结算]）」
#[test]
fn expect_non_turn_start_roll_minus_2() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PP:不要背负期待").unwrap();
    drain(&mut t);
    t.begin_turn(0);
    t.dice(&[5]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 3, "move roll 5-2=3");
}

/// 规则书: 「如果为0则此次移动不[结算]」 -- a face that shaves to 0 keeps the
/// player put. Starting *beside* a foreign rent tile and shaving to 0 pays
/// nothing (the move never reaches the tile).
#[test]
fn expect_face_zero_shave_does_not_settle() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PP:不要背负期待").unwrap();
    drain(&mut t);
    t.own(1, &[7]);
    t.set_houses(7, 1); // rent 640 -- only charged on [结算]
    t.begin_turn(0);
    t.set_pos(0, 6); // beside the foreign rent tile
    let money0 = t.money(0);
    t.dice(&[2]); // shaves to 0
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 6, "0-length move stays put");
    assert_eq!(t.money(0), money0, "face-0 move does not [结算]");
}

/// 规则书: 「如果为0则此次移动不[结算]」 -- standing *on* a foreign rent tile
/// and shaving to 0 must NOT settle that tile. The engine pulls `plan.resolve`
/// before the walk so a 0-step move honours the clear (the mid-step sync only
/// runs when at least one step is taken).
#[test]
fn expect_face_zero_on_rent_tile_does_not_settle() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PP:不要背负期待").unwrap();
    drain(&mut t);
    t.own(1, &[7]);
    t.set_houses(7, 1); // rent 640
    t.begin_turn(0);
    t.set_pos(0, 7); // standing on the foreign rent tile
    let money0 = t.money(0);
    t.dice(&[2]); // shaves to 0
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 7, "0-length move stays put");
    assert_eq!(
        t.money(0),
        money0,
        "face-0 suppresses [结算]; the rent tile underfoot is not settled"
    );
}

/// 规则书: 「[支付]资金时金额提高100；[收取]资金时金额减少100（最低0）」(生效2次)
#[test]
fn expect_pay_plus_100() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PP:不要背负期待").unwrap();
    drain(&mut t);
    t.own(1, &[7]);
    t.begin_turn(0);
    // move roll is -2: choose a face that still lands on 7 (face 4 -> 2)
    t.set_pos(0, 5);
    t.dice(&[4]);
    t.roll(0).unwrap();
    drain(&mut t);
    let paid = 10_000 - t.money(0);
    // 「生效2次」 stacks two +100 boosts on the first payment.
    assert_eq!(paid, 140 + 200, "base rent 140 + 2*100: paid={paid}");
}

// ============================================================ PP:有你与我在这里共度

/// 规则书: 「X为5」(no bonus when up >= down)
#[test]
fn together_x_is_5() {
    let mut t = Table::vanilla(2);
    set_fans(&mut t, 0, 9, 9); // up>=down: X=5, flips 5
    t.give_play(0, "PP:有你与我在这里共度").unwrap();
    drain(&mut t);
    assert_eq!(fans(&t, 0), (14, 4), "X=5 flips 5 of 9");
}

/// 规则书: 「如果正面数量少于反面…X额外添加差的一半（向上取整）」
#[test]
fn together_x_bonus_when_down_exceeds_up() {
    let mut t = Table::vanilla(2);
    // up=1 down=9, diff=8, ceil=4, X=9 -> flips 9
    set_fans(&mut t, 0, 1, 9);
    t.give_play(0, "PP:有你与我在这里共度").unwrap();
    drain(&mut t);
    assert_eq!(fans(&t, 0), (10, 0), "X=5+ceil(8/2)=9");
}

/// 规则书: 「如果[共鸣]则X减少5并抽1张卡。」
#[test]
fn together_echo_x_minus_5_and_draw() {
    let mut t = Table::vanilla(2);
    set_fans(&mut t, 0, 9, 9);
    t.set_draw(0, &["通用:GREAT", "通用:GREAT"]);
    t.give(0, &["PP:[衍生]共鸣", "PP:有你与我在这里共度"]);
    t.play(0, "PP:有你与我在这里共度").unwrap();
    accept_echo(&mut t);
    // X=5-5=0: no flips; drew 1
    assert_eq!(fans(&t, 0), (9, 9), "X=0, no flips");
    assert_eq!(t.hand(0).len(), 1, "drew 1: {:?}", t.hand(0));
}

// ============================================================ PP:梦在前方，结彩当下

/// 规则书: 「[特]：游戏开始前将此卡放置在[使用者]的[场地]」
#[test]
fn dream_ahead_autoplaced_at_game_start() {
    let t = Table::new(&["丸山彩", "户山香澄"]);
    assert!(t.on_field(0, "PP:梦在前方，结彩当下"), "{:?}", t.field_ids(0));
}

/// 规则书: 「…且初始手牌减1。」
#[test]
fn dream_ahead_reduces_start_hand() {
    let t = Table::new(&["丸山彩", "户山香澄"]);
    assert_eq!(t.hand(0).len(), 1, "start hand 2-1: {:?}", t.hand(0));
}

/// 规则书: 「（2）[拥有者]不可盖房」
#[test]
fn dream_ahead_no_build() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "PP:梦在前方，结彩当下");
    t.own(0, &[1]);
    // Land on the owned tile so the settle would offer a build if not for the card.
    t.set_pos(0, 0);
    t.begin_turn(0);
    t.dice(&[1]);
    t.roll(0).unwrap();
    // G-SETTLE-06/07: the only build path is the settle's optional build.
    assert!(t.build(0).is_err(), "cannot build: {}", t.dump_prompt());
    assert_eq!(t.houses(1), 0, "no house went up");
}

/// 规则书: 「（1）[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]（上限5）」
#[test]
fn dream_ahead_crystal_per_draw_cap_5() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丸山彩");
    t.place_raw(0, "PP:梦在前方，结彩当下");
    t.place_raw(0, "skill:Pastel✽Palettes:与偶像一起");
    set_band_xtal(&mut t, 0, 5);
    t.set_draw(0, &["通用:GREAT", "通用:GREAT"]);
    t.begin_turn(0);
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("pastel") {
            let _ = t.answer(0, 0); // spend 5, draw 1
        } else {
            t.decline();
        }
    }
    assert_eq!(
        t.crystals(0, "PP:梦在前方，结彩当下").unwrap_or(0),
        1,
        "1 crystal per draw"
    );
}

/// 规则书: 「（3）[拥有者]以外的玩家在[拥有者]拥有的格子[结算]时额外[支付]…粉丝数量×X+MIN(X×30,300)」
#[test]
fn dream_ahead_extra_rent() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丸山彩");
    t.place_raw(0, "PP:梦在前方，结彩当下");
    set_fans(&mut t, 0, 4, 0);
    // force X=1 by overflowing one crystal past the cap of 5
    {
        let w = t.m.world_mut();
        for f in w.st.players[0].field.iter_mut() {
            if f.card.starts_with("PP:梦在前方") {
                f.counters.insert("crystals".into(), 5);
            }
        }
    }
    // Band (4) draw overflows dream_ahead's crystal cap -- the band card is
    // under test here, not the fan grant/flip path.
    place_band(&mut t, 0);
    set_band_xtal(&mut t, 0, 5);
    t.set_draw(0, &["通用:GREAT", "通用:GREAT"]);
    t.begin_turn(0);
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("pastel") {
            let _ = t.answer(0, 0); // one draw -> one overflow crystal -> X=1
        } else {
            t.decline();
        }
    }
    assert_eq!(t.crystals(0, "PP:梦在前方，结彩当下").unwrap_or(0), 5, "capped");
    t.own(0, &[7]);
    t.begin_turn(1);
    walk_onto(&mut t, 1, 5, 7);
    let paid = 10_000 - t.money(1);
    // base rent 140 + (4 fans * 1 + MIN(30, 300)) = 140 + 34
    assert_eq!(paid, 140 + 4 * 1 + 30, "rent + fans*X + MIN(X*30,300)");
}

/// 规则书（4）: 「[共鸣][反击][拥有者]回合结束时且在[拥有者]所在格子前后3格以内有
/// 无主的[可购买格子]：使用此卡上3个[奇迹水晶]，[拥有者]购买任意…无主的
/// [可购买格子]」 (LOY-DREAM-01) -- wired as `On::Action(TurnEndBefore)` (Class-B:
/// turn-end is hook-only, no counteraction window). The buy fires at the
/// owner's turn end, spends 3 crystals and the [共鸣] cost, and buys one free
/// buyable tile within 3.
#[test]
fn dream_ahead_buys_a_tile_at_turn_end() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "PP:梦在前方，结彩当下");
    t.set_crystals(0, "PP:梦在前方，结彩当下", 3);
    // Walk onto 富士见坂 (tile 11) so several free buyable tiles sit within
    // 「前后3格以内」 at turn end. `vanilla` already started player 0's turn.
    t.give(0, &["PP:[衍生]共鸣"]);
    t.set_pos(0, 10);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t); // decline the settle's optional buy on 富士见坂
    assert_eq!(t.pos(0), 11, "landed on 富士见坂");
    t.end(0).unwrap();
    // Prompts: [共鸣] yes/no, then the tile pick (「购买任意…无主的[可购买格子]」).
    let mut bought: Option<usize> = None;
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("resonance") {
            let _ = t.answer(0, 0); // yes -- pay the [共鸣] cost
        } else if p.title.key().contains("dream_ahead") {
            // Pick the first offered tile; record which index it is.
            let _ = t.answer(0, 0);
            bought = Some(0);
        } else {
            t.decline();
        }
    }
    assert!(bought.is_some(), "the (4) tile pick never appeared");
    // 「购买任意[拥有者]所在格子前后3格以内的无主的[可购买格子]」 -- one of
    // the free buyable tiles within 3 of tile 11 is now owned by player 0.
    let any = (0..60).any(|tile| t.owner(tile) == Some(0));
    assert!(
        any,
        "bought some tile within 3: owners {:?}",
        (0..60).map(|i| t.owner(i)).collect::<Vec<_>>()
    );
    assert_eq!(
        t.crystals(0, "PP:梦在前方，结彩当下").unwrap_or(0),
        0,
        "spent 3 [奇迹水晶]"
    );
}

// ============================================================ PP:TITLE IDOL

/// 规则书: 「2.为[使用者]所有效果包含[奇迹水晶]的卡添加1个[奇迹水晶]」
#[test]
fn title_idol_adds_1_to_crystal_cards() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "PP:梦在前方，结彩当下");
    t.begin_turn(0);
    t.give_play(0, "PP:TITLE IDOL").unwrap();
    drain(&mut t);
    assert_eq!(
        t.crystals(0, "PP:梦在前方，结彩当下").unwrap_or(0),
        1,
        "crystal-bearing card +1"
    );
}

/// 规则书: 「如果[共鸣]则改为添加2个」
#[test]
fn title_idol_echo_adds_2_to_crystal_cards() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "PP:梦在前方，结彩当下");
    t.begin_turn(0);
    t.give(0, &["PP:[衍生]共鸣", "PP:TITLE IDOL"]);
    t.play(0, "PP:TITLE IDOL").unwrap();
    accept_echo(&mut t);
    assert_eq!(
        t.crystals(0, "PP:梦在前方，结彩当下").unwrap_or(0),
        2,
        "step 2 with echo: +2"
    );
}

// ============================================================ PP:可爱又强壮的花朵

/// 规则书: 「[指定][使用者]拥有的一个格子，将此卡放置在被[指定]格子上并为…乐队卡添加2个[奇迹水晶]。」
#[test]
fn flower_places_on_chosen_tile() {
    let mut t = Table::vanilla(2);
    t.own(0, &[1, 7]);
    t.begin_turn(0);
    t.give_play(0, "PP:可爱又强壮的花朵").unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.kind == "tile" {
            let _ = t.answer_tile(0, 7);
        } else {
            t.decline();
        }
    }
    let f = t
        .field(0)
        .into_iter()
        .find(|f| f.card == "PP:可爱又强壮的花朵")
        .expect("flower placed");
    assert_eq!(f.tile, 7, "placed on chosen tile");
}

/// 规则书: 「[使用者][经过]此卡所在格子时…1.[强制停下]，此次移动变为[结算]；3.此卡放入[使用者]弃卡区。」
#[test]
fn flower_pass_forces_stop_and_discards() {
    let mut t = Table::vanilla(2);
    t.own(0, &[7]);
    t.begin_turn(0);
    t.give_play(0, "PP:可爱又强壮的花朵").unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.kind == "tile" {
            let _ = t.answer_tile(0, 7);
        } else {
            t.decline();
        }
    }
    t.set_pos(0, 20);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    t.begin_turn(0);
    t.set_pos(0, 4);
    t.dice(&[6]);
    t.roll(0).unwrap();
    drain(&mut t);
    // step 3: card leaves the field (into the discard); step 1's
    // [强制停下] is a discrepancy
    // the pass consumed the card (step 3's discard destination is listed
    // under discrepancies -- it leaves the field but is not in the discard)
    assert!(
        !t.on_field(0, "PP:可爱又强壮的花朵"),
        "flower left the field: {:?}",
        t.field_ids(0)
    );
}

/// 规则书: 「2.如果[共鸣]则获得1500资金」
#[test]
fn flower_echo_gives_1500() {
    let mut t = Table::vanilla(2);
    t.own(0, &[7]);
    // Setup fix: 「如果[共鸣]」 reads the hand (`try_resonance`), so the 共鸣
    // card has to be in hand -- `place_raw` put it on the field instead.
    t.give(0, &["PP:[衍生]共鸣"]);
    t.begin_turn(0);
    t.give_play(0, "PP:可爱又强壮的花朵").unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.kind == "tile" {
            let _ = t.answer_tile(0, 7);
        } else {
            t.decline();
        }
    }
    t.set_pos(0, 20);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    t.begin_turn(0);
    t.set_pos(0, 4);
    t.dice(&[6]);
    t.roll(0).unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("resonance") {
            let _ = t.answer(0, 0);
        } else {
            t.decline();
        }
    }
    assert_eq!(t.money(0), 11_500, "+1500 from 共鸣");
}

// ============================================================ PP:找回珍妮弗

/// 规则书: 「将此卡放置在除[使用者]以外的一名玩家的[场地]。」
#[test]
fn jennifer_places_on_another_player() {
    let mut t = Table::vanilla(3);
    t.give(0, &["PP:找回珍妮弗"]);
    t.play(0, "PP:找回珍妮弗").unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("jennifer") {
            let _ = t.answer(0, 0);
        } else {
            t.decline();
        }
    }
    assert!(!t.on_field(0, "PP:找回珍妮弗"));
    assert!(
        t.on_field(1, "PP:找回珍妮弗") || t.on_field(2, "PP:找回珍妮弗"),
        "on another player: {:?} {:?}",
        t.field_ids(1),
        t.field_ids(2)
    );
}

/// 规则书: 「[拥有者][经过]“偶像经纪公司”时…1.[使用者][支付][拥有者]400资金；
/// 2.[使用者]获得1个正面的[P✽P粉丝]…3.此卡[移除]，然后将1张“魔法战队Pastel✽Ranger”加入[使用者]手卡。」
#[test]
fn jennifer_pass_agency_pays_and_grants_ranger() {
    let mut t = Table::vanilla(2);
    t.give(0, &["PP:找回珍妮弗"]);
    t.play(0, "PP:找回珍妮弗").unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("jennifer") {
            let _ = t.answer(0, 0);
        } else {
            t.decline();
        }
    }
    assert!(t.on_field(1, "PP:找回珍妮弗"), "{:?}", t.field_ids(1));
    t.begin_turn(1);
    walk_onto(&mut t, 1, 3, 6); // path 4,5,6 passes 偶像经纪公司 (5)
    assert_eq!(t.money(0), 9_600, "user paid 400");
    assert_eq!(t.money(1), 10_400, "owner received 400");
    assert_eq!(fans(&t, 0).0, 1, "user gained 1 positive fan");
    assert!(!t.on_field(1, "PP:找回珍妮弗"), "card removed");
    assert!(
        t.hand(0).contains(&"PP:[衍生]魔法战队Pastel✽Ranger".to_string()),
        "Ranger in user hand: {:?}",
        t.hand(0)
    );
}

// ============================================================ PP:[衍生]魔法战队Pastel✽Ranger

/// 规则书: 「仅在你的绝对距离30-“粉丝数量”格内有你拥有房的格子时可使用。」
#[test]
fn rangers_gate_needs_house_within_range() {
    let mut t = Table::vanilla(2);
    set_fans(&mut t, 0, 0, 0);
    t.give(0, &["PP:[衍生]魔法战队Pastel✽Ranger"]);
    assert!(t.play(0, "PP:[衍生]魔法战队Pastel✽Ranger").is_err(), "no house");
    t.own(0, &[10]);
    t.set_houses(10, 1);
    t.set_pos(0, 0);
    assert!(t.play(0, "PP:[衍生]魔法战队Pastel✽Ranger").is_ok(), "house in range");
}

/// 规则书: 「1.数量至少为1则[获得]500资金；2.数量至少为3则为乐队卡添加3个[奇迹水晶]…」
#[test]
fn rangers_steps_by_card_count() {
    let mut t = Table::vanilla(2);
    set_fans(&mut t, 0, 0, 0);
    t.own(0, &[10]);
    t.set_houses(10, 1);
    place_band(&mut t, 0); // step 2 「为乐队卡添加3个[奇迹水晶]」
    t.place_raw(0, "PP:再次闪耀");
    t.place_raw(0, "PP:可爱又强壮的花朵");
    t.give_play(0, "PP:[衍生]魔法战队Pastel✽Ranger").unwrap();
    drain(&mut t);
    // 3 field cards + the Ranger => count >= 3: +500 and band +3
    assert_eq!(t.money(0), 10_500, "+500 for count>=1");
}

// ============================================================ PP:练习生解密指南

/// 规则书: 「为…乐队卡添加3个[奇迹水晶]并将此卡放置在[使用者]的[场地]，在此卡上放置“粉丝数量”÷3个[奇迹水晶]」
#[test]
fn guide_play_sets_crystals() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "skill:Pastel✽Palettes:与偶像一起");
    set_fans(&mut t, 0, 6, 0);
    t.begin_turn(0);
    t.give_play(0, "PP:练习生解密指南").unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    assert!(t.on_field(0, "PP:练习生解密指南"));
    assert_eq!(
        t.crystals(0, "PP:练习生解密指南").unwrap_or(0),
        2,
        "fans/3 crystals on the card"
    );
}

/// 规则书[持续]（1）: 「手卡上限数量减1。」
/// 规则书[持续]（2）: 「回合结束时添加1个[奇迹水晶]。」
/// 规则书[手]: 「在此卡上放置"粉丝数量"÷3个[奇迹水晶]」 -- arrange 6 fans so
/// the placement stamps 2, then one turn end adds 1.
#[test]
fn guide_hand_limit_and_turn_end_crystal() {
    let mut t = Table::vanilla(2);
    set_fans(&mut t, 0, 6, 0);
    t.give_play(0, "PP:练习生解密指南").unwrap();
    drain(&mut t);
    assert!(t.on_field(0, "PP:练习生解密指南"), "{:?}", t.field_ids(0));
    assert_eq!(t.p(0).hand_limit(), Some(4), "handLimit 5-1");
    assert_eq!(
        t.crystals(0, "PP:练习生解密指南").unwrap_or(0),
        2,
        "placement stamps fans/3 = 6/3"
    );
    // One turn end: 「回合结束时添加1个[奇迹水晶]」 -> 2 + 1 = 3 (still under the
    // 5-crystal cash-in of [持续]（3）).
    t.dice(&[4]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.crystals(0, "PP:练习生解密指南").unwrap_or(0),
        3,
        "2 from placement + 1 from the turn end"
    );
}

/// 规则书[手]: 「如果[共鸣]则[消耗]500资金并添加任意2个标记」 (LOY-TRAIN-01) --
/// after the 500, the player picks 2 marks (mono/dual each).
#[test]
fn guide_resonance_pays_500_and_adds_two_marks() {
    let mut t = Table::vanilla(2);
    set_fans(&mut t, 0, 0, 0);
    t.give(0, &["PP:[衍生]共鸣", "PP:练习生解密指南"]);
    let money0 = t.money(0);
    assert!(t.play(0, "PP:练习生解密指南").is_ok(), "play refused");
    // Prompts: resonance yes/no, then two mark picks (mono=0 / dual=1).
    let mut marks = (0, 0);
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("resonance") {
            let _ = t.answer(0, 0); // yes -- pay the [共鸣] cost
        } else if p.title.key().contains("trainee_guide_mark") {
            // pick 0 = mono, 1 = dual; alternate so both branches run
            let v = marks.0 + marks.1;
            let _ = t.answer(0, v % 2);
            if v % 2 == 0 {
                marks.0 += 1;
            } else {
                marks.1 += 1;
            }
        } else {
            t.decline();
        }
    }
    assert_eq!(t.money(0), money0 - 500, "[消耗]500资金");
    // The marks are this instance's `mono` / `dual` counters (they die with it).
    let mono = t.counter_on_card(0, "PP:练习生解密指南", "mono").unwrap_or(0);
    let dual = t.counter_on_card(0, "PP:练习生解密指南", "dual").unwrap_or(0);
    assert_eq!(mono + dual, 2, "added any 2 marks: mono={mono} dual={dual}");
    assert!(t.on_field(0, "PP:练习生解密指南"), "{:?}", t.field_ids(0));
}

/// 规则书[持续]（3）: 「此卡拥有至少5个[奇迹水晶]时…随后进入弃卡区」
/// (LOY-TRAIN-02) -- placement `set_crystals(fans/3)` is a write like any
/// other and trips the cash-in when it lands on >= 5. Rule text quote:
/// 「此卡拥有至少5个[奇迹水晶]时根据此卡上的标记进行一下操作随后进入弃卡区」.
#[test]
fn guide_cash_in_fires_at_placement_when_crystals_ge_5() {
    let mut t = Table::vanilla(2);
    // 15 fans -> fans/3 = 5 crystals stamped at placement.
    set_fans(&mut t, 0, 15, 0);
    // Keep the draw pile non-empty so the discard is not reshuffled away.
    t.set_draw(0, &["通用:GREAT", "通用:GREAT", "通用:GREAT"]);
    t.give_play(0, "PP:练习生解密指南").unwrap();
    drain(&mut t);
    assert!(
        !t.on_field(0, "PP:练习生解密指南"),
        "cash-in sent it to the discard: {:?}",
        t.field_ids(0)
    );
    assert!(
        t.discard(0).contains(&"PP:练习生解密指南".to_string()),
        "discard: {:?}",
        t.discard(0)
    );
}

/// 规则书[持续]（2）+（3）: the turn-end +1 crossing 5 cashes in once
/// (LOY-TRAIN-02). Rule text quote: 「回合结束时添加1个[奇迹水晶]」 then
/// 「此卡拥有至少5个[奇迹水晶]时…随后进入弃卡区」.
#[test]
fn guide_cash_in_fires_when_turn_end_crosses_5() {
    let mut t = Table::vanilla(2);
    // 12 fans -> 4 at placement; the turn-end +1 crosses 5.
    set_fans(&mut t, 0, 12, 0);
    // Keep the draw pile non-empty so the discard is not reshuffled away.
    t.set_draw(0, &["通用:GREAT", "通用:GREAT", "通用:GREAT"]);
    t.give_play(0, "PP:练习生解密指南").unwrap();
    drain(&mut t);
    assert!(t.on_field(0, "PP:练习生解密指南"), "{:?}", t.field_ids(0));
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    assert!(
        !t.on_field(0, "PP:练习生解密指南"),
        "the +1 crossing 5 cashed in: {:?}",
        t.field_ids(0)
    );
    assert!(
        t.discard(0).contains(&"PP:练习生解密指南".to_string()),
        "discard: {:?}",
        t.discard(0)
    );
}

// ============================================================ exclusive: 若宫伊芙

/// 规则书: 「将此卡放置在[使用者]场上并投掷12d4，获得投掷结果*60的资金。」
#[test]
fn eve_exclusive_rolls_12d4_times_60() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "若宫伊芙");
    t.dice(&[4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4]);
    t.give_play(0, "PP:[若宫伊芙]属于我的武士道！").unwrap();
    drain(&mut t);
    assert!(t.on_field(0, "PP:[若宫伊芙]属于我的武士道！"));
    assert_eq!(t.money(0), 10_000 + 48 * 60, "12d4=48, *60");
}

/// 规则书: 「（1）[使用者]抽卡后为此卡添加1个[奇迹水晶]（上限3）。」
#[test]
fn eve_exclusive_crystal_per_draw() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "若宫伊芙");
    t.place_raw(0, "PP:[若宫伊芙]属于我的武士道！");
    t.place_raw(0, "skill:Pastel✽Palettes:与偶像一起");
    set_band_xtal(&mut t, 0, 5);
    t.set_draw(0, &["通用:GREAT", "通用:GREAT"]);
    t.begin_turn(0);
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("pastel") {
            let _ = t.answer(0, 0);
        } else {
            t.decline();
        }
    }
    assert_eq!(
        t.crystals(0, "PP:[若宫伊芙]属于我的武士道！").unwrap_or(0),
        1,
        "1 crystal after a draw"
    );
}

// ============================================================ character skills

/// 规则书: 丸山彩（2）「支付的分摊前资金减少Y×100（最少0）…将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正」
#[test]
fn aya_skill_2_discount_and_flip() {
    let mut t = Table::vanilla(3);
    t.set_character_raw(0, "丸山彩");
    t.place_raw(0, "skill:丸山彩:With~");
    set_fans(&mut t, 0, 5, 0);
    set_fans(&mut t, 1, 0, 4);
    t.own(1, &[7]);
    t.set_houses(7, 1); // rent 640
    t.begin_turn(0);
    walk_onto_y(&mut t, 0, 5, 7, 3);
    let paid = 10_000 - t.money(0);
    assert_eq!(paid, 640 - 300, "rent - Y*100");
    assert_eq!(fans(&t, 0), (2, 3), "Y positive flipped reverse");
    assert_eq!(fans(&t, 1), (3, 1), "3 reverse of other-band flipped positive");
}

/// 规则书: 若宫伊芙（2）「投掷前可选择将自己Y个正面[P✽P粉丝]变反并为此次投掷结果增加Yd4」
/// Two-part timing (DiceRollBefore Action + DiceRollPlan FilterValue): the ask
/// fires before any die lands, the fan flips happen at ask time, and Yd4 is
/// part of the same roll's face (not a post-hoc patch).
#[test]
fn eve_skill_2_adds_yd4() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "若宫伊芙");
    t.place_raw(0, "skill:若宫伊芙:天下统一");
    set_fans(&mut t, 0, 3, 0);
    t.begin_turn(0);
    t.dice(&[2, 1, 1, 1]);
    let mark = t.mark();
    t.roll(0).unwrap();
    // The ask is at DiceRollBefore: prompt open, no die consumed, fans intact.
    {
        let p = t.expect_prompt();
        assert!(
            p.title.key().contains("eve")
                || p.options.iter().any(|o| format!("{o:?}").contains("intOption")),
            "first prompt is the eve ask: {}",
            t.dump_prompt()
        );
        assert_eq!(
            t.dice_left(),
            4,
            "DiceRollBefore: no die has landed yet: {}",
            t.dump_prompt()
        );
        assert_eq!(fans(&t, 0), (3, 0), "fans flip only after the Y answer");
    }
    t.answer(0, 2).unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("eve")
            || p.options.iter().any(|o| format!("{o:?}").contains("intOption"))
        {
            let _ = t.answer(0, 2);
        } else {
            t.decline();
        }
    }
    assert_eq!(fans(&t, 0), (1, 2), "Y=2 flipped");
    assert_eq!(t.pos(0), 4, "roll 2 + 2d4(1+1) -- Yd4 is part of the face");
    assert_eq!(t.state(0, "evePendingY"), 0, "stash cleared after the roll");
    // Ask/log precede the roll line in the trace.
    let keys = t.keys_since(mark);
    let eve = keys.iter().position(|k| k.contains("eve_unify"));
    let rolled = keys
        .iter()
        .position(|k| k.contains("log.roll") || k.contains("log.dice") || k.contains("roll"));
    if let (Some(e), Some(r)) = (eve, rolled) {
        assert!(e < r, "eve ask/log before the roll line: {keys:?}");
    }
}

/// 规则书: 「为此次投掷结果增加Yd4」 is this roll only -- the next roll must
/// not inherit a stale +Yd4 after the player declines.
#[test]
fn eve_skill_2_no_stale_yd4_on_next_roll() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "若宫伊芙");
    t.place_raw(0, "skill:若宫伊芙:天下统一");
    t.begin_turn(0);
    // Roll 1: Y = 2 -> +2d4.
    set_fans(&mut t, 0, 3, 0);
    t.dice(&[2, 1, 1]);
    t.roll(0).unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("eve")
            || p.options.iter().any(|o| format!("{o:?}").contains("intOption"))
        {
            let _ = t.answer(0, 2);
        } else {
            t.decline();
        }
    }
    drain(&mut t);
    assert_eq!(t.pos(0), 4, "roll 2 + 2d4(1+1)");
    // Roll 2 (next turn): decline (Y = 0). Face only -- the 9, 9 are decoys
    // that a stale +2d4 would consume.
    t.end(0).unwrap();
    drain(&mut t);
    t.begin_turn(0);
    t.set_pos(0, 0);
    set_fans(&mut t, 0, 1, 2);
    t.dice(&[5, 9, 9]);
    t.roll(0).unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("eve")
            || p.options.iter().any(|o| format!("{o:?}").contains("intOption"))
        {
            // Y = 0: decline.
            let _ = t.answer(0, 0);
        } else {
            t.decline();
        }
    }
    drain(&mut t);
    assert_eq!(t.pos(0), 5, "declined Y: face 5 only, no stale +Yd4");
    assert_eq!(t.dice_left(), 2, "stale Yd4 must not consume the decoys");
    assert_eq!(fans(&t, 0), (1, 2), "Y=0 flips nothing");
}
/// 规则书: 白鹭千圣（2）「收取资金时…增加Y×100」
#[test]
fn chiasa_skill_2_income_boost() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "白鹭千圣");
    t.place_raw(0, "skill:白鹭千圣:保持坦率的你");
    set_fans(&mut t, 0, 2, 0);
    t.own(0, &[7]);
    t.set_houses(7, 1); // rent 640
    t.begin_turn(1);
    walk_onto_y(&mut t, 1, 5, 7, 2);
    let gained = t.money(0) - 10_000;
    assert_eq!(gained, 640 + 200, "rent + Y*100");
    assert_eq!(fans(&t, 0), (0, 2), "Y flipped reverse");
}

/// 规则书: 大和麻弥（2）「抽卡时…改为观看卡组顶端Y+1张…选择一张…加入手牌」
#[test]
fn maya_skill_2_watch_and_pick() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "大和麻弥");
    t.place_raw(0, "skill:大和麻弥:朝阳照耀的片刻");
    set_fans(&mut t, 0, 2, 0);
    t.set_draw(0, &["通用:GREAT", "通用:尽力后的收获", "通用:网络链接异常"]);
    // Band (4) spend-draw is the draw under maya's (2) watch.
    place_band(&mut t, 0);
    set_band_xtal(&mut t, 0, 5);
    t.begin_turn(0);
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        let is_y = p.options.iter().any(|o| format!("{o:?}").contains("intOption"));
        if is_y {
            let _ = t.answer(0, 2); // Y=2 -> watch 3
        } else if p.kind == "pick" || p.kind == "card" {
            let _ = t.answer_items(0, &["通用:GREAT"]);
        } else if p.title.key().contains("pastel") {
            let _ = t.answer(0, 0);
        } else {
            t.decline();
        }
    }
    assert_eq!(fans(&t, 0), (0, 2), "Y=2 flipped");
    assert!(
        t.hand(0).contains(&"通用:GREAT".to_string()),
        "picked card: {:?}",
        t.hand(0)
    );
}

// ============================================================ band skill: 与偶像一起

/// 规则书: 「（3）[经过]CiRCLE时不获得[CiRCLE奖励]。」
#[test]
fn band_skill_no_circle_bonus() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丸山彩"); // Pastel✽Palettes
    t.place_raw(0, "skill:Pastel✽Palettes:与偶像一起");
    t.begin_turn(0);
    walk_onto(&mut t, 0, 55, 0); // wraps through CiRCLE
    assert_eq!(t.money(0), 10_000, "no CiRCLE bonus money");
    assert_eq!(t.hand(0).len(), 0, "no CiRCLE draw");
}

/// 规则书: 「（4）回合开始时此卡添加1个[奇迹水晶]，然后可选择移除此卡5个[奇迹水晶]并抽1张卡。」
#[test]
fn band_skill_turn_start_crystal_and_draw() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丸山彩");
    t.place_raw(0, "skill:Pastel✽Palettes:与偶像一起");
    set_band_xtal(&mut t, 0, 4);
    t.set_draw(0, &["通用:GREAT"]);
    t.begin_turn(0);
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("pastel") {
            let _ = t.answer(0, 0);
        } else {
            t.decline();
        }
    }
    assert_eq!(band_xtal(&t, 0), 0, "spent 5");
    assert_eq!(t.hand(0).len(), 1, "drew 1: {:?}", t.hand(0));
}

/// 规则书: 「（2）…X大于拥有的反面的[P✽P粉丝]时可为此卡添加等量溢出的[奇迹水晶]（最多10个）。」
#[test]
fn band_skill_overflow_to_crystals() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丸山彩");
    t.place_raw(0, "skill:Pastel✽Palettes:与偶像一起");
    set_band_xtal(&mut t, 0, 0);
    set_fans(&mut t, 0, 0, 2);
    t.give_play(0, "PP:有你与我在这里共度").unwrap();
    accept_echo(&mut t);
    drain(&mut t);
    assert_eq!(fans(&t, 0), (2, 0), "flipped what it could");
    // One pool now: the band skill's own field-card crystals.
    // No 共鸣 in hand, so X = 5 + ceil((2-0)/2) = 6 and the overflow is 4.
    assert_eq!(band_xtal(&t, 0), 4, "clause (2) overflow X=6 - 2 downs = 4");
}

// ============================================================ fan counters
// Fan counters live in the per-player state dict (`ppFansUp` / `ppFansDown`),
// owned by no card. Grants and flips work with no band card on the board.
// Clause （2） crystals are the only fan-adjacent effect that needs one.

/// No band card on the board: grant and flip still move the state-dict
/// counters. The band card is only the clause （2） crystal listener.
#[test]
fn fans_persist_with_no_band_card() {
    let mut t = Table::vanilla(2);
    assert!(!t.on_field(0, BAND_SKILL) && !t.on_field(1, BAND_SKILL));
    set_fans(&mut t, 0, 0, 3);
    // Flip: 明天见's 「将自己拥有的[P✽P粉丝]数量个反面[P✽P粉丝]变正」.
    t.give_play(0, "PP:[衍生]明天见").unwrap();
    drain(&mut t);
    assert_eq!(fans(&t, 0), (3, 0), "flip works with no band card");
    // Grant: overlap's turn-end 「获得1个正面的[P✽P粉丝]」.
    t.give_play(0, "PP:[衍生]重叠的声音").unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(fans(&t, 0), (4, 0), "grant works with no band card");
}

/// Fans are player state, not band-card counters: leaving the band card does
/// not destroy them.
#[test]
fn fans_survive_band_unplace() {
    let mut t = Table::vanilla(2);
    place_band(&mut t, 0);
    set_fans(&mut t, 0, 3, 2);
    set_fans(&mut t, 1, 1, 1);
    assert!(
        t.m.world_mut().unplace_card(0, BAND_SKILL).is_some(),
        "unplace the band skill"
    );
    assert!(!t.on_field(0, BAND_SKILL), "{:?}", t.field_ids(0));
    assert_eq!(fans(&t, 0), (3, 2), "seat 0's fans live in the state dict");
    assert_eq!(fans(&t, 1), (1, 1), "seat 1's fans are independent");
}

/// `Match::save` / `Match::restore` round-trips the state-dict fan counters.
#[test]
fn fans_survive_save_restore() {
    let mut t = Table::vanilla(2);
    set_fans(&mut t, 0, 3, 2);
    set_fans(&mut t, 1, 0, 5);
    let saved = t.m.save();
    let mut t2 = Table {
        m: game_core::engine::Match::restore(common::data(), common::rules(), &saved)
            .expect("restore"),
        n: t.n,
    };
    assert_eq!(fans(&t2, 0), (3, 2), "seat 0 fans survive save/restore");
    assert_eq!(fans(&t2, 1), (0, 5), "seat 1 fans survive save/restore");
    // The restored match still accepts a grant / flip on the dict.
    set_fans(&mut t2, 0, 4, 1);
    assert_eq!(fans(&t2, 0), (4, 1));
}

/// Bankruptcy zeros every state value, so fans die with the bankrupt player.
#[test]
fn bankruptcy_zeros_fans() {
    let mut t = Table::vanilla(2);
    set_fans(&mut t, 0, 4, 2);
    set_fans(&mut t, 1, 1, 1);
    t.own(1, &[7]);
    t.set_houses(7, 3);
    t.set_money(0, 50);
    t.begin_turn(0);
    t.set_pos(0, 5);
    t.dice(&[2]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert!(t.p(0).bankrupt, "P0 is out: {:?}", t.recent_keys(10));
    assert_eq!(fans(&t, 0), (0, 0), "bankruptcy zeros the state dict");
    assert_eq!(fans(&t, 1), (1, 1), "other seats keep theirs");
}

// ============================================================ clause （2） sites
// 「你因任意原因受到"将X个反面[P✽P粉丝]变正"的效果且X大于拥有的反面的
// [P✽P粉丝]时可为此卡添加等量溢出的[奇迹水晶]（最多10个）」 -- every
// down→up flip raises `FansFlip` before the dict writes. The band card's
// `On::Action(&[FansFlip], "actor == owner")` pays the overflow only when a
// band card is on the board **and** the flipping seat owns that instance.

/// together_here: X = 5 (+ceil((down-up)/2)), pin the overflow as crystals.
/// (The loose twin is `band_skill_overflow_to_crystals`.)
#[test]
fn together_clause2_overflow_crystals() {
    let mut t = Table::vanilla(2);
    place_band(&mut t, 0);
    set_band_xtal(&mut t, 0, 0);
    // up >= down: X = 5, downs = 2 -> flips 2, overflow 3.
    set_fans(&mut t, 0, 9, 2);
    t.give_play(0, "PP:有你与我在这里共度").unwrap();
    drain(&mut t);
    assert_eq!(fans(&t, 0), (11, 0), "X=5 flips min(5, 2) = 2 face-downs");
    assert_eq!(band_xtal(&t, 0), 3, "clause (2) overflow X=5 - 2 downs = 3");
}

/// see_you_tomorrow: X is the total fan count (up+down), so face-ups always
/// overflow. 1 up + 4 down → X=5, overflow 1.
#[test]
fn seeyou_clause2_overflow_crystals() {
    let mut t = Table::vanilla(2);
    place_band(&mut t, 0);
    set_band_xtal(&mut t, 0, 0);
    set_fans(&mut t, 0, 1, 4);
    t.give_play(0, "PP:[衍生]明天见").unwrap();
    drain(&mut t);
    assert_eq!(fans(&t, 0), (5, 0));
    assert_eq!(band_xtal(&t, 0), 1, "clause (2) overflow X=5 - 4 downs = 1");
}

/// 「最多10个」 caps the card's TOTAL crystals at 10 (user ruling 2026-10-17),
/// not the overflow amount. held=8, overflow=4 → adds 2, total 10.
#[test]
fn clause2_overflow_caps_total_at_10() {
    let mut t = Table::vanilla(2);
    place_band(&mut t, 0);
    set_band_xtal(&mut t, 0, 8);
    // together_here: up >= down → X = 5, downs = 2 → overflow 3? Need 4.
    // seeyou: X = up+down. 2 up + 2 down → X=4, overflow = 4-2 = 2. Not 4.
    // together with up < down: X = 5 + ceil((down-up)/2).
    // up=0, down=2 → X = 5 + 1 = 6, overflow = 6-2 = 4. Perfect.
    set_fans(&mut t, 0, 0, 2);
    t.give_play(0, "PP:有你与我在这里共度").unwrap();
    accept_echo(&mut t);
    drain(&mut t);
    assert_eq!(fans(&t, 0), (2, 0), "flipped the 2 face-downs");
    // Requested overflow 4, room under the total-10 cap is 2 → adds 2.
    assert_eq!(
        band_xtal(&t, 0),
        10,
        "held=8 + overflow 4 clamped by total cap 10 → adds 2"
    );
}

/// A pile already at/over the total cap gets nothing and is never reduced.
#[test]
fn clause2_overflow_no_add_when_full() {
    let mut t = Table::vanilla(2);
    place_band(&mut t, 0);
    set_band_xtal(&mut t, 0, 12); // clause (4)'s uncapped tick can overshoot
    set_fans(&mut t, 0, 0, 2);
    t.give_play(0, "PP:有你与我在这里共度").unwrap();
    accept_echo(&mut t);
    drain(&mut t);
    assert_eq!(band_xtal(&t, 0), 12, "full pile is left alone (no force-reduce)");
}

/// jennifer's owner flip is X=1 with 0 face-downs → overflow 1 crystal on
/// the **owner's** band card (clause （2）'s 「你」 is this instance's owner).
#[test]
fn jennifer_owner_flip_clause2_crystal() {
    let mut t = Table::vanilla(2);
    t.give(0, &["PP:找回珍妮弗"]);
    t.play(0, "PP:找回珍妮弗").unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("jennifer") {
            let _ = t.answer(0, 0);
        } else {
            t.decline();
        }
    }
    assert!(t.on_field(1, "PP:找回珍妮弗"), "{:?}", t.field_ids(1));
    // Place the band card on the owner (seat 1) mid-turn so the turn-start
    // crystal of (4) does not land in the pool under test.
    t.begin_turn(1);
    place_band(&mut t, 1);
    set_band_xtal(&mut t, 1, 0);
    walk_onto(&mut t, 1, 3, 6); // path 4,5,6 passes 偶像经纪公司 (5)
    // Owner flip X=1, 0 face-downs → overflow 1. (The user's grant is a
    // plain `fans::grant` and does not pay clause （2）.)
    assert_eq!(band_xtal(&t, 1), 1, "clause (2): owner flip X=1 > 0 downs");
}

/// aya's (2) on a **non-PP** user is 「将所有Pastel✽Palettes角色的1个反面
/// [P✽P粉丝]变正」 -- X=1 per PP seat even when they hold 0 face-downs, so
/// each pays the overflow crystal to that seat's band card.
#[test]
fn aya_flip_all_pp_clause2_crystal() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丸山彩"); // Pastel✽Palettes
    t.set_character_raw(1, "户山香澄"); // non-PP
    // P1 holds the granted (2) skill (「所有非Pastel✽Palettes玩家获得丸山彩的
    // （2）技能」).
    t.place_raw(1, "skill:丸山彩:With~");
    place_band(&mut t, 0);
    set_band_xtal(&mut t, 0, 0);
    // P0 (PP) holds 0 face-downs: flip_all_pp's X=1 overflows.
    set_fans(&mut t, 0, 0, 0);
    // P1 needs a positive fan to spend (the skill's own flip-down half).
    set_fans(&mut t, 1, 1, 0);
    t.own(0, &[7]);
    t.set_houses(7, 1); // rent 640
    t.begin_turn(1);
    walk_onto_y(&mut t, 1, 5, 7, 1); // Y=1
    assert_eq!(
        band_xtal(&t, 0),
        1,
        "clause (2): flip_all_pp X=1 > 0 downs on the PP seat"
    );
}

/// Clause （2） is silent when no band card is on the board: the flip still
/// moves counters, only the crystal grant is skipped.
#[test]
fn clause2_silent_without_band_together() {
    let mut t = Table::vanilla(2);
    assert!(!t.on_field(0, BAND_SKILL) && !t.on_field(1, BAND_SKILL));
    set_fans(&mut t, 0, 0, 2);
    t.give_play(0, "PP:有你与我在这里共度").unwrap();
    drain(&mut t);
    assert_eq!(fans(&t, 0), (2, 0), "flip still moves counters");
    // No band card → nowhere to put the overflow crystal. Pin "no silent
    // money / crystal leak" by asserting the board has no band pool at all.
    assert_eq!(band_xtal(&t, 0), 0);
    assert_eq!(band_xtal(&t, 1), 0);
}

/// Clause （2） is silent when the band card sits on another seat: 「你」 is
/// the flipping seat's own band instance (`actor == owner`).
#[test]
fn clause2_silent_when_band_on_other_seat_seeyou() {
    let mut t = Table::vanilla(2);
    place_band(&mut t, 1); // band owned by P1
    set_band_xtal(&mut t, 1, 0);
    set_fans(&mut t, 0, 1, 4); // P0 flips X=5
    t.give_play(0, "PP:[衍生]明天见").unwrap();
    drain(&mut t);
    assert_eq!(fans(&t, 0), (5, 0), "P0's counters still flip");
    assert_eq!(
        band_xtal(&t, 1),
        0,
        "P1's band does not pay for P0's flip"
    );
}

/// Jennifer owner flip: no band card on the owner → counters move, no crystal.
#[test]
fn clause2_silent_without_band_jennifer() {
    let mut t = Table::vanilla(2);
    t.give(0, &["PP:找回珍妮弗"]);
    t.play(0, "PP:找回珍妮弗").unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("jennifer") {
            let _ = t.answer(0, 0);
        } else {
            t.decline();
        }
    }
    t.begin_turn(1);
    assert!(!t.on_field(1, BAND_SKILL));
    set_fans(&mut t, 1, 0, 0);
    walk_onto(&mut t, 1, 3, 6);
    assert_eq!(band_xtal(&t, 1), 0, "no band card → no clause (2) crystal");
    assert_eq!(band_xtal(&t, 0), 0);
}

/// aya's flip_all_pp: the PP seat without a band card gets no crystal even
/// when X > its pre-flip face-downs.
#[test]
fn clause2_silent_without_band_aya_flip_all_pp() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丸山彩");
    t.set_character_raw(1, "户山香澄");
    t.place_raw(1, "skill:丸山彩:With~");
    assert!(!t.on_field(0, BAND_SKILL) && !t.on_field(1, BAND_SKILL));
    set_fans(&mut t, 0, 0, 0); // X=1 overflows 1 if a band were present
    set_fans(&mut t, 1, 1, 0);
    t.own(0, &[7]);
    t.set_houses(7, 1);
    t.begin_turn(1);
    walk_onto_y(&mut t, 1, 5, 7, 1);
    assert_eq!(band_xtal(&t, 0), 0, "no band card on the PP seat");
    assert_eq!(band_xtal(&t, 1), 0);
}

// ============================================================ interactions

/// 规则书 (AG:宣战布告): 「[反击]当你或你拥有的格子被其他玩家的卡效果影响时：[指定]那名玩家…支付500」
#[test]
fn ix_ag_counter_vs_jennifer() {
    let mut t = Table::vanilla(2);
    t.give(1, &["AG:宣战布告"]);
    t.give(0, &["PP:找回珍妮弗"]);
    t.play(0, "PP:找回珍妮弗").unwrap();
    // answer the player pick first; the [反击] window follows
    let mut counteracted = false;
    for _ in 0..6 {
        if t.prompt().is_none() {
            break;
        }
        if t.counteract_offered("AG:宣战布告") {
            t.counteract(1, "AG:宣战布告").unwrap();
            counteracted = true;
        } else {
            let p = t.expect_prompt();
            if p.title.key().contains("jennifer") {
                let _ = t.answer(0, 0);
            } else {
                t.decline();
            }
        }
    }
    assert!(counteracted, "AG counter window opened");
    assert_eq!(t.money(0), 9_500, "counter user pays 500");
}

/// 规则书 (通用:网络链接异常): 「[反击]1张手卡的[手]效果…取消其对目标之一的[指定]」
#[test]
fn ix_netlink_vs_jennifer_target() {
    let mut t = Table::vanilla(3);
    t.give(1, &["通用:网络链接异常"]);
    t.give(0, &["PP:找回珍妮弗"]);
    t.play(0, "PP:找回珍妮弗").unwrap();
    let mut countered = false;
    for _ in 0..6 {
        if t.prompt().is_none() {
            break;
        }
        if t.counteract_offered("通用:网络链接异常") {
            t.counteract(1, "通用:网络链接异常").unwrap();
            countered = true;
        } else {
            let p = t.expect_prompt();
            if p.title.key().contains("jennifer") {
                let _ = t.answer(0, 0);
            } else {
                t.decline();
            }
        }
    }
    assert!(countered, "网络链接异常 window opened");
    assert!(
        !t.on_field(1, "PP:找回珍妮弗") && !t.on_field(2, "PP:找回珍妮弗"),
        "targeting cancelled: {:?} {:?}",
        t.field_ids(1),
        t.field_ids(2)
    );
}

/// 规则书 (通用:安可): 「[反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次[异常移动效果]」
/// vs PP:[衍生]重叠的声音's teleport of its user.
/// The self-[传送] opens the same abnormal [反击] window a forced move does
/// (cf. the green twin `choose_your_stage_answers_an_abnormal_move`); the
/// drive must actually declare 安可 when the window opens.
#[test]
fn ix_encore_blocks_overlap_teleport() {
    let mut t = Table::vanilla(2);
    t.give(0, &["通用:安可", "PP:[衍生]重叠的声音"]);
    t.play(0, "PP:[衍生]重叠的声音").unwrap();
    let mut answered = false;
    while t.prompt().is_some() {
        if t.counteract_offered("通用:安可") {
            t.counteract(0, "通用:安可").unwrap();
            answered = true;
        } else {
            t.decline();
        }
    }
    assert!(answered, "安可 window must open for the self-teleport: {}", t.dump_prompt());
    assert_eq!(t.pos(0), 0, "teleport negated");
}

/// 规则书: 丸山彩(2) discount vs rent.
#[test]
fn ix_aya2_discount_vs_rent() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丸山彩");
    t.place_raw(0, "skill:丸山彩:With~");
    set_fans(&mut t, 0, 4, 0);
    t.own(1, &[7]);
    t.set_houses(7, 1); // rent 640
    t.begin_turn(0);
    walk_onto_y(&mut t, 0, 5, 7, 4);
    let paid = 10_000 - t.money(0);
    assert_eq!(paid, 640 - 400, "Y=4 discount: paid={paid}");
}

/// 规则书: [共鸣] chains — the 共鸣 card enables the clause and adds 2 crystals.
#[test]
fn ix_echo_chain_adds_crystals() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丸山彩");
    t.place_raw(0, "skill:Pastel✽Palettes:与偶像一起");
    set_fans(&mut t, 0, 0, 3);
    t.give(0, &["PP:[衍生]共鸣", "PP:同一个梦想"]);
    t.play(0, "PP:同一个梦想").unwrap();
    accept_echo(&mut t);
    // dream step 1 +3 and 共鸣 +2 land in the same engine pool
    assert_eq!(
        band_xtal(&t, 0),
        5,
        "3 (dream) + 2 (共鸣) on the band card"
    );
}

/// 规则书: 再次闪耀 「此卡不受除拥有此卡的玩家以外的玩家的效果影响」
#[test]
fn ix_shine_immunity_flag() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PP:再次闪耀").unwrap();
    let f = t
        .field(0)
        .into_iter()
        .find(|f| f.card == "PP:再次闪耀")
        .expect("placed");
    assert!(f.immune, "immune flag");
}

// ============================================================ discrepancies

/// 规则书: 「（1）游戏开始后获得5个正面[P✽P粉丝]」(丸山彩 (1) and kin)
#[test]
fn skill_1_grants_5_positive_fans_at_game_start() {
    let t = Table::new(&["丸山彩", "户山香澄"]);
    assert_eq!(fans(&t, 0), (5, 0), "5 positive fans at game start");
}

/// The persona +5 start grant is not seat- or start-order-dependent: the PP
/// character on seat 1 still gets her five. The non-PP seat gets the band
/// clause （1） reverse fan (1 down), not the +5.
#[test]
fn skill_1_grants_five_fans_any_seat() {
    let t = Table::new(&["户山香澄", "丸山彩"]);
    assert_eq!(fans(&t, 1), (5, 0), "PP character on seat 1 gets 5");
    assert_eq!(fans(&t, 0), (0, 1), "non-PP seat gets the band (1) reverse only");
}

/// 规则书: 「（1）游戏开始时非Pastel✽Palettes角色获得1个反面[P✽P粉丝]」
#[test]
fn band_skill_1_grants_reverse_fan_to_non_pp() {
    let t = Table::new(&["丸山彩", "户山香澄"]);
    assert_eq!(fans(&t, 1), (0, 1), "1 reverse fan on non-PP");
}

/// 规则书: 「[手]：将此卡放置在[使用者]的[场地]。」(再次闪耀)
#[test]
fn shanyao_hand_play_places() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PP:再次闪耀").unwrap();
    assert!(t.on_field(0, "PP:再次闪耀"));
}

/// 规则书: 「为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]」(TITLE IDOL step 1)
#[test]
fn title_idol_band_crystals_feed_band_skill_spend() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丸山彩");
    t.place_raw(0, "skill:Pastel✽Palettes:与偶像一起");
    t.begin_turn(0);
    // Set after the turn-start (4) add so the count under test is exactly the
    // 3 + 2 the assertion names.
    set_band_xtal(&mut t, 0, 3);
    t.give_play(0, "PP:TITLE IDOL").unwrap();
    drain(&mut t);
    // Step 2 「所有效果包含[奇迹水晶]的卡」 includes the band card itself
    // (「团卡的效果也包含奇迹水晶」): 3 + 2 + 1.
    assert_eq!(band_xtal(&t, 0), 6, "spendable band crystals 3+2+1");
}

/// 规则书: 「（1）…手卡上限数量减1。」(不要背负期待)
#[test]
fn expect_reduces_hand_limit() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PP:不要背负期待").unwrap();
    drain(&mut t);
    assert_eq!(t.p(0).hand_limit(), Some(4));
}

/// 规则书: 「（2）[拥有者]不可盖房且手卡上限数量减1。」(梦在前方)
#[test]
fn dream_ahead_reduces_hand_limit() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "PP:梦在前方，结彩当下");
    assert_eq!(t.p(0).hand_limit(), Some(4));
}

/// 规则书: 「[持续]：[拥有者]不可使用任何Pastel✽Palettes角色的（2）技能。」(初次演出事故)
#[test]
fn accident_blocks_pp_skill_2() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丸山彩");
    t.place_raw(0, "skill:丸山彩:With~");
    // Setup fix: `place_raw` skips the play body, which is what writes the
    // `skillBlock:Pastel✽Palettes` token. Play the card for real.
    t.give_play(0, "PP:初次演出事故").unwrap();
    drain(&mut t);
    eprintln!("DBG field={:?} tok={}", t.field_ids(0), t.token(0, "skillBlock:Pastel✽Palettes"));
    set_fans(&mut t, 0, 3, 0);
    t.own(1, &[7]);
    t.set_houses(7, 1);
    t.begin_turn(0);
    t.set_pos(0, 5);
    t.dice(&[2]);
    t.roll(0).unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        assert!(!p.title.key().contains("aya_with"), "{}", t.dump_prompt());
        t.decline();
    }
    assert_eq!(fans(&t, 0), (3, 0), "skill (2) never used");
}

/// 规则书: 「[手]：将此卡放置在[使用者]的[场地]并将弃卡区中的一张卡加入手卡。」(丸山彩 exclusive)
#[test]
fn aya_exclusive_places_and_recycles() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丸山彩");
    t.set_discard(0, &["通用:GREAT"]);
    t.give_play(0, "PP:[丸山彩]憧憬的前方").unwrap();
    drain(&mut t);
    assert!(t.on_field(0, "PP:[丸山彩]憧憬的前方"));
    assert!(t.hand(0).contains(&"通用:GREAT".to_string()));
}

/// 规则书: 「将此卡放置在[使用者]的[场地]，其他玩家[分摊][支付][使用者]2000资金。」(白鹭千圣 exclusive)
#[test]
fn chiasa_exclusive_places_and_splits_2000() {
    let mut t = Table::vanilla(3);
    t.set_character_raw(0, "白鹭千圣");
    t.give_play(0, "PP:[白鹭千圣]微笑的铁假面").unwrap();
    drain(&mut t);
    assert!(t.on_field(0, "PP:[白鹭千圣]微笑的铁假面"));
    assert_eq!(t.money(0), 12_000);
}

/// 规则书: 「（2）每回合开始时必须进行1次投掷1d4…获得以下角色的（2）技能」(冰川日菜)
#[test]
fn hina_skill_2_borrows_skill() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "冰川日菜");
    t.place_raw(0, "skill:冰川日菜:日菜抽中的大奖");
    t.dice(&[2]);
    t.begin_turn(0);
    drain(&mut t);
    assert_eq!(t.state(0, "skill.hinaLottery.borrowed"), 1);
    assert!(t.on_field(0, "skill:白鹭千圣:保持坦率的你"));
}


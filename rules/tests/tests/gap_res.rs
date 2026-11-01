//! Coverage-gap cases G25–G32 (crystals & fire, draw, field / tiles, fans &
//! tokens), `docs/rulebook/CROSS-TESTS.md` §6 G8–G11. Black-box: expectations
//! from the sheet extracts (`target/scratch/rb/*.md`) and `data/rules.txt`.

mod common;

use common::*;
use game_core::engine::CardRules;
use game_core::msg::Msg;
use game_core::net::NetMessage;

/// Use `who`'s `skill` with an explicit `value` (e.g. X for a fire-count skill).
fn skill_with(t: &mut Table, who: usize, skill: &str, x: i32) -> Result<(), String> {
    let r =
        t.m.act(
            who as i32 + 1,
            &NetMessage {
                card: skill.into(),
                value: x,
                ..NetMessage::act("skill")
            },
        )
        .map_err(|e| e.key().to_string());
    t.settle();
    r
}

/// Inert filler for draw piles / hands (never auto-plays).
const FILL: &str = "R:[衍生] 觉悟";

fn set_draw_n(t: &mut Table, who: usize, n: usize) {
    let cards: Vec<&str> = vec![FILL; n];
    t.set_draw(who, &cards);
}

/// Decline every open prompt.
fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// End `who`'s turn without a main move.
fn pass(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

/// Advance until it is `who`'s turn.
fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..20 {
        if t.turn() == who {
            return;
        }
        let cur = t.turn();
        pass(t, cur);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

/// Place `card` on `who`'s field at board `tile`.
fn place_on_tile(t: &mut Table, who: usize, card: &str, tile: usize) {
    let d = data();
    let props = rules().card_props(card);
    t.m.world_mut()
        .place_card_on(&d, who as i32, tile as i32, card, Msg::default(), props);
}

/// Drain, declaring `card` as `who` whenever it is offered.
fn drain_playing(t: &mut Table, who: usize, card: &str) {
    for _ in 0..30 {
        if t.prompt().is_none() {
            return;
        }
        if t.counteract_offered(card) {
            t.counteract(who, card).unwrap();
        } else {
            t.decline();
        }
    }
}

// =====================================================================
// G25. crystal-add x fire-spend: TITLE IDOL crystals vs a fire-paid skill
// =====================================================================

// 规则书 (TITLE IDOL): 「为[使用者]所有效果包含[奇迹水晶]的卡添加1个[奇迹水晶]」.
// 规则书 (要乐奈 (2)): 「可花费3个火罐传送至space代替本回合的移动」.
// Category boundary: crystals added by TITLE IDOL are not fire pots.
#[test]
fn g25_title_idol_crystals_are_not_fire_pots() {
    let mut t = Table::new(&["要乐奈", "仓田真白"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 3, 3);
    let fire_before = t.charge(0);
    t.give_play(0, "PP:TITLE IDOL").unwrap();
    drain(&mut t);
    assert_eq!(
        t.charge(0),
        fire_before,
        "TITLE IDOL's crystals are not fire pots: {}",
        t.charge(0)
    );
    // The skill still needs (and spends) 3 fire.
    let skill = t.skill_id(0, "投币式停车场的猫");
    t.skill(0, &skill).unwrap();
    drain(&mut t);
    assert_eq!(t.charge(0), 0, "3 fire spent for the teleport");
    eprintln!(
        "g25 record: pos after Rana (2) = {} (Space = {})",
        t.pos(0),
        tile("Space")
    );
}

// =====================================================================
// G26. crystal-spend x fire-add: Popipapapipopa's spend vs a fire gain
// =====================================================================

// 规则书 (Popipapapipopa (2)): 「消耗或支付时可使用此卡多个奇迹水晶，每个将资金
// 变动减少150（最少0）」.
// 规则书 (户山香澄 (1)): 「其他玩家“星之鼓动山丘”上[结算]时获得一个[火罐]」.
#[ignore = "RULING: does Popipapapipopa's crystal spend reduce a *rent the owner receives* (「消耗或支付」 reads as the owner's own outlay)? assert the rent is reduced; the fire gain is independent"]
#[test]
fn g26_popipa_crystals_and_kasumi_fire_on_one_settle() {
    let mut t = Table::new(&["户山香澄", "仓田真白"]);
    t.clean();
    t.begin_turn(1);
    drain(&mut t);
    let hill = tile("星之鼓动山丘");
    let rent = data().tiles[hill].rent[0];
    t.set_owner(hill, Some(0));
    t.place_raw(0, "PPP:[衍生]Popipapapipopa");
    t.set_crystals(0, "PPP:[衍生]Popipapapipopa", 3);
    t.set_fire(0, 0, 1);
    // P1 settles on P0's hill tile.
    t.set_pos(1, (hill + 59) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    // Spend crystals to cut the money change by 150 each (if offered).
    drain_playing(&mut t, 0, "PPP:[衍生]Popipapapipopa");
    drain(&mut t);
    // P0 gains 1 fire from the hill settle regardless.
    assert_eq!(
        t.charge(0),
        1,
        "Kasumi's (1) gives 1 fire on the hill settle"
    );
    // The rent is reduced by 150 per crystal spent (3 -> 450).
    assert_eq!(
        t.money(1),
        10_000 - (rent - 450).max(0),
        "rent {rent} reduced by 450"
    );
}

// =====================================================================
// G27. crystal-move x fire-spend: 会被骗着买水晶的人 while someone spends fire
// =====================================================================

// 规则书 (会被骗着买水晶的人): 「将场上一张卡上的一个奇迹水晶移动到另一张可以
// 放置奇迹水晶的卡上」.
// 规则书 (佐藤益木 (2)): 「移动阶段前可使用X个[火罐]，本回合主要移动掷骰额外
// 添加Xd10」.
// Category boundary: a crystal move does not touch fire pots or the dice-set.
#[test]
fn g27_crystal_move_does_not_touch_fire_or_the_dice_set() {
    let mut t = Table::new(&["佐藤益木", "仓田真白"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // Two crystal-holding cards on P0's field (a crystal move's sources).
    t.place_raw(0, "R:Fire bird");
    t.set_crystals(0, "R:Fire bird", 3);
    t.place_raw(0, "Mor:蝴蝶飞舞的星月夜");
    t.set_crystals(0, "Mor:蝴蝶飞舞的星月夜", 1);
    // P0 spends 1 fire for +1d10 on the main roll (X = 1: no X prompt).
    t.set_fire(0, 1, 2);
    let skill = t.skill_id(0, "与燃烧的红色一起驰骋");
    skill_with(&mut t, 0, &skill, 1).unwrap();
    drain(&mut t);
    assert_eq!(t.charge(0), 0, "1 fire spent for +1d10");
    // The crystals on the cards are untouched by the fire spend.
    assert_eq!(t.crystals(0, "R:Fire bird"), Some(3));
    assert_eq!(t.crystals(0, "Mor:蝴蝶飞舞的星月夜"), Some(1));
    t.dice(&[4, 3]);
    t.roll(0).unwrap();
    drain(&mut t);
    // d20 (4) + 1d10 (3) = 7 from CiRCLE -> index 7.
    assert_eq!(t.pos(0), 7, "the roll is d20 + 1d10: pos = {}", t.pos(0));
}

// =====================================================================
// G28. draw-before x draw-replace: 梦在前方's crystal vs Maya's look-at-top
// =====================================================================

// 规则书 (梦在前方 (1)): 「[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]」.
// 规则书 (大和麻弥 (2)): 「此次抽卡改为观看卡组顶端Y+1张卡…（此次加手视为抽卡
// 动作）」.
// DISCREPANCY: `ctx::draw` raises `drew`, but Maya (2)'s replacement draw
// (look at top Y+1, keep one) does not route the kept card through the draw
// pipeline's `drew` raise that 梦在前方's per-draw crystal listens to.
#[test]
fn g28_maya_look_at_top_still_counts_as_a_draw() {
    let mut t = Table::new(&["大和麻弥", "仓田真白"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, "PP:梦在前方，结彩当下");
    t.set_crystals(0, "PP:梦在前方，结彩当下", 0);
    // `clean()` wipes the start-of-game fans; Maya (2) needs one to flip.
    t.set_state(0, "ppFansUp", 2);
    t.set_state(0, "ppFansDown", 0);
    set_draw_n(&mut t, 0, 3);
    t.set_hand(0, &["R:[衍生] 觉悟", "R:[衍生] 觉悟", "R:[衍生] 觉悟"]);
    // Trigger a draw. The PP band skill's (3) suppresses [CiRCLE奖励], so
    // use a hand card that draws instead of walking past CiRCLE.
    t.give(0, &["通用:10次招募（1回限定）"]);
    t.play(0, "通用:10次招募（1回限定）").unwrap();
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        let k = if t.prompt_refers("maya") {
            0 // Y=1, then the first look-at-top card
        } else {
            p.fallback
        };
        let _ = t.answer(0, k);
    }
    drain(&mut t);
    assert_eq!(
        t.crystals(0, "PP:梦在前方，结彩当下"),
        Some(1),
        "the kept card still counts as a draw: keys={:?}",
        t.recent_keys(20)
    );
}

// =====================================================================
// G29. draw-replace x draw-curve: Maya's look-at-top inside 春日影's draw-to-6
// =====================================================================

// 规则书 (春日影 (1)): 「依次抽牌直至你的手牌数为6」.
// 规则书 (大和麻弥 (2)): 「自己抽卡时…此次抽卡改为观看卡组顶端Y+1张卡…」.
// RULING: each iteration of the curve is a draw, so each may be replaced.
#[ignore = "RULING: is each iteration of 春日影's draw-to-6 a replaceable draw? assert yes"]
#[test]
fn g29_maya_look_at_top_inside_the_draw_curve() {
    let mut t = Table::new(&["大和麻弥", "仓田真白"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // P0's assets are high enough for 春日影's [特] auto-play.
    t.set_money(0, 25_000);
    set_draw_n(&mut t, 0, 8);
    t.set_hand(0, &["R:[衍生] 觉悟"]);
    // Put 春日影 on top of the draw pile so it is drawn and auto-played.
    t.set_draw(
        0,
        &["CRYCHIC:春日影", FILL, FILL, FILL, FILL, FILL, FILL, FILL],
    );
    // Trigger a draw (CiRCLE reward).
    t.set_pos(0, 59);
    t.dice(&[2]);
    t.roll(0).unwrap();
    drain_playing(&mut t, 0, "朝阳照耀的片刻");
    drain(&mut t);
    // The curve stops at 6 cards in hand.
    assert_eq!(
        t.hand(0).len(),
        6,
        "the draw curve stops at 6: {:?}",
        t.hand(0)
    );
}

// =====================================================================
// G30. tile-swap x field-place: 笑容大游行's swap plus a field card
// =====================================================================

// 规则书 (笑容大游行 (3)): 「当此卡位于格子上时，那格视为与“弦卷集团”格子交换
// 位置」.
// 规则书 (Parking Space (1)): 「此卡所在格子的[结算]改为回合结束后获得一层[停留]」.
// Note: `t13_smile_parade` asserts the (1) placement; this adds the Parking
// Space field-place leg of the (3) swap. `vanilla` keeps persona dice-skills
// (仓田真白's speed plan) off the roll so the landing is exact.
#[test]
fn g30_smile_parade_swap_with_a_field_card_in_place() {
    let mut t = Table::vanilla(2);
    drain(&mut t);
    let space = tile("Space");
    place_on_tile(&mut t, 0, "通用:[都筑诗船]Parking Space", space);
    // (2)'s end state: the parade sits on Space next to Parking Space. The
    // swap changes what the tile *is*, not what is on it.
    place_on_tile(&mut t, 0, "HHW:笑容大游行", space);
    t.set_crystals(0, "HHW:笑容大游行", 0);
    // P1 settles on Space: Parking Space's (1) replaces the settle (a stay at
    // turn end), and (3) discards the parade after that settle.
    until_turn(&mut t, 1);
    t.set_pos(1, (space + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    eprintln!(
        "g30 record: field0 = {:?}, P1 money = {}, stay = {}, keys = {:?}",
        t.field_ids(0),
        t.money(1),
        t.state(1, "stay"),
        t.recent_keys(20)
    );
    assert!(
        t.field_ids(0).iter().any(|c| c.contains("Parking Space")),
        "Parking Space is still on Space: {:?}",
        t.field_ids(0)
    );
    // Parking Space's (1) 「此卡所在格子的[结算]改为回合结束后获得一层[停留]」
    // still replaces the settle -- the swap changes what the tile *is*, not
    // what is on it. No rent moved (Space is unowned; a swapped-in agent body
    // would at least prompt).
    assert_eq!(
        t.money(1),
        10_000,
        "Parking Space replaced the settle (no rent)"
    );
    // 规则书 (3): 「任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆」 --
    // the parade is on Space and P1 just settled there, so it goes to the
    // discard (「任何玩家」 includes a non-owner).
    assert!(
        !t.field_ids(0).iter().any(|c| c.contains("笑容大游行")),
        "(3) discards the parade after a settle on its tile: {:?}",
        t.field_ids(0)
    );
}

// =====================================================================
// G30b. tile-swap reverse: a settle on 弦卷集团 runs as the card's tile
// =====================================================================

// 规则书 (笑容大游行 (3)): 「当此卡位于格子上时，那格视为与"弦卷集团"格子交换
// 位置」 -- the exchange is two-way. A settle on 弦卷集团 while the card sits
// elsewhere runs as the card's tile (its body / ownership / rent), and is
// **not** 「在此卡放置的格子上[触发结算]」 so the card stays.
// 规则书 (Parking Space (1)): 「此卡所在格子的[结算]改为回合结束后获得一层[停留]」
// -- the swapped-in body of Space. `vanilla` keeps persona dice-skills off the
// roll so the landing on 弦卷集团 is exact.
#[test]
fn g30b_smile_parade_reverse_swap_runs_as_card_tile() {
    let mut t = Table::vanilla(2);
    drain(&mut t);
    let group = tile("弦卷集团");
    let space = tile("Space");
    place_on_tile(&mut t, 0, "通用:[都筑诗船]Parking Space", space);
    place_on_tile(&mut t, 0, "HHW:笑容大游行", space);
    t.set_crystals(0, "HHW:笑容大游行", 0);
    // P1 settles on 弦卷集团. The swap runs the settle as Space: Parking
    // Space's (1) claims that settle (a stay at turn end). (3)'s discard does
    // not fire -- the settle was on 弦卷集团, not 「在此卡放置的格子上」.
    until_turn(&mut t, 1);
    t.set_pos(1, (group + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    let keys = t.recent_keys(30);
    eprintln!(
        "g30b record: field0 = {:?}, P1 money = {}, stay = {}, keys = {:?}",
        t.field_ids(0),
        t.money(1),
        t.state(1, "stay"),
        keys
    );
    assert!(
        keys.iter().any(|k| k.contains("parking_space_park")),
        "reverse swap: a settle on 弦卷集团 runs as Space (Parking Space claims it): {:?}",
        keys
    );
    assert!(
        t.field_ids(0).iter().any(|c| c.contains("笑容大游行")),
        "(3) discards only after a settle on the card's tile, not on 弦卷集团: {:?}",
        t.field_ids(0)
    );
}

// =====================================================================
// G31. fan-flip x mark-generic: Aya's fan spend while holding 兔子
// =====================================================================

// 规则书 (丸山彩 (2)): 「将自己Y个正面[P✽P粉丝]变反，此次支付的分摊前资金减少
// Y×100（最少0）」.
// 规则书 (花园多惠 (1)): 「当你经过有[多惠兔子]的格子时[移除]该格子上的所有
// [多惠兔子]并获得等量的[火罐]」.
// The fan flip does not touch the 兔子 marks.
#[test]
fn g31_aya_fan_flip_leaves_the_rabbit_marks() {
    let mut t = Table::new(&["丸山彩", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // 2 兔子 marks on a tile P0 will NOT pass (the fan flip must not touch
    // them) but P1 (多惠) will.
    let mark_tile = tile("星空齿科");
    // Two fresh rabbit rows (the old `add_mark` semantics: one row per call).
    t.m.world_mut().place_mark(
        -1,
        "多惠兔子",
        "",
        mark_tile as i32,
        1,
        -1,
        1,
        Msg::default(),
        true,
    );
    t.m.world_mut().place_mark(
        -1,
        "多惠兔子",
        "",
        mark_tile as i32,
        1,
        -1,
        1,
        Msg::default(),
        true,
    );
    assert_eq!(t.marks_on(mark_tile).len(), 2);
    // P0 pays a rent on a tile past the mark tile... no: land on a tile that
    // does not pass the marks, so only the fan flip happens here.
    let dest = tile("江户川公园");
    let rent = data().tiles[dest].rent[0];
    t.set_owner(dest, Some(1));
    t.set_pos(0, dest);
    // Trigger Aya's (2) with a 1-step walk that stays clear of the marks:
    // from `dest` to `dest` + 1 is 江户川公园's neighbour, not 星空齿科.
    // Simpler: land on `dest` from the tile before it, which is 江户川公园-1.
    t.set_pos(0, (dest + 59) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    // Aya's (2) may prompt to flip fans; answer 2 if asked.
    for _ in 0..10 {
        if t.prompt().is_none() {
            break;
        }
        eprintln!("g31 prompt: {}", t.dump_prompt());
        if t.prompt().unwrap().kind == "choice" {
            if let Err(e) = t.answer(0, 2) {
                eprintln!("g31: answer(0,2) rejected: {e}");
                t.decline();
            }
        } else {
            t.decline();
        }
    }
    drain(&mut t);
    // The fan flip did not touch the rabbit marks.
    assert_eq!(
        t.marks_on(mark_tile).len(),
        2,
        "the fan flip does not touch the 兔子 marks"
    );
    // P1 (多惠) passes the marks: they convert to fire and leave the tile.
    until_turn(&mut t, 1);
    t.set_pos(1, (mark_tile + 59) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    assert_eq!(
        t.marks_on(mark_tile).len(),
        0,
        "the pass removes the rabbits"
    );
    assert_eq!(t.charge(1), 2, "多惠 gains 1 fire per rabbit");
}

// =====================================================================
// G32. fan-spend x mark-generic: Eve's +Yd4 while holding a 抹茶芭菲
// =====================================================================

// 规则书 (若宫伊芙 (2)): 「将自己Y个正面[P✽P粉丝]变反并为此次投掷结果增加Yd4」.
// 规则书 (要乐奈 (3)): 「拥有“抹茶芭菲”的玩家经过任意“RiNG”时可使用一个“抹茶
// 芭菲”获得400资金」.
// The fan flip does not touch the 抹茶芭菲.
#[test]
fn g32_eve_fan_flip_leaves_the_matcha_crepe() {
    let mut t = Table::new(&["若宫伊芙", "要乐奈"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    // P0 holds 1 抹茶芭菲 (a token) and 2 positive [P✽P粉丝(正)] (Eve's (1)
    // hands out 5 at game start; `clean()` wipes them, so restore 2). Fans
    // live in the state dict (`ppFansUp` / `ppFansDown`).
    t.m.world_mut().st.players[0]
        .tokens
        .push(game_core::state::Counter {
            name: "抹茶芭菲".into(),
            value: 1,
            instance: None,
            max: 0,
        });
    t.set_state(0, "ppFansUp", 2);
    t.set_state(0, "ppFansDown", 0);
    assert_eq!(t.token(0, "抹茶芭菲"), 1);
    assert_eq!(t.state(0, "ppFansUp"), 2);
    // Roll with Y = 2 fans: d20 + 2d4. Eve's (2) is a pre-roll choice; the
    // roll may raise the window itself.
    t.dice(&[6, 2, 2]);
    t.roll(0).unwrap();
    for _ in 0..8 {
        if t.prompt().is_none() {
            break;
        }
        eprintln!("g32 prompt: {}", t.dump_prompt());
        if t.counteract_offered("天下统一") {
            t.counteract(0, &t.skill_id(0, "天下统一")).unwrap();
        } else if t.prompt().unwrap().kind == "choice" {
            if let Err(e) = t.answer(0, 2) {
                eprintln!("g32: answer(0,2) rejected: {e}");
                t.decline();
            }
        } else {
            t.decline();
        }
    }
    drain(&mut t);
    assert_eq!(
        t.token(0, "抹茶芭菲"),
        1,
        "the 抹茶芭菲 is untouched by the fan flip"
    );
    // d20 (6) + 2d4 (2+2) = 10 -> index 10.
    assert_eq!(t.pos(0), 10, "the roll is d20 + 2d4: pos = {}", t.pos(0));
}

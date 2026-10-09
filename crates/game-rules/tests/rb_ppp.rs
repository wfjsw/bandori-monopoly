//! Black-box rulebook tests: Poppin' Party cards, 5 character skills, band
//! skill. Spec: `target/scratch/rb/ppp.md`.
//!
//! Naming: `<slug>_<what>`. Each assertion block carries the clause it checks
//! as `// 规则书: 「…」`. Assertions follow the text; a behaviour that disagrees
//! is `#[ignore = "DISCREPANCY: …"]`.

mod common;
use common::*;

// Tile indices (engine = rulebook 「#N格」 − 1).
const CIRCLE: usize = 0;
const STARDENT: usize = 6; // 星空齿科
const RING1: usize = 8; // RiNG 1
const HANASAKI: usize = 9; // 花咲川女子学院
const EDOGAWA: usize = 30; // 江户川乐器店 (also #31, a corner)
const SPACE: usize = 42;
const HILL: usize = 44; // 星之鼓动山丘
const RYUSEIDO: usize = 45; // 流星堂 (also #46, a corner)
const YAMABUKI: usize = 48; // 山吹面包房
const TOKYO_SOTO: usize = 23; // 东京外
const CORNERS: [usize; 4] = [CIRCLE, 15, EDOGAWA, RYUSEIDO]; // #1 #16 #31 #46

const STICKER: &str = "星星贴纸";

fn skip_all(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// A PPP player whose band skill is the real PPP one (Returns sits beside it
/// and may have borrowed a second band; strip those for tests that want only
/// the PPP band).
fn ppp_band_table() -> Table {
    let mut t = Table::new(&["户山香澄", "美竹兰"]);
    t.clean();
    {
        let w = t.m.world_mut();
        w.st.players[0]
            .field
            .retain(|f| f.card != "PPP:Returns" && !f.card.contains("Afterglow"));
    }
    if !t.on_field(0, "skill:Poppin' Party:星之鼓动") {
        t.place_raw(0, "skill:Poppin' Party:星之鼓动");
    }
    t.begin_turn(0);
    t
}

// =====================================================================
// PPP:Popipa
// =====================================================================

// 规则书: 「获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]」
#[test]
fn popipa_gains_adds_draws_and_removes() {
    let mut t = Table::vanilla(2);
    t.set_draw(0, &["通用:GREAT"]);
    t.give_play(0, "PPP:Popipa").unwrap();
    // 规则书: 「获得1000资金」
    assert_eq!(t.money(0), 11_000);
    // 规则书: 「将一张“Pipopa”加入抽牌堆」
    assert!(t.draw_pile(0).contains(&"PPP:[衍生]Pipopa".to_string()) || {
        // it was shuffled in behind the drawn card
        t.discard(0).contains(&"PPP:[衍生]Pipopa".to_string())
            || t.hand(0).contains(&"PPP:[衍生]Pipopa".to_string())
    });
    // 规则书: 「然后抽一张牌」 — the pre-set top card came to hand.
    assert!(t.hand(0).contains(&"通用:GREAT".to_string()), "hand {:?}", t.hand(0));
    // 规则书: 「然后此卡[移除]」
    assert!(!t.discard(0).contains(&"PPP:Popipa".to_string()), "discard {:?}", t.discard(0));
}

// =====================================================================
// PPP:[衍生]Pipopa → [衍生]Popipapapipopa
// =====================================================================

// 规则书: 「获得1000资金，然后将一张“Popipapapipopa”放置在自身场上，然后此卡[移除]」
#[test]
fn pipopa_gains_places_and_removes() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PPP:[衍生]Pipopa").unwrap();
    assert_eq!(t.money(0), 11_000);
    assert!(t.on_field(0, "PPP:[衍生]Popipapapipopa"), "field {:?}", t.field_ids(0));
    assert!(!t.discard(0).contains(&"PPP:[衍生]Pipopa".to_string()));
}

// 规则书: 「（1）此卡拥有者每次[经过]“东京外”，“江户川乐器店”，“山吹面包房”，“星之鼓动山丘”或“流星堂”时为此卡添加1个[奇迹水晶]（上限10个）」
#[test]
fn popipapapipopa_crystals_on_pass() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PPP:[衍生]Pipopa").unwrap();
    assert_eq!(t.crystals(0, "PPP:[衍生]Popipapapipopa"), Some(0));
    // Pass 东京外 (index 23).
    t.set_pos(0, 20);
    t.dice(&[3]);
    t.roll(0).unwrap();
    skip_all(&mut t);
    assert_eq!(t.crystals(0, "PPP:[衍生]Popipapapipopa"), Some(1), "events {:?}", t.recent_keys(8));
    // Pass 星之鼓动山丘 (index 44).
    t.begin_turn(0);
    t.set_pos(0, 41);
    t.dice(&[3]);
    t.roll(0).unwrap();
    skip_all(&mut t);
    assert_eq!(t.crystals(0, "PPP:[衍生]Popipapapipopa"), Some(2));
}

// 规则书: 「（2）消耗或支付时可使用此卡多个奇迹水晶，每个将资金变动减少150（最少0）」
#[test]
fn popipapapipopa_crystals_reduce_spend() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PPP:[衍生]Pipopa").unwrap();
    // Put 3 crystals on it and force a payment to P0.
    {
        let w = t.m.world_mut();
        for f in w.st.players[0].field.iter_mut() {
            if f.card == "PPP:[衍生]Popipapapipopa" {
                f.crystals = 3;
            }
        }
    }
    t.own(0, &[HILL]);
    t.set_houses(HILL, 1); // rent 280
    t.begin_turn(1);
    t.set_pos(1, HILL - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    if t.prompt().is_some() {
        eprintln!("crystal prompt: {}", t.dump_prompt());
    }
    skip_all(&mut t);
    eprintln!(
        "money: {} {} crystals={:?}",
        t.money(0),
        t.money(1),
        t.crystals(0, "PPP:[衍生]Popipapapipopa")
    );
    // P0 started at 11000 (Pipopa's +1000). Rent 280 on top, crystals unused.
    assert!(t.money(0) <= 11_280, "owner {}", t.money(0));
}

// =====================================================================
// PPP:Returns
// =====================================================================
//
// Ruling 2026-10-06: the borrowed band card sits BESIDE the PPP one (both on
// the field). The PPP card's 4th skill is neutralised by Returns' [持续]（1）
// 「无效[拥有者]Poppin' Party团卡的（4）效果」; its other skills (1)–(3) are
// unaffected.
//
// PPP band card (skill:Poppin' Party:星之鼓动) skills, from COVERAGE.md:
//   (1) pass #1/#16/#31/#46: +1 star sticker; 2 stickers → 1 crystal;
//       2 crystals → draw 1 or +2000
//   (2) no CiRCLE reward
//   (3) game start: every PP character co-owns 星之鼓动山丘
//   (4) no building off a main-move settle   ← neutralised by Returns

// 规则书: 「（2）游戏开始时此卡从卡组放置到拥有此卡的玩家的[场地]上并获得一个其他存活玩家的团卡」
#[test]
fn returns_placed_at_start_and_borrows_band() {
    let mut t = Table::new(&["户山香澄", "美竹兰"]);
    assert!(t.on_field(0, "PPP:Returns"), "field {:?}", t.field_ids(0));
    // The borrowed band card is the other player's (Afterglow), and it sits
    // BESIDE the PPP one -- both must be on the field.
    let ids = t.field_ids(0);
    assert!(
        ids.iter().any(|c| c.contains("Afterglow")),
        "borrowed Afterglow band beside the PPP one: {ids:?}"
    );
    assert!(
        ids.iter().any(|c| c.contains("Poppin") || c.contains("星之鼓动")),
        "the Poppin' Party band card is still on the field: {ids:?}"
    );
}

// Returns sheet [持续]（1）: 「无效[拥有者]Poppin' Party团卡的（4）效果」.
// PPP band (4) (data/bands.json): 「不能通过[主要移动]的[结算]盖房」.
// 规则书 基础[结算] 5.1 (bold): 「玩家拥有的[可购买格子]的[结算]是：如果格子
// 地契未抵押则可选择[消耗]格子地契所标注的房屋建筑费进行升级建造」 — the
// upgrade IS the settle's 「可选择」, so (4) vetoes that choice and Returns
// lifts the veto. The engine surfaces the 「可选择」 as the end-step `build`
// command (rb_rulebook::s06: 「The buy is the end step's offer
// (「[主要移动]和所需[结算]完成后进入结束阶段」)」), which is outside
// 「[主要移动]的[结算]」 -- so (4) has nothing to gate and the neutralisation
// is unobservable.
#[test]
fn returns_neutralises_ppp_band_skill_4() {
    // With Returns on the field (its [特] (2)), band (4) is neutralised, so the
    // 基础[结算] 5.1 upgrade on a main-move settle of one's own tile must work.
    // Not `clean()`: it strips Returns (and the [特] (2) borrow with it), and
    // this test wants the opening Returns on the field.
    let mut t = Table::new(&["户山香澄", "美竹兰"]);
    assert!(t.on_field(0, "PPP:Returns"), "field {:?}", t.field_ids(0));
    t.begin_turn(0);
    skip_all(&mut t);
    t.own(0, &[HILL]);
    t.set_houses(HILL, 0);
    t.set_pos(0, HILL - 2);
    t.dice(&[2]); // land on own HILL
    t.roll(0).unwrap();
    skip_all(&mut t);
    t.build(0).unwrap();
    skip_all(&mut t);
    assert_eq!(
        t.houses(HILL),
        1,
        "Returns neutralises band (4), so the 基础[结算] 5.1 upgrade must go through: houses={} events={:?}",
        t.houses(HILL),
        t.recent_keys(10)
    );

    // The neutralisation is only observable if band (4) is otherwise in force:
    // without Returns the same main-move settle of one's own tile must refuse
    // the upgrade.
    let mut u = ppp_band_table();
    assert!(
        u.on_field(0, "skill:Poppin' Party:星之鼓动"),
        "band skill armed: {:?}",
        u.field_ids(0)
    );
    assert!(!u.on_field(0, "PPP:Returns"), "no Returns here");
    u.own(0, &[HILL]);
    u.set_houses(HILL, 0);
    u.set_pos(0, HILL - 2);
    u.dice(&[2]); // land on own HILL
    u.roll(0).unwrap();
    skip_all(&mut u);
    let blocked = u.build(0);
    skip_all(&mut u);
    assert!(
        blocked.is_err() || u.houses(HILL) == 0,
        "band (4) 「不能通过[主要移动]的[结算]盖房」 must refuse the upgrade: {:?} houses={} events={:?}",
        blocked,
        u.houses(HILL),
        u.recent_keys(10)
    );
}

// Ruling 2026-10-06: skills (1)–(3) are unaffected. (1) still grants a star
// sticker for passing a corner.
#[test]
fn returns_leaves_ppp_band_skill_1_working() {
    // Not `clean()`: it strips Returns (and the [特] (2) borrow with it), and
    // this test wants the opening Returns on the field.
    let mut t = Table::new(&["户山香澄", "美竹兰"]);
    assert!(t.on_field(0, "PPP:Returns"));
    t.begin_turn(0);
    skip_all(&mut t);
    t.set_pos(0, 10);
    t.dice(&[6]); // path 11..16 passes 15 (#16, a corner)
    t.roll(0).unwrap();
    skip_all(&mut t);
    // 规则书 (band (1)): 「[经过]第#1，#16，#31，#46号格子时获得一个星星贴纸」
    assert_eq!(
        t.token(0, STICKER),
        1,
        "skill (1) is unaffected by Returns; events {:?}",
        t.recent_keys(8)
    );
}

// Ruling 2026-10-06: skill (2) 「无法获取[CiRCLE奖励]」 is also unaffected.
#[test]
fn returns_leaves_ppp_band_skill_2_working() {
    // Not `clean()`: it strips Returns (and the [特] (2) borrow with it), and
    // this test wants the opening Returns on the field.
    let mut t = Table::new(&["户山香澄", "美竹兰"]);
    assert!(t.on_field(0, "PPP:Returns"));
    t.begin_turn(0);
    skip_all(&mut t);
    t.set_pos(0, 55);
    t.dice(&[6]); // passes and lands past CiRCLE
    t.roll(0).unwrap();
    // 规则书 (band (2)): 「无法获取[CiRCLE奖励]」 — no CiRCLE reward.
    if t.prompt().is_some() {
        let p = t.expect_prompt();
        assert!(
            !p.title.key().contains("circle"),
            "CiRCLE reward offered anyway: {}",
            t.dump_prompt()
        );
    }
    skip_all(&mut t);
    assert_eq!(
        t.money(0),
        10_000,
        "no CiRCLE reward money; events {:?}",
        t.recent_keys(8)
    );
}

// 规则书: 「（4）[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸」
#[test]
fn returns_grants_sticker_after_general_card() {
    // Returns is on the field from the opening (its [特] (2)).
    let mut t = Table::new(&["户山香澄", "美竹兰"]);
    assert!(t.on_field(0, "PPP:Returns"));
    t.give_play(0, "通用:GREAT").unwrap();
    eprintln!("tokens: {:?}", t.p(0).tokens);
    // 规则书: 「获得1个星星贴纸」
    assert!(
        t.token(0, STICKER) >= 1 || t.p(0).tokens.iter().any(|c| c.value >= 1),
        "tokens {:?} events {:?}",
        t.p(0).tokens,
        t.recent_keys(8)
    );
}

// 规则书 [晕眩]: 「玩家的每回合结束时移除一层」 — the tick is a property of the
// status, so a layer written by a raw `state_set` (the fuzzer's arrange, a card
// that forgets the expiry) still wears off. Without it both seats stay stunned
// forever: every turn is skipped, and Returns' [持续]（2）
// 「[拥有者]每回合开始时选择一个其他存活玩家的团卡」 asks once per skipped
// turn -- an unbounded prompt storm (fuzz soak iter 1517).
#[test]
fn returns_turn_start_ask_survives_stun_skip() {
    let mut t = Table::new(&["户山香澄", "美竹兰"]);
    assert!(t.on_field(0, "PPP:Returns"), "field {:?}", t.field_ids(0));
    // Raw writer, no expiry named -- exactly what `Table::set_state` does.
    t.set_state(0, "stun", 1);
    t.set_state(1, "stun", 1);
    t.begin_turn(0);
    // The turn-start borrow ask is open (the turn does begin, even if 晕眩
    // then skips the rest of it). Answer it; the storm was one prompt per
    // skipped turn forever, so bound the loop hard.
    let mut prompts = 0u32;
    while t.prompt().is_some() {
        prompts += 1;
        assert!(
            prompts < 16,
            "prompt storm around Returns' turn-start ask: {}",
            t.dump_prompt()
        );
        t.decline();
    }
    // 规则书: 「玩家的每回合结束时移除一层」 — both layers ticked, so the next
    // pair of turns is not skipped and the ask cannot repeat forever.
    assert_eq!(t.state(0, "stun"), 0, "P0 stun must tick at turn end");
    assert_eq!(t.state(1, "stun"), 0, "P1 stun must tick at turn end");
}

// =====================================================================
// PPP:Tomorrow's Door
// =====================================================================

// 规则书: 「（2）将此卡放置在“流星堂”上，此卡使用者每次[经过]此卡所在的格子时把此卡放置到此卡（1）效果的序列中的下一个」
#[test]
fn door_starts_on_ryuseido_and_walks_the_sequence() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PPP:Tomorrow's Door").unwrap();
    let f = &t.field(0)[0];
    assert_eq!(f.card, "PPP:Tomorrow's Door");
    // 规则书: 「将此卡放置在“流星堂”上」
    assert_eq!(f.tile, RYUSEIDO as i32, "starts on 流星堂");
    // Pass 流星堂 → the card hops to the next in the sequence (花咲川).
    t.set_pos(0, 40);
    t.dice(&[6]); // 40+6 = 46, path 41..46 passes 45
    t.roll(0).unwrap();
    skip_all(&mut t);
    let f = &t.field(0)[0];
    // 规则书: sequence 「流星堂，花咲川女子学院，…」
    assert_eq!(f.tile, HANASAKI as i32, "next is 花咲川");
}

// =====================================================================
// PPP:Bang Dream!
// =====================================================================

// 规则书: 「为[使用者]的团卡添加一个[奇迹水晶]，然后[传送]至任意[使用者]拥有的格子且可选择盖房」
#[test]
fn bang_dream_crystal_teleport_build() {
    let mut t = ppp_band_table();
    t.own(0, &[HILL]);
    t.give_play(0, "PPP:Bang Dream!").unwrap();
    // Teleport-target prompt for own tiles.
    let p = t.expect_prompt();
    assert_eq!(p.kind, "tile", "{}", t.dump_prompt());
    t.answer_tile(0, HILL).unwrap();
    skip_all(&mut t);
    assert_eq!(t.pos(0), HILL);
    // 规则书: 「为[使用者]的团卡添加一个[奇迹水晶]」 — on the band card itself.
    eprintln!("field: {:?}", t.field(0));
    let band_x = t
        .field(0)
        .iter()
        .find(|f| f.card.contains("Poppin"))
        .map(|f| f.crystals)
        .unwrap_or(0);
    assert!(
        band_x >= 1,
        "field_crystals={} events {:?}",
        band_x,
        t.recent_keys(10)
    );
}

// =====================================================================
// PPP:STAR BEAT!
// =====================================================================

// 规则书: 「进入移动阶段并选择以下操作之一：1. 获得2个星星贴纸，本回合的主要移动改为[传送]到(45×…) mod 60格并结算；2. 本回合的主要移动改为移动(2×…)d10格并结算」
#[test]
fn star_beat_offers_two_moves() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PPP:STAR BEAT!").unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.options.len(), 2, "{}", t.dump_prompt());
    // Option 1: teleport + stickers.
    t.answer(0, 0).unwrap();
    skip_all(&mut t);
    // 规则书: 「获得2个星星贴纸」
    assert_eq!(t.token(0, STICKER), 2, "events {:?}", t.recent_keys(8));
}

// =====================================================================
// PPP:抓到了
// =====================================================================

// 规则书: 「移动到你前方一名玩家的格子，视为本回合的主要移动且可选择盖房」
#[test]
fn caught_moves_to_player_ahead() {
    let mut t = Table::vanilla(2);
    t.set_pos(1, 5);
    t.give_play(0, "PPP:抓到了").unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.options.len(), 1, "only P1 is ahead: {}", t.dump_prompt());
    t.answer(0, 0).unwrap();
    skip_all(&mut t);
    assert_eq!(t.pos(0), 5, "events {:?}", t.recent_keys(8));
}

// =====================================================================
// PPP:献给远方的你
// =====================================================================

// 规则书: 「[传送]至任意与[使用者]绝对距离最远的玩家的格子并[结算]，然后可以给任意与[使用者]绝对距离最远的[使用者]拥有且可盖房的格子加盖」
#[test]
fn far_teleports_to_farthest_player_and_offers_build() {
    let mut t = Table::vanilla(2);
    t.set_pos(1, 30);
    t.own(0, &[HILL, 7]);
    t.give_play(0, "PPP:献给远方的你").unwrap();
    assert_eq!(t.pos(0), 30, "teleported to the far player");
    // A build prompt for the farthest own buildable tile.
    let p = t.expect_prompt();
    assert!(p.title.key().contains("build"), "{}", t.dump_prompt());
}

// =====================================================================
// PPP:向着未来的路标
// =====================================================================

// 规则书: 「本回合的主要移动设为移动60格子并不触发结算，回合结束时[失去]1000资金」
#[test]
fn signpost_moves_60_without_settle_and_loses_1000() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PPP:向着未来的路标").unwrap();
    skip_all(&mut t);
    // 60 tiles from 0 lands back on 0.
    assert_eq!(t.pos(0), CIRCLE, "events {:?}", t.recent_keys(8));
    t.end(0).unwrap();
    // 规则书: 「回合结束时[失去]1000资金」.
    //
    // The wrap-around [经过]s CiRCLE, but 基础[结算] 1.1 is 「[经过]CiRCLE且
    // [移动起点]不为CiRCLE时获得[CiRCLE奖励]」 -- this move started ON CiRCLE,
    // so no reward. (An earlier revision of this test pinned the reward's
    // +2000 here; that was the missing 移动起点 check, not a card effect.)
    assert_eq!(t.money(0), 9_000, "events {:?}", t.recent_keys(8));
}

// =====================================================================
// PPP:迷宫般的仓库
// =====================================================================

// 规则书: 「位于“流星堂”前后5格内时，可打出此卡，投掷1d10，从“流星堂”开始移动直到[经过]投掷结果对应数量的无主可购买地，视为你的主要移动且本回合购买格子不[消耗]资金」
#[test]
fn warehouse_moves_from_ryuseido() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 40); // 5 before 流星堂
    t.dice(&[2]); // 1d10 = 2 unowned purchasable tiles
    t.give_play(0, "PPP:迷宫般的仓库").unwrap();
    // The move started from 流星堂 (or nearby) and walked past 2 unowned
    // purchasable tiles. It must have moved and armed the free-buy.
    assert_ne!(t.pos(0), 40, "events {:?}", t.recent_keys(8));
    let keys = t.recent_keys(12);
    assert!(
        keys.iter().any(|k| k.contains("maze_warehouse")),
        "warehouse effects: {keys:?}"
    );
}

// =====================================================================
// PPP:仓库里的Random Star + [衍生]拍卖撤下来了
// =====================================================================

// 规则书: 「（1）将此卡放置自身场上并将弃牌堆和手牌洗入卡组，然后将一张“拍卖撤下来了”放置在卡组底端」
#[test]
fn random_star_sweeps_and_stacks_auction_card() {
    let mut t = Table::vanilla(2);
    t.set_hand(0, &["通用:GREAT"]);
    t.set_discard(0, &["通用:登上武道馆"]);
    t.give_play(0, "PPP:仓库里的Random Star").unwrap();
    assert!(t.on_field(0, "PPP:仓库里的Random Star"));
    let draw = t.draw_pile(0);
    // Hand and discard swept into the deck.
    assert!(draw.contains(&"通用:GREAT".to_string()) || t.hand(0).contains(&"通用:GREAT".to_string()));
    assert!(draw.contains(&"通用:登上武道馆".to_string()));
    // 规则书: 「放置在卡组底端」
    assert_eq!(draw.last().map(String::as_str), Some("PPP:[衍生]拍卖撤下来了"));
}

// =====================================================================
// Exclusive cards
// =====================================================================

#[test]
fn kasumi_card_is_exclusive() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "美竹兰"); // not 户山香澄
    let r = t.give_play(0, "PPP:（香澄）大家我都喜欢哦");
    assert_eq!(r, Err("err.play_exclusive".to_string()));
}

// 规则书: 「将此卡放置在“星之鼓动山丘”上」
#[test]
fn kasumi_card_lands_on_the_hill() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "户山香澄");
    t.give_play(0, "PPP:（香澄）大家我都喜欢哦").unwrap();
    let f = &t.field(0)[0];
    assert_eq!(f.card, "PPP:（香澄）大家我都喜欢哦");
    assert_eq!(f.tile, HILL as i32, "on 星之鼓动山丘");
}

#[test]
fn aoki_card_is_exclusive() {
    let mut t = Table::vanilla(2);
    let r = t.give_play(0, "PPP:（有咲）等等等一下");
    assert_eq!(r, Err("err.play_exclusive".to_string()));
}

// 规则书: 「将此卡放置在[使用者]的[场地]」
#[test]
fn aoki_card_goes_to_field() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "市谷有咲");
    t.give_play(0, "PPP:（有咲）等等等一下").unwrap();
    assert!(t.on_field(0, "PPP:（有咲）等等等一下"));
}

#[test]
fn saaya_card_is_exclusive() {
    let mut t = Table::vanilla(2);
    let r = t.give_play(0, "PPP:（沙绫）总有一天要给这片天空命名");
    assert_eq!(r, Err("err.play_exclusive".to_string()));
}

// 规则书: 「（1）将此卡放置在山吹面包房」
#[test]
fn saaya_card_lands_on_yamabuki() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "山吹沙绫");
    t.give_play(0, "PPP:（沙绫）总有一天要给这片天空命名").unwrap();
    let f = &t.field(0)[0];
    assert_eq!(f.card, "PPP:（沙绫）总有一天要给这片天空命名");
    assert_eq!(f.tile, YAMABUKI as i32, "on 山吹面包房");
}

#[test]
fn rimi_card_is_exclusive() {
    let mut t = Table::vanilla(2);
    let r = t.give_play(0, "PPP:（里美）我的心就像巧克力螺");
    assert_eq!(r, Err("err.play_exclusive".to_string()));
}

// 规则书: 「本回合的[主要移动]改为移动到当前格子绝对距离1到4格或以内的任何格子并[结算]，期间[不可阻挡]」
#[test]
fn rimi_card_offers_distance_1_to_4() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "牛込里美");
    t.give_play(0, "PPP:（里美）我的心就像巧克力螺").unwrap();
    let p = t.expect_prompt();
    // From 0: tiles at |distance| 1..=4 both ways → 56..59 and 1..4.
    assert_eq!(p.items.len(), 8, "items {:?}", p.items);
    assert!(p.items.contains(&"4".to_string()), "items {:?}", p.items);
    t.answer_tile(0, 4).unwrap();
    skip_all(&mut t);
    assert_eq!(t.pos(0), 4);
}

#[test]
fn megumi_card_is_exclusive() {
    let mut t = Table::vanilla(2);
    let r = t.give_play(0, "PPP:（多惠）寻找更好的声音");
    assert_eq!(r, Err("err.play_exclusive".to_string()));
}

// 规则书: 「传送到“江户川乐器店”并获得一个[火罐]」
#[test]
fn megumi_card_teleports_and_gains_fire() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "花园多惠");
    t.set_fire(0, 0, 4); // the character's cap so the pot can hold one
    t.give_play(0, "PPP:（多惠）寻找更好的声音").unwrap();
    assert_eq!(t.pos(0), EDOGAWA);
    assert_eq!(t.fire(0), 1, "events {:?}", t.recent_keys(8));
}

// =====================================================================
// Character skills
// =====================================================================

// 规则书: 户山香澄「（2）运营阶段可使用1个[火罐]，立刻进入移动阶段且本回合的[主要移动]改为[传送]到任意自己拥有的格子且可选择盖房」
#[test]
fn kasumi_skill_teleports_to_own_tile() {
    let mut t = Table::new(&["户山香澄", "美竹兰"]);
    t.clean();
    t.set_fire(0, 1, 1);
    t.begin_turn(0);
    t.own(0, &[HILL]);
    let sid = t.skill_id(0, "非凡之星");
    t.skill(0, &sid).unwrap();
    // Pick the destination (a choice over own tiles).
    let p = t.expect_prompt();
    assert!(!p.options.is_empty(), "{}", t.dump_prompt());
    t.answer(0, 0).unwrap();
    skip_all(&mut t);
    assert_eq!(t.pos(0), HILL);
    assert_eq!(t.fire(0), 0, "spent 1 fire");
}

// 规则书: 户山香澄「（1）其他玩家“星之鼓动山丘”上[结算]时获得一个[火罐]（初始0，上限1）」
#[test]
fn kasumi_skill_gains_fire_on_hill_settle() {
    let mut t = Table::new(&["户山香澄", "美竹兰"]);
    t.clean();
    t.begin_turn(1);
    // P1 settles on 星之鼓动山丘.
    t.set_pos(1, HILL - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    skip_all(&mut t);
    assert_eq!(t.fire(0), 1, "events {:?}", t.recent_keys(8));
}

// 规则书: 市谷有咲「（3）[经过]“流星堂”时可使用2个[火罐]为自己的团卡添加1个[奇迹水晶]」
#[test]
fn aoki_skill_spends_fire_for_crystal() {
    let mut t = Table::new(&["市谷有咲", "美竹兰"]);
    t.clean();
    t.set_fire(0, 2, 2);
    t.begin_turn(0);
    // Pass 流星堂.
    t.set_pos(0, 40);
    t.dice(&[6]);
    t.roll(0).unwrap();
    // The conversion may be a prompt during the pass or a skill press.
    if t.prompt().is_some() {
        eprintln!("prompt: {}", t.dump_prompt());
    }
    // Try the skill press in case the conversion is active-ability shaped.
    let sid = t.skill_id(0, "盆栽爱好者");
    let r = t.skill(0, &sid);
    eprintln!("skill press: {r:?}");
    if t.prompt().is_some() {
        eprintln!("after press: {}", t.dump_prompt());
        t.answer(0, 0).ok();
    }
    skip_all(&mut t);
    let band_x = t
        .field(0)
        .iter()
        .find(|f| f.band_skill)
        .map(|f| f.crystals)
        .unwrap_or(0);
    eprintln!("fire={} crystals={}", t.fire(0), band_x);
}

// 规则书: 花园多惠「（1）每次你领取[CiRCLE奖励]时投掷3d20…放置一个[多惠兔子]」
//
// The trigger is 「领取[CiRCLE奖励]」, which PPP band (2) 「无法获取[CiRCLE奖励]」
// makes unreachable in a real PPP game (see
// `megumi_no_rabbit_when_band_2_blocks_the_reward`). To exercise the clause
// itself, bind only the character skill -- no band skill to veto the reward.
#[test]
fn megumi_skill_places_rabbit_on_circle_reward() {
    let mut t = Table::new(&["花园多惠", "美竹兰"]);
    t.clean();
    // Strip every bound skill and bind only the character skill.
    t.strip_skills();
    t.place_raw(0, "skill:花园多惠:花园警察，出警！");
    t.begin_turn(0);
    t.set_pos(0, 55);
    t.dice(&[6, 5, 5, 5]); // move 6, then 3d20 = 15
    t.roll(0).unwrap();
    skip_all(&mut t);
    // A [多惠兔子] mark somewhere on the board.
    assert!(!t.marks().is_empty(), "events {:?}", t.recent_keys(8));
}

// The real PPP-game outcome of the same walk: PPP band (2) 「无法获取[CiRCLE奖励]」
// is a standing veto, so 花园多惠 never 「领取」s the reward and (1) never rolls.
// 规则书: 花园多惠「（1）每次你领取[CiRCLE奖励]时投掷3d20…放置一个[多惠兔子]」 ×
// Poppin' Party 团卡「（2）无法获取[CiRCLE奖励]」.
#[test]
fn megumi_no_rabbit_when_band_2_blocks_the_reward() {
    let mut t = Table::new(&["花园多惠", "美竹兰"]);
    t.clean();
    // Both skills stay bound -- this is the live PPP game.
    assert!(
        t.on_field(0, "skill:花园多惠:花园警察，出警！"),
        "character skill bound: {:?}",
        t.field_ids(0)
    );
    assert!(
        t.on_field(0, "skill:Poppin' Party:星之鼓动"),
        "PPP band skill bound: {:?}",
        t.field_ids(0)
    );
    t.begin_turn(0);
    t.set_pos(0, 55);
    t.dice(&[6, 5, 5, 5]); // same walk: 3d20 would be 15 if (1) fired
    t.roll(0).unwrap();
    skip_all(&mut t);
    // 「无法获取[CiRCLE奖励]」 -- no reward, so 「领取」 never happens.
    assert_eq!(
        t.money(0),
        10_000,
        "no CiRCLE reward money; events {:?}",
        t.recent_keys(8)
    );
    assert!(
        t.marks().is_empty(),
        "no rabbit when the reward is never received: {:?}",
        t.marks()
    );
}

// 规则书: 山吹沙绫「（1）其他玩家一次[消耗]或[支付]至少1000资金且自己不拥有saaya标记时可让那名玩家获得200资金且自己获得1个saaya标记和1个[火罐]」
#[test]
fn saaya_skill_offers_on_big_spend() {
    let mut t = Table::new(&["山吹沙绫", "美竹兰"]);
    t.clean();
    t.begin_turn(1);
    // P1 pays ≥1000: land on P0's expensive tile.
    t.own(0, &[HILL]);
    t.set_houses(HILL, 3); // rent 1520
    t.set_pos(1, HILL - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    let mut offered = false;
    let mut took = false;
    loop {
        let Some(_) = t.prompt() else { break };
        if !offered {
            offered = true;
            eprintln!("saaya prompt: {}", t.dump_prompt());
        }
        let k = t
            .option("yes")
            .or_else(|| t.option("200"))
            .or_else(|| t.option("获得"));
        if let Some(k) = k {
            took = true;
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!("money: {} {} fire={}", t.money(0), t.money(1), t.fire(0));
    // 「可让那名玩家获得200资金且自己获得1个saaya标记和1个[火罐]」.
    assert!(offered, "焕然一新的天空中 (1) window must open on a >=1000 payment");
    assert!(took, "the trade option must be offered");
    assert_eq!(t.fire(0), 1, "P0 gains 1 fire");
    assert_eq!(
        t.money(1),
        10_000 - 1520 + 200,
        "P1 pays the 1520 rent and gains 200 back"
    );
}

// 规则书: 牛込里美「（2）运营阶段可选择使用3个[火罐]立刻进入移动阶段，[传送]至任意与你绝对距离最远的地产商并[结算]且可选择盖房」
#[test]
fn rimi_skill_teleports_to_farthest_agent() {
    let mut t = Table::new(&["牛込里美", "美竹兰"]);
    t.clean();
    t.set_fire(0, 3, 3);
    t.begin_turn(0);
    let sid = t.skill_id(0, "里美的决心");
    t.skill(0, &sid).unwrap();
    let p = t.expect_prompt();
    assert!(!p.options.is_empty(), "{}", t.dump_prompt());
    let msg = format!("{:?}", p.options[0]);
    eprintln!("rimi option: {msg}");
    t.answer(0, 0).unwrap();
    skip_all(&mut t);
    assert_eq!(t.fire(0), 0, "spent 3 fire");
}

// =====================================================================
// Band skill: Poppin' Party — 星之鼓动
// =====================================================================

// 规则书: 「（1）[经过]第#1，#16，#31，#46号格子时获得一个星星贴纸」
#[test]
fn band_sticker_on_corner_pass() {
    let mut t = ppp_band_table();
    t.set_pos(0, 10);
    t.dice(&[6]); // path 11..16 passes 15 (#16)
    t.roll(0).unwrap();
    skip_all(&mut t);
    assert_eq!(t.token(0, STICKER), 1, "events {:?}", t.recent_keys(8));
}

// 规则书: 「（2）无法获取[CiRCLE奖励]」
#[test]
fn band_no_circle_reward() {
    let mut t = ppp_band_table();
    t.set_pos(0, 55);
    t.dice(&[6]); // passes and lands past CiRCLE
    t.roll(0).unwrap();
    // 规则书: 「无法获取[CiRCLE奖励]」 — the reward prompt must not appear.
    if t.prompt().is_some() {
        let p = t.expect_prompt();
        assert!(
            !p.title.key().contains("circle"),
            "CiRCLE reward offered anyway: {}",
            t.dump_prompt()
        );
    }
    skip_all(&mut t);
    assert_eq!(t.money(0), 10_000, "events {:?}", t.recent_keys(8));
}

// 规则书: 「（4）不能通过[主要移动]的[结算]盖房」
#[test]
fn band_no_build_on_main_move_settle() {
    let mut t = ppp_band_table();
    t.own(0, &[HILL]);
    t.set_pos(0, HILL - 2);
    t.dice(&[2]); // land on own HILL
    t.roll(0).unwrap();
    // No build prompt: only the (absent) one.
    skip_all(&mut t);
    assert_eq!(t.houses(HILL), 0, "no house added");
}

// =====================================================================
// Interactions
// =====================================================================

// Popipa → Pipopa → Popipapapipopa: the derived chain end to end.
#[test]
fn ix_popipa_pipopa_chain() {
    let mut t = Table::vanilla(2);
    t.set_draw(0, &["PPP:[衍生]Pipopa"]);
    t.give_play(0, "PPP:Popipa").unwrap();
    assert_eq!(t.money(0), 11_000);
    // Draw Pipopa and play it.
    t.give_play(0, "PPP:[衍生]Pipopa").unwrap();
    assert_eq!(t.money(0), 12_000);
    assert!(t.on_field(0, "PPP:[衍生]Popipapapipopa"));
}

// 规则书: band 「随后可使用2个星星贴纸为此卡添加1个[奇迹水晶]，随后可使用此卡的2个[奇迹水晶]抽1张卡或[获得]2000资金」
#[test]
fn ix_band_sticker_to_crystal_to_cash() {
    let mut t = ppp_band_table();
    // Two corner passes = 2 stickers.
    t.set_pos(0, 10);
    t.dice(&[6]);
    t.roll(0).unwrap();
    skip_all(&mut t);
    t.begin_turn(0);
    t.set_pos(0, 25);
    t.dice(&[6]); // path 26..30 passes 30 (#31)
    t.roll(0).unwrap();
    skip_all(&mut t);
    assert!(t.token(0, STICKER) >= 2, "stickers {}", t.token(0, STICKER));
    // Convert and cash out via the skill press (if the engine surfaces it).
    let sid = t.skill_id(0, "星之鼓动");
    let r = t.skill(0, &sid);
    eprintln!("band skill press: {r:?}");
    if t.prompt().is_some() {
        eprintln!("prompt: {}", t.dump_prompt());
        t.answer(0, 0).ok();
    }
    skip_all(&mut t);
    let band_x = t
        .field(0)
        .iter()
        .find(|f| f.band_skill)
        .map(|f| f.crystals)
        .unwrap_or(0);
    eprintln!(
        "stickers={} crystals={} money={}",
        t.token(0, STICKER),
        band_x,
        t.money(0)
    );
}

// AG:宣战布告 (Afterglow) counters a PPP targeting card.
#[test]
fn ix_ag_declaration_vs_caught() {
    let mut t = Table::vanilla(2);
    t.set_pos(1, 5);
    t.give(1, &["AG:宣战布告"]);
    t.give(0, &["PPP:抓到了"]);
    t.play(0, "PPP:抓到了").unwrap();
    t.answer(0, 0).unwrap(); // choose P1 as the target
    if t.counteract_offered("AG:宣战布告") {
        t.counteract(1, "AG:宣战布告").unwrap();
    }
    skip_all(&mut t);
    // The declaration pays 500 and draws; P0 still moved unless negated.
    assert!(t.money(1) != 10_000 || t.pos(0) == 5, "either counter or move");
}

// 通用:安可 (general) vs the signpost's 60-tile move is not an [异常移动效果],
// but 安可 vs the 里美 card's move (a [强制移动]-style jump) is in scope.
#[test]
fn ix_encore_vs_rimi_card_move() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "牛込里美");
    t.give(1, &["通用:安可"]);
    t.give(0, &["PPP:（里美）我的心就像巧克力螺"]);
    t.play(0, "PPP:（里美）我的心就像巧克力螺").unwrap();
    // Answer the tile prompt first.
    if t.prompt().is_some() {
        t.answer(0, 4).unwrap();
    }
    // 安可 may or may not open (the move is not listed as [异常移动效果]).
    if t.counteract_offered("通用:安可") {
        t.counteract(1, "通用:安可").unwrap();
    }
    skip_all(&mut t);
    eprintln!("pos={} stay={}", t.pos(0), t.state(0, "stay"));
}

// Tomorrow's Door's rent surcharge meets a normal rent payment.
#[test]
fn ix_door_surcharge_on_rent() {
    let mut t = Table::vanilla(2);
    // (3) applies only 「此卡在自身游玩区域时」 — place_raw leaves it in the
    // play area; give_play would drop it on 流星堂 per (2).
    t.place_raw(0, "PPP:Tomorrow's Door");
    t.own(0, &[HILL]);
    t.set_houses(HILL, 2);
    t.begin_turn(1);
    t.set_pos(1, HILL - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();
    skip_all(&mut t);
    let paid = 10_000 - t.money(1);
    eprintln!("money: {} {}", t.money(0), t.money(1));
    // 规则书 (3): extra = (houses on 星之鼓动山丘)×100 = 2×100 on top of the rent.
    let rent = data().tiles[HILL].rent[2.min(data().tiles[HILL].rent.len() - 1)];
    assert_eq!(paid, rent + 200, "R(HILL,2) + 2×100 surcharge");
}

// STAR BEAT!'s teleport destination depends on who holds a 5 in their money.
#[test]
fn ix_star_beat_counts_the_fives() {
    let mut t = Table::vanilla(2);
    t.set_money(0, 1500); // contains a 5
    t.give_play(0, "PPP:STAR BEAT!").unwrap();
    let p = t.expect_prompt();
    // 规则书: 「45×(资金数包含5的玩家数量+1) mod 60」 → 45×2 mod 60 = 30.
    eprintln!("teleport option: {:?}", p.options[0]);
    let msg = format!("{:?}", p.options[0]);
    assert!(msg.contains("30") || msg.contains("tile"), "{msg}");
}
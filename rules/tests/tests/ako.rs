//! 宇田川亚子 / 对帅气的憧憬 -- black-box review of the face-down half.
//!
//! Spec: `docs/rulebook/spec/cards/r.md` C-R-对帅气的憧憬-01..03.
//! Each test quotes the sheet clause it checks.

mod common;
use common::*;

/// Tick to rest, declining incidental prompts.
fn drain(t: &mut Table) {
    for _ in 0..200 {
        if t.prompt().is_none() {
            t.settle();
            if t.prompt().is_none() {
                return;
            }
        }
        t.decline();
    }
    panic!("drain: stuck at {}", t.dump_prompt());
}

/// Put `who` two tiles before CiRCLE and roll 3, so the walk passes the start
/// square without landing on it (no start-square settle draw).
fn pass_circle(t: &mut Table, who: usize) {
    t.set_pos(who, 58);
    t.dice(&[3]);
    t.roll(who).unwrap();
}

/// Mark the newest field instance with `card` of `who` face-down.
fn lie_face_down(t: &mut Table, who: usize, card: &str) {
    let uid = t.field(who).iter().find(|f| f.card == card).unwrap().uid;
    t.m.world_mut().st.players[who]
        .field
        .iter_mut()
        .find(|f| f.uid == uid)
        .unwrap()
        .face_down = true;
}

// --------------------------------------------------------- clause (1)

/// 规则书（1）: 「每次经过CiRCLE时，若选择抽卡，则可以看抽牌堆顶至多两张卡，
/// 选择其中1张加入手牌，剩余的卡放入弃牌堆或花费500资金将其背面朝上放置在自己
/// 场上。」
///
/// The peek-and-keep **replaces** the draw: one card reaches the hand, not the
/// peeked card *plus* the normal reward draw.
#[test]
fn peek_keep_replaces_the_draw() {
    let mut t = Table::new(&["宇田川亚子", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.set_money(0, 10_000);
    t.set_draw(0, &["通用:GREAT", "通用:安可", "通用:登上武道馆"]);

    pass_circle(&mut t, 0);

    // The reward pick: 0 = money, 1 = draw.
    let p = t.expect_prompt();
    assert!(p.title.key().contains("pass_reward"), "{}", t.dump_prompt());
    t.answer_one(1).unwrap();
    // 「至多两张」: choose to look at both, then keep the first.
    assert!(
        t.prompt_refers("ako_count"),
        "count prompt: {}",
        t.dump_prompt()
    );
    t.answer_one(1).unwrap();
    assert!(t.prompt_refers("ako"), "peek prompt: {}", t.dump_prompt());
    t.answer_one(0).unwrap();
    // The remaining card: decline the 500 face-down placement -> discard.
    assert!(
        t.prompt_refers("ako_place"),
        "place prompt: {}",
        t.dump_prompt()
    );
    t.decline();
    drain(&mut t);

    assert_eq!(
        t.hand(0),
        vec!["通用:GREAT".to_string()],
        "exactly the kept card; the reward draw is replaced, not added"
    );
    assert_eq!(
        t.discard(0),
        vec!["通用:安可".to_string()],
        "the unchosen looked-at card goes to the discard"
    );
    assert_eq!(
        t.draw_pile(0),
        vec!["通用:登上武道馆".to_string()],
        "the rest of the pile is untouched -- no normal reward draw"
    );
}

/// 规则书（1）: 「花费500资金将其背面朝上放置在自己场上」 -- the paid card
/// **moves** from the looked-at pile onto the field (it is not a duplicate copy
/// that stays in the deck), and it lands face-down.
#[test]
fn paying_places_the_card_face_down_and_off_the_deck() {
    let mut t = Table::new(&["宇田川亚子", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.set_money(0, 10_000);
    t.set_draw(0, &["通用:GREAT", "通用:安可", "通用:登上武道馆"]);

    pass_circle(&mut t, 0);

    t.answer_one(1).unwrap(); // draw -- replaced by the peek
    assert!(t.prompt_refers("ako_count"), "{}", t.dump_prompt());
    t.answer_one(1).unwrap(); // look at both
    assert!(t.prompt_refers("ako"), "{}", t.dump_prompt());
    t.answer_one(0).unwrap(); // keep GREAT
    assert!(t.prompt_refers("ako_place"), "{}", t.dump_prompt());
    t.answer_one(0).unwrap(); // pay the 500 and place it face-down
    drain(&mut t);

    let placed = t
        .field(0)
        .into_iter()
        .find(|f| f.card == "通用:安可")
        .expect("the paid card is on the field");
    assert!(placed.face_down, "and face-down: {:?}", t.field_ids(0));
    assert_eq!(t.money(0), 9_500, "500 was paid");
    assert_eq!(
        t.draw_pile(0),
        vec!["通用:登上武道馆".to_string()],
        "the placed copy left the deck (no duplicate): {:?}",
        t.draw_pile(0)
    );
    assert_eq!(t.hand(0), vec!["通用:GREAT".to_string()]);
}

/// 规则书（1）: 「若选择抽卡」 -- the money option does not peek.
#[test]
fn money_reward_does_not_peek() {
    let mut t = Table::new(&["宇田川亚子", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.set_money(0, 10_000);
    t.set_draw(0, &["通用:GREAT", "通用:安可"]);

    pass_circle(&mut t, 0);

    let before = t.money(0);
    t.answer_one(0).unwrap(); // money
    drain(&mut t);

    assert!(!t.prompt_refers("ako"), "no peek on the money option");
    assert_eq!(t.money(0), before + 2000, "the money reward is paid");
    assert!(
        t.hand(0).is_empty(),
        "no card reaches the hand: {:?}",
        t.hand(0)
    );
    assert_eq!(t.draw_pile(0), vec!["通用:GREAT", "通用:安可"]);
}

/// 规则书（1）: 「至多两张」 -- the player may look at only one; the second
/// card is then left on the deck, not even looked at (so not discarded).
#[test]
fn looking_at_one_leaves_the_second_on_the_deck() {
    let mut t = Table::new(&["宇田川亚子", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.set_money(0, 10_000);
    t.set_draw(0, &["通用:GREAT", "通用:安可", "通用:登上武道馆"]);

    pass_circle(&mut t, 0);

    t.answer_one(1).unwrap(); // draw
    assert!(
        t.prompt_refers("ako_count"),
        "count prompt: {}",
        t.dump_prompt()
    );
    t.answer_one(0).unwrap(); // look at 1
    assert!(t.prompt_refers("ako"), "{}", t.dump_prompt());
    t.answer_one(0).unwrap(); // keep GREAT
    drain(&mut t);

    assert_eq!(t.hand(0), vec!["通用:GREAT".to_string()]);
    assert!(t.discard(0).is_empty(), "nothing was looked at twice");
    assert_eq!(
        t.draw_pile(0),
        vec!["通用:安可".to_string(), "通用:登上武道馆".to_string()],
        "the second card stays on top of the deck"
    );
}

// --------------------------------------------------------- clause (2)

/// 规则书（2）: 「经过CiRCLE领取奖励时，若自己场上存在背面朝上的卡，则本次领取
/// 奖励被替换为将自己场上背面朝上的卡拿取至手中。」
///
/// The face-down card is **instead of** the reward -- the chosen reward is not
/// paid on top.
#[test]
fn face_down_card_replaces_the_reward() {
    let mut t = Table::new(&["宇田川亚子", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.set_money(0, 10_000);
    t.set_draw(0, &["通用:安可"]);
    t.place_raw(0, "通用:GREAT");
    lie_face_down(&mut t, 0, "通用:GREAT");

    pass_circle(&mut t, 0);

    let before = t.money(0);
    t.answer_one(0).unwrap(); // money -- replaced by the face-down take
    drain(&mut t);

    assert_eq!(
        t.hand(0),
        vec!["通用:GREAT".to_string()],
        "the face-down card comes to the hand"
    );
    assert!(
        !t.on_field(0, "通用:GREAT"),
        "and leaves the field: {:?}",
        t.field_ids(0)
    );
    assert_eq!(t.money(0), before, "no money -- the reward was replaced");
    assert_eq!(
        t.draw_pile(0),
        vec!["通用:安可".to_string()],
        "no draw either"
    );
}

// --------------------------------------------------------- clause (3)

/// 规则书（3）: 「若自己场上背面朝上放置的卡为[反击]卡，可在符合条件时将其直接
/// 打出。」
///
/// A face-down [反击] card on the field must be offered in its counteraction
/// window, exactly like a face-up field copy.
#[test]
fn face_down_counter_card_is_offered() {
    // Face-up control: the same card, face-up on the payer's field.
    let mut t = Table::new(&["宇田川亚子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, "R:（亚子）黑暗大魔姬亚子");
    let blue = tile("江户川公园");
    t.set_owner(blue, Some(1));
    t.set_pos(0, (blue + 59) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(
        t.counteract_offered("R:（亚子）黑暗大魔姬亚子"),
        "face-up control: {}",
        t.dump_prompt()
    );
    drain(&mut t);

    // Face-down copy.
    let mut t = Table::new(&["宇田川亚子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, "R:（亚子）黑暗大魔姬亚子");
    lie_face_down(&mut t, 0, "R:（亚子）黑暗大魔姬亚子");
    t.set_owner(blue, Some(1));
    t.set_pos(0, (blue + 59) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(
        t.counteract_offered("R:（亚子）黑暗大魔姬亚子"),
        "a face-down [反击] on the field must be offered: {}",
        t.dump_prompt()
    );

    // Playing it runs the card's own body: the [眩晕] is gained, the instance
    // is revealed, and (like any field copy) it stays in place.
    t.counteract(0, "R:（亚子）黑暗大魔姬亚子").unwrap();
    drain(&mut t);
    assert!(
        t.state(0, "stun") + t.state(0, "stunStart") >= 1,
        "the played card's body ran: gained [眩晕]"
    );
    let still = t
        .field(0)
        .into_iter()
        .find(|f| f.card == "R:（亚子）黑暗大魔姬亚子")
        .expect("a played field copy stays in place");
    assert!(!still.face_down, "the played card is revealed");
}

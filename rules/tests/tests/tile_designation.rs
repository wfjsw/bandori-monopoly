//! Black-box tests for lasting tile designations ([指定] a tile / 地契 / 地块
//! while a field card / skill is in play).
//!
//! Spec: `docs/CARDS.md` 「Tile designations」 -- each pick is a mark of
//! `mark_category::DESIGNATE` / `mark_kind::DESIGNATE` owned by the designating
//! instance; `destroy_instance_units` clears it when that instance leaves the
//! field.
//!
//! Naming: `<slug>_<what>`. Each assertion block carries the clause it checks
//! as `// 规则书: 「…」`.

mod common;
use common::*;

use game_core::msg::Arg;
use game_core::state::{mark_category, mark_kind, FieldCard, TileMark};

const FUJIMI: usize = 11; // 富士见坂, price 600
const HILL: usize = 44; // 星之鼓动山丘, price 600

fn skip_all(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// `who`'s first field copy of `card`.
fn field_card(t: &Table, who: usize, card: &str) -> Option<FieldCard> {
    t.field(who).into_iter().find(|f| f.card == card)
}

/// Designation marks on `tile` belonging to `uid`.
fn designates_on(t: &Table, tile: usize, uid: i32) -> Vec<TileMark> {
    t.marks_on(tile)
        .into_iter()
        .filter(|m| m.category == mark_category::DESIGNATE && m.instance == Some(uid))
        .collect()
}

/// The mark row at `uid`, if it is still on the board.
fn mark_by_uid(t: &Table, uid: i32) -> Option<TileMark> {
    t.marks().into_iter().find(|m| m.uid == uid)
}

/// Play `AG:ONE OF US` the standard way (HILL mine / FUJIMI theirs).
fn play_one_of_us(t: &mut Table) -> i32 {
    t.set_character_raw(0, "美竹兰");
    t.set_character_raw(1, "青叶摩卡"); // another Afterglow character
    t.own(0, &[HILL]);
    t.own(1, &[FUJIMI]);
    t.give_play(0, "AG:ONE OF US").unwrap();
    let k = t.option("PlayerId(1)").expect("partner prompt");
    t.answer(0, k).unwrap();
    t.answer_tile(0, HILL).unwrap();
    t.answer_tile(0, FUJIMI).unwrap();
    skip_all(t);
    field_card(t, 0, "AG:ONE OF US")
        .expect("field instance")
        .uid
}

// =====================================================================
// AG:ONE OF US
// =====================================================================

// 规则书: 「将此卡放置在场上…指定双方各一块地契…」 -- the pair of deeds is the
// lasting [指定] while the card is in play. Replaces the old
// `one_of_us_mine` / `one_of_us_theirs` tile slots.
#[test]
fn one_of_us_designation_marks_appear_and_die_with_the_card() {
    // 规则书: 「选择场上的另一个Afterglow角色或者拥有商店街格子的角色，指定双方
    // 各一块地契」
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "美竹兰");
    t.set_character_raw(1, "青叶摩卡"); // another Afterglow character
    t.own(0, &[HILL]);
    t.own(1, &[FUJIMI]);
    t.give_play(0, "AG:ONE OF US").unwrap();
    // Partner: the only candidate. Then one deed each (「指定双方各一块地契」).
    let k = t.option("PlayerId(1)").expect("partner prompt");
    t.answer(0, k).unwrap();
    t.answer_tile(0, HILL).unwrap();
    t.answer_tile(0, FUJIMI).unwrap();
    skip_all(&mut t);
    assert!(t.on_field(0, "AG:ONE OF US"), "field {:?}", t.field_ids(0));
    let uid = field_card(&t, 0, "AG:ONE OF US")
        .expect("field instance")
        .uid;
    assert!(uid > 0, "field uid {uid}");
    // One [指定] mark per deed, owned by the field card's instance.
    let mine = designates_on(&t, HILL, uid);
    assert_eq!(mine.len(), 1, "mine designates: {:?}", t.marks());
    assert_eq!(mine[0].category, mark_category::DESIGNATE);
    assert_eq!(mine[0].kind, mark_kind::DESIGNATE);
    assert_eq!(
        mine[0].instance,
        Some(uid),
        "instance = the field card: {:?}",
        mine[0]
    );
    assert_eq!(
        mine[0].owner, 0,
        "label colour = the designating seat: {:?}",
        mine[0]
    );
    let theirs = designates_on(&t, FUJIMI, uid);
    assert_eq!(theirs.len(), 1, "theirs designates: {:?}", t.marks());
    assert_eq!(theirs[0].instance, Some(uid), "{:?}", theirs[0]);
    // No stray designations anywhere else.
    let all: Vec<i32> = t
        .marks()
        .iter()
        .filter(|m| m.category == mark_category::DESIGNATE)
        .map(|m| m.tile)
        .collect();
    let mut sorted = all.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        vec![FUJIMI as i32, HILL as i32],
        "exactly the pair: {all:?}"
    );
    // 规则书: the designation lasts while the card is in play -- leaving the
    // field destroys the instance's units (`destroy_instance_units`).
    assert!(
        t.m.world_mut().unplace_card(0, "AG:ONE OF US").is_some(),
        "unplace the field card"
    );
    assert!(!t.on_field(0, "AG:ONE OF US"), "field {:?}", t.field_ids(0));
    assert!(
        designates_on(&t, HILL, uid).is_empty(),
        "mine designation died with the card: {:?}",
        t.marks()
    );
    assert!(
        designates_on(&t, FUJIMI, uid).is_empty(),
        "theirs designation died with the card: {:?}",
        t.marks()
    );
}

// 规则书: 「指定双方各一块地契」 -- the two picks are stored as `MarkRef`s on
// card state and retrieve to exactly those two deeds.
#[test]
fn one_of_us_refs_store_and_retrieve_the_pair() {
    let mut t = Table::vanilla(2);
    let uid = play_one_of_us(&mut t);
    assert!(uid > 0, "field uid {uid}");
    // `play` stores the two designation handles as packed `MarkRef`s.
    let mine_ref = t.state(0, "one_of_us_mine");
    let theirs_ref = t.state(0, "one_of_us_theirs");
    assert!(mine_ref > 0, "mine MarkRef packed uid: {mine_ref}");
    assert!(theirs_ref > 0, "theirs MarkRef packed uid: {theirs_ref}");
    assert_ne!(mine_ref, theirs_ref, "distinct handles");
    // Retrieving the refs resolves to exactly the two designated deeds.
    let mine = mark_by_uid(&t, mine_ref).expect("mine mark live");
    let theirs = mark_by_uid(&t, theirs_ref).expect("theirs mark live");
    assert_eq!(mine.tile, HILL as i32, "mine resolves to HILL: {mine:?}");
    assert_eq!(theirs.tile, FUJIMI as i32, "theirs resolves to FUJIMI: {theirs:?}");
    assert_ne!(mine.tile, theirs.tile, "distinct tiles");
    assert_eq!(mine.category, mark_category::DESIGNATE, "{mine:?}");
    assert_eq!(theirs.category, mark_category::DESIGNATE, "{theirs:?}");
    assert_eq!(mine.instance, Some(uid), "{mine:?}");
    assert_eq!(theirs.instance, Some(uid), "{theirs:?}");
    // An **unset** slot reads back 0. Uid 0 is never issued (allocation starts
    // at 1), so unpacking it must yield `MarkRef::NONE`, never a live handle.
    let unset = t.state(0, "no_such_ref");
    assert_eq!(unset, 0, "unset slot reads 0");
    assert!(
        card_sdk::abi::MarkRef::from(unset).is_none(),
        "unset slot unpacks to MarkRef::NONE"
    );
    assert!(
        !card_sdk::abi::MarkRef::from(mine_ref).is_none(),
        "a stored ref unpacks to a live handle"
    );
}

// Pure API: a stored mark handle dies with its creating instance and is never
// recycled onto a later mark (monotonic uid allocation).
#[test]
fn mark_ref_dies_with_the_card_and_does_not_alias() {
    let mut t = Table::vanilla(2);
    let _uid = play_one_of_us(&mut t);
    let mine_ref = t.state(0, "one_of_us_mine");
    let theirs_ref = t.state(0, "one_of_us_theirs");
    assert!(mark_by_uid(&t, mine_ref).is_some(), "mine live before unplace");
    assert!(mark_by_uid(&t, theirs_ref).is_some(), "theirs live before unplace");
    // 规则书: the designation lasts while the card is in play -- leaving the
    // field destroys the instance's units (`destroy_instance_units`).
    assert!(
        t.m.world_mut().unplace_card(0, "AG:ONE OF US").is_some(),
        "unplace the field card"
    );
    assert!(mark_by_uid(&t, mine_ref).is_none(), "mine handle is dead");
    assert!(mark_by_uid(&t, theirs_ref).is_none(), "theirs handle is dead");
    assert_eq!(t.m.world().st.marks.iter().find(|m| m.uid == mine_ref).map(|m| m.tile).unwrap_or(-1), -1,
        "dead handle reports tile -1");
    // A new mark must not recycle the old uids.
    let new_uid = t.m.world_mut().place_mark(
        -1,
        "test:alias",
        "",
        HILL as i32,
        -1,
        -1,
        1,
        game_core::msg::Msg::default(),
        true,
    );
    assert!(new_uid > 0, "new mark uid {new_uid}");
    assert_ne!(new_uid, mine_ref, "new mark did not recycle mine's uid");
    assert_ne!(new_uid, theirs_ref, "new mark did not recycle theirs' uid");
    // The old stored refs still report dead -- the new mark did not alias.
    assert!(mark_by_uid(&t, mine_ref).is_none(), "mine still dead after new mark");
    assert!(mark_by_uid(&t, theirs_ref).is_none(), "theirs still dead after new mark");
    assert!(mark_by_uid(&t, new_uid).is_some(), "the new mark is live");
}

// Pure API: `undesignate` (drop that one designation row) removes only the
// named row; the other designation of the same instance stays on the board.
#[test]
fn undesignate_drops_that_one_row() {
    let mut t = Table::vanilla(2);
    let uid = play_one_of_us(&mut t);
    let mine_ref = t.state(0, "one_of_us_mine");
    let theirs_ref = t.state(0, "one_of_us_theirs");
    assert_eq!(designates_on(&t, HILL, uid).len(), 1);
    assert_eq!(designates_on(&t, FUJIMI, uid).len(), 1);
    // `ctx::undesignate(r)` is `remove_mark(r)` on that exact row.
    assert!(
        t.m.world_mut().remove_mark_by_uid(mine_ref),
        "undesignate(mine) removes that row"
    );
    // Only that one row: the other ref stays live and its mark is still on the board.
    assert!(mark_by_uid(&t, mine_ref).is_none(), "mine row gone");
    assert!(
        mark_by_uid(&t, theirs_ref).is_some(),
        "theirs row still live: {:?}",
        t.marks()
    );
    assert!(designates_on(&t, HILL, uid).is_empty(), "HILL designation gone");
    assert_eq!(designates_on(&t, FUJIMI, uid).len(), 1, "FUJIMI designation stays");
    // Dropping the other row is also exact.
    assert!(t.m.world_mut().remove_mark_by_uid(theirs_ref));
    assert!(mark_by_uid(&t, theirs_ref).is_none());
    assert!(t.marks().iter().all(|m| m.category != mark_category::DESIGNATE));
}

// =====================================================================
// MyGO:无路矢
// =====================================================================

// 规则书: 「指定场上自己以外的一位玩家所在格子，获得2层[除外]并在[除外]层数归0后
// [传送]至该格子」 -- the exile return stays engine-side; the designation mark is
// the displayed record of the pick while the field stand-in is in play.
#[test]
fn no_road_designation_mark_appears_and_dies_with_the_card() {
    let mut t = Table::vanilla(3);
    t.set_pos(1, 10);
    t.give_play(0, "MyGO:无路矢").unwrap();
    let k = t.option("PlayerId(1)").expect("player-choice prompt");
    t.answer(0, k).unwrap();
    assert_eq!(t.state(0, "exile"), 2, "two [除外] layers");
    assert_eq!(t.state(0, "exileTo"), 10, "the engine-owned return tile");
    assert!(
        t.on_field(0, "MyGO:无路矢"),
        "field stand-in: {:?}",
        t.field_ids(0)
    );
    let uid = field_card(&t, 0, "MyGO:无路矢")
        .expect("field instance")
        .uid;
    // The [指定] mark is the displayed record alongside `exileTo`.
    let marks = designates_on(&t, 10, uid);
    assert_eq!(marks.len(), 1, "designates P1's tile: {:?}", t.marks());
    assert_eq!(marks[0].category, mark_category::DESIGNATE);
    assert_eq!(marks[0].instance, Some(uid), "{:?}", marks[0]);
    assert_eq!(marks[0].owner, 0, "{:?}", marks[0]);
    // Leaving the field destroys the instance's units.
    assert!(
        t.m.world_mut().unplace_card(0, "MyGO:无路矢").is_some(),
        "unplace the field card"
    );
    assert!(
        designates_on(&t, 10, uid).is_empty(),
        "designation died with the card: {:?}",
        t.marks()
    );
}
// =====================================================================
// Placement logs never speak a raw mark key
// =====================================================================

/// `log.outcome.mark_*` / `log.outcome.designate` must not print a wire kind
/// (`mark:*` / `cards:…`) as plain text. Designations use the readable
/// `log.outcome.designate` line; every other kind rides a nested `Msg` the
/// view resolves (or a display name like 「育美标记」).
#[test]
fn placement_logs_never_speak_a_raw_mark_key() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "美竹兰");
    t.set_character_raw(1, "青叶摩卡");
    t.own(0, &[HILL]);
    t.own(1, &[FUJIMI]);
    t.give_play(0, "AG:ONE OF US").unwrap();
    let k = t.option("PlayerId(1)").expect("partner prompt");
    t.answer(0, k).unwrap();
    t.answer_tile(0, HILL).unwrap();
    t.answer_tile(0, FUJIMI).unwrap();
    skip_all(&mut t);

    let mut saw_designate = false;
    for e in t.events_since(0) {
        let key = e.msg.key().to_string();
        if !key.starts_with("log.outcome.mark") && key != "log.outcome.designate" {
            continue;
        }
        // Designation placement is the readable line, never mark_add + kind.
        if key == "log.outcome.designate" {
            saw_designate = true;
        }
        if key == "log.outcome.mark_add" {
            let kind = e.msg.a.get("kind");
            let raw = match kind {
                Some(Arg::Text(s)) => s.clone(),
                Some(Arg::Msg(m)) => m.key().to_string(),
                _ => String::new(),
            };
            assert!(
                !raw.starts_with("mark:"),
                "mark_add must not carry a raw mark:* key: {raw:?} ({:?})",
                e.msg
            );
        }
        // No plain-text argument may be a raw namespace key.
        for (name, arg) in e.msg.a.iter() {
            if let Arg::Text(s) = arg {
                assert!(
                    !(s.starts_with("mark:") || s.starts_with("cards:")),
                    "raw key in text arg {name}={s:?} of {}",
                    key
                );
            }
        }
    }
    assert!(
        saw_designate,
        "the designation placement speaks log.outcome.designate: {:?}",
        t.recent_keys(30)
    );
}

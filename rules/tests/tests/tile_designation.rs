//! Black-box tests for lasting tile designations ([指定] a tile / 地契 / 地块
//! while a field card / skill is in play).
//!
//! Spec: `docs/CARDS.md` 「Tile designations」 -- each pick is a mark of
//! `mark_category::DESIGNATE` / `mark_kind::DESIGNATE` owned by the designating
//! instance; `destroy_instance_units` clears it when that instance leaves the
//! field. ONE OF US parks its pair as attached deed rows instead (G08 §8.2,
//! 「放置在该卡上」), so its records are the attachments.
//!
//! Naming: `<slug>_<what>`. Each assertion block carries the clause it checks
//! as `// 规则书: 「…」`.

mod common;
use common::*;

use game_core::msg::Arg;
use game_core::state::{mark_category, FieldCard, TileMark};

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

/// A per-`instance` property of `who`'s first field copy of `card` (unset = 0).
/// The card's own record must ride the instance: a second copy of the same card
/// on one field keeps its own pair.
fn field_prop(t: &Table, who: usize, card: &str, key: &str) -> i32 {
    field_card(t, who, card)
        .and_then(|f| f.props.get(key).copied())
        .unwrap_or(0)
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

/// Remove a host the way the zone writer's leave does: apply the instance
/// edges (the host-left cascade) first, then drop the row itself.
fn unplace_host(t: &mut Table, uid: i32) {
    let edges = t.m.world_mut().drop_instance_edges(uid);
    for edge in &edges {
        t.m.world_mut().apply_zone_row(edge);
    }
    assert!(t.m.world_mut().unplace_at(uid) >= 0, "unplace the host");
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
// lasting [指定] while the card is in play, parked on the card as attached deed
// rows (「放置在该卡上」). The `one_of_us_mine` / `one_of_us_theirs` handles
// ride the instance's props, not per-player state.
#[test]
fn one_of_us_designated_deeds_appear_and_die_with_the_card() {
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
    // Both designated deeds are parked on the card, each recording its tile.
    let parked = t.m.world().attached_cards(uid);
    assert_eq!(parked.len(), 2, "the pair is parked on the card");
    let mut tiles: Vec<i32> = parked
        .iter()
        .map(|&u| t.m.world().prop_at(u, "one_of_us_tile") - 1)
        .collect();
    tiles.sort_unstable();
    assert_eq!(tiles, vec![FUJIMI as i32, HILL as i32], "exactly the pair");
    assert!(
        parked.iter().all(|&u| t.m.world().holder_of(u) == 0),
        "the parked deeds are the player's"
    );
    // 规则书: the designation lasts while the card is in play -- leaving the
    // field takes the attachments with it (the host-left cascade).
    unplace_host(&mut t, uid);
    assert!(!t.on_field(0, "AG:ONE OF US"), "field {:?}", t.field_ids(0));
    assert!(
        t.m.world().attached_cards(uid).is_empty(),
        "the pair died with the card"
    );
    assert!(
        parked
            .iter()
            .all(|&u| t.m.world().field_by_uid(u).is_none()),
        "the parked rows are dead"
    );
}

// 规则书: 「指定双方各一块地契」 -- the two parked deeds are stored as uids
// (`+1`) on the card instance's props and retrieve to exactly those two rows.
#[test]
fn one_of_us_props_store_and_retrieve_the_pair() {
    let mut t = Table::vanilla(2);
    let uid = play_one_of_us(&mut t);
    assert!(uid > 0, "field uid {uid}");
    // `play` stores the two parked uids on the placed instance, `+1` so the
    // first uid survives an unset read.
    let mine_ref = field_prop(&t, 0, "AG:ONE OF US", "one_of_us_mine");
    let theirs_ref = field_prop(&t, 0, "AG:ONE OF US", "one_of_us_theirs");
    assert!(mine_ref > 0, "mine stored uid: {mine_ref}");
    assert!(theirs_ref > 0, "theirs stored uid: {theirs_ref}");
    assert_ne!(mine_ref, theirs_ref, "distinct handles");
    // Retrieving the refs resolves to exactly the two designated deeds.
    let mine = mine_ref - 1;
    let theirs = theirs_ref - 1;
    assert_eq!(
        t.m.world().attachment_host(mine),
        uid,
        "mine is parked on the card"
    );
    assert_eq!(t.m.world().attachment_host(theirs), uid, "theirs too");
    let mine_tile = t.m.world().prop_at(mine, "one_of_us_tile") - 1;
    let theirs_tile = t.m.world().prop_at(theirs, "one_of_us_tile") - 1;
    assert_eq!(mine_tile, HILL as i32, "mine resolves to HILL");
    assert_eq!(theirs_tile, FUJIMI as i32, "theirs resolves to FUJIMI");
    assert_ne!(mine_tile, theirs_tile, "distinct tiles");
    // An **unset** prop reads back 0. Uid 0 is never issued (allocation starts
    // at 1), so unpacking it must yield `MarkRef::NONE`, never a live handle.
    let unset = field_prop(&t, 0, "AG:ONE OF US", "no_such_ref");
    assert_eq!(unset, 0, "unset prop reads 0");
    assert!(
        card_sdk::abi::MarkRef::from(unset).is_none(),
        "unset prop unpacks to MarkRef::NONE"
    );
    assert!(
        !card_sdk::abi::MarkRef::from(mine_ref).is_none(),
        "a stored ref unpacks to a live handle"
    );
}

// Pure API: a stored deed uid dies with its creating instance and is never
// recycled onto a later row (monotonic uid allocation).
#[test]
fn parked_deed_dies_with_the_card_and_does_not_alias() {
    let mut t = Table::vanilla(2);
    let uid = play_one_of_us(&mut t);
    let mine = field_prop(&t, 0, "AG:ONE OF US", "one_of_us_mine") - 1;
    let theirs = field_prop(&t, 0, "AG:ONE OF US", "one_of_us_theirs") - 1;
    assert!(t.m.world().field_by_uid(mine).is_some(), "mine live");
    assert!(t.m.world().field_by_uid(theirs).is_some(), "theirs live");
    // 规则书: the designation lasts while the card is in play -- leaving the
    // field takes the parked rows with it.
    unplace_host(&mut t, uid);
    assert!(t.m.world().field_by_uid(mine).is_none(), "mine is dead");
    assert!(t.m.world().field_by_uid(theirs).is_none(), "theirs is dead");
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
    assert_ne!(new_uid, mine, "new mark did not recycle mine's uid");
    assert_ne!(new_uid, theirs, "new mark did not recycle theirs' uid");
    assert!(
        t.m.world().field_by_uid(mine).is_none(),
        "mine still dead after new mark"
    );
    assert!(
        t.m.world().field_by_uid(theirs).is_none(),
        "theirs still dead after new mark"
    );
    assert!(mark_by_uid(&t, new_uid).is_some(), "the new mark is live");
}

// Pure API: dropping one parked deed removes only that row; the other deed of
// the same instance stays on the board.
#[test]
fn dropping_one_parked_deed_leaves_the_other() {
    let mut t = Table::vanilla(2);
    let uid = play_one_of_us(&mut t);
    let mine = field_prop(&t, 0, "AG:ONE OF US", "one_of_us_mine") - 1;
    let theirs = field_prop(&t, 0, "AG:ONE OF US", "one_of_us_theirs") - 1;
    assert_eq!(t.m.world().attached_cards(uid).len(), 2);
    assert!(t.m.world_mut().unplace_at(mine) >= 0, "drop the mine row");
    assert!(t.m.world().field_by_uid(mine).is_none(), "mine row gone");
    assert!(
        t.m.world().field_by_uid(theirs).is_some(),
        "theirs row still live"
    );
    assert_eq!(t.m.world().attached_cards(uid), vec![theirs], "only theirs");
    // Dropping the other row is also exact.
    assert!(t.m.world_mut().unplace_at(theirs) >= 0);
    assert!(t.m.world().attached_cards(uid).is_empty());
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
    // 无路矢 places a [指定] mark (ONE OF US parks attachments instead).
    let mut t = Table::vanilla(3);
    t.set_pos(1, 10);
    t.give_play(0, "MyGO:无路矢").unwrap();
    let k = t.option("PlayerId(1)").expect("player-choice prompt");
    t.answer(0, k).unwrap();

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
            let kind = e.msg.args.get("kind");
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
        for (name, arg) in e.msg.args.iter() {
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

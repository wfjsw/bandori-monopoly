//! A hook that runs on **every step** of a walk is bookkeeping: it flashes no
//! card and writes no line (the state it moves is the trace). An entry a
//! condition or guard has already narrowed past "every tile" keeps announcing
//! (`docs/CARDS.md` "Per-step hooks").
//!
//! * 濑田薰 (1) passes any tile: no header, no gain line, the pot still lands;
//! * 凑友希那 (3) drops every pot off RiNG: no line, the pots still go;
//! * PPP:Tomorrow's Door hops per tile: no line, the card still advances;
//! * a narrowed entry (PPP:（香澄）大家我都喜欢哦 -- its own tile) still
//!   announces.

mod common;
use common::*;

use game_core::state::MatchEvent;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

fn card_events(t: &Table, mark: i32, card: &str) -> Vec<MatchEvent> {
    t.events_since(mark)
        .into_iter()
        .filter(|e| e.r#type == "card" && e.card == card)
        .collect()
}

/// 濑田薰 (1): one pot per pass, silently. 羽丘女子学院 (11) / 富士见坂 (12) do
/// not gain on their own, so any `gain` here is the skill's.
#[test]
fn kaoru_per_pass_pot_is_silent() {
    let mut t = Table::new(&["濑田薰", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.set_pos(0, 10);
    t.set_fire(0, 0, 7);
    let mark = t.mark();
    t.dice(&[2]);
    t.roll(0).unwrap();
    drain(&mut t);
    let skill = "skill:濑田薰:梦幻的王子殿下";
    assert!(
        card_events(&t, mark, skill).is_empty(),
        "no header per step: {:?}",
        t.recent_keys(20)
    );
    assert!(
        !t.keys_since(mark).iter().any(|k| k.contains("kaoru_gain")),
        "no pot line per step: {:?}",
        t.recent_keys(20)
    );
    assert_eq!(t.charge(0), 2, "both passes still gained their pot");
}

/// 凑友希那 (3): every pot is lost on the first non-RiNG step, with no line.
#[test]
fn yukina_pot_drop_is_silent() {
    let mut t = Table::new(&["凑友希那", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    // 富士见坂 (11) is a plain tile: the walk leaves CiRCLE at once.
    t.set_pos(0, 11);
    t.set_fire(0, 2, 3);
    let mark = t.mark();
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    let skill = "skill:凑友希那:来练习吧";
    assert!(
        card_events(&t, mark, skill).is_empty(),
        "no header per step: {:?}",
        t.recent_keys(20)
    );
    assert!(
        !t.keys_since(mark).iter().any(|k| k.contains("yukina_lost")),
        "no pot-loss line per step: {:?}",
        t.recent_keys(20)
    );
    assert_eq!(t.charge(0), 0, "the pots are still gone");
}

/// PPP:Tomorrow's Door (2): the card advances a stop per pass, with no line.
#[test]
fn tomorrows_door_hop_is_silent() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "户山香澄");
    t.begin_turn(0);
    t.set_pos(0, 40);
    t.give_play(0, "PPP:Tomorrow's Door").unwrap();
    drain(&mut t);
    let mark = t.mark();
    t.dice(&[8]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert!(
        !t.keys_since(mark).iter().any(|k| k.contains("tomorrows_door")),
        "no hop line: {:?}",
        t.recent_keys(20)
    );
    let field = t.field(0);
    let f = field
        .iter()
        .find(|f| f.card.contains("Tomorrow's Door"))
        .expect("door still on the field");
    assert_eq!(
        f.tile,
        tile("花咲川女子学院") as i32,
        "the door still hops"
    );
}

/// A per-step entry narrowed to its own tile still announces.
#[test]
fn narrowed_per_step_entry_still_announces() {
    let mut t = Table::vanilla(2);
    t.begin_turn(0);
    // P0 plays the card: it parks itself on 星之鼓动山丘.
    t.give_play(0, "PPP:（香澄）大家我都喜欢哦").unwrap();
    drain(&mut t);
    let parked = t
        .field(0)
        .iter()
        .find(|f| f.card.contains("大家我都喜欢"))
        .and_then(|f| (f.tile >= 0).then_some(f.tile as usize))
        .expect("the card parked on its tile");
    // P1 walks over that tile.
    t.set_pos(1, (parked + 59) % 60);
    t.begin_turn(1);
    let mark = t.mark();
    t.dice(&[2]);
    t.roll(1).unwrap();
    drain(&mut t);
    let events = card_events(&t, mark, "PPP:（香澄）大家我都喜欢哦");
    assert!(
        !events.is_empty(),
        "a narrowed entry announces: {:?}",
        t.recent_keys(30)
    );
}

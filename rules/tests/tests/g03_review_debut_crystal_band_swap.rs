//! G03 review regression: why does debut's turn-end crystal not land?

mod common;
use common::*;

const DEBUT: &str = "CRYCHIC:初演大成功";

#[test]
fn zz_probe_debut_crystal() {
    let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
    t.clean();
    t.begin_turn(0);
    while t.prompt().is_some() {
        t.decline();
    }
    let band_id = t.skill_id(0, "CRYCHIC");
    eprintln!("band_id {band_id:?} field {:?}", t.field(0));
    t.give(0, &[DEBUT]);
    t.play(0, DEBUT).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    t.dice(&[3]);
    let r = t.roll(0);
    eprintln!("roll: {r:?}");
    while t.prompt().is_some() {
        t.decline();
    }
    t.end(0).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    eprintln!("after: field {:?} crystals {:?}", t.field(0), Some(t.crystals(0, &band_id)));
    for e in t.events_since(0) {
        eprintln!("{} {:?}", e.r#type, e.msg);
    }
}

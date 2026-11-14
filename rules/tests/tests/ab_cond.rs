//! COND-01: a condition on a later entry of one hook window must see the
//! state write an earlier entry's body just made.
//!
//! The probe `TEST:condLiveProbe` has two `rollPlan` entries: the first sets
//! `slot('cond.live')`, the second is gated on it and logs. Before the fix the
//! condition was admitted against the pre-window world snapshot, so the second
//! entry never ran; the body's own `state::get` would have seen the write.

mod common;
use common::*;

fn drain(t: &mut Table) {
    for _ in 0..40 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
    panic!("drain: prompts never stopped: {}", t.dump_prompt());
}

#[test]
fn a_slot_condition_sees_an_earlier_entry_write_in_the_same_window() {
    let space = tile("Space");
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.clean();
    t.place_raw(0, "TEST:condLiveProbe");
    t.set_pos(0, space - 1);
    t.begin_turn(0);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    let keys = t.recent_keys(64);
    let bump = keys
        .iter()
        .position(|k| k.contains("cond_live_bump"))
        .unwrap_or_else(|| panic!("the bump entry did not run: {keys:?}"));
    let see = keys
        .iter()
        .position(|k| k.contains("cond_live_see"))
        .unwrap_or_else(|| {
            panic!("the gated entry read the stale snapshot: {keys:?}")
        });
    assert!(bump < see, "the gated entry runs after the write: {keys:?}");
}

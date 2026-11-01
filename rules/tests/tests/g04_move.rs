//! G04 cross-command flows that no single card owns
//! (`docs/rulebook/spec/abi/G04-move-plan.md` §8.2 preamble):
//! Play-time staging vs a side move in the same activation, a replacement's
//! body resuming after the settle, and a window write on a `moveBefore`
//! counter.

mod common;
use common::*;

fn drain(t: &mut Table) {
    for _ in 0..64 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
    panic!("drain: prompts never stopped: {}", t.dump_prompt());
}

// A Play body stages a teleport on the plan and runs it as a **side** move in
// the same activation. The staging belongs to the command: the main move
// afterwards walks its own dice, from the teleport's destination, and is not a
// second teleport.
#[test]
fn g04_play_staged_teleport_then_side_move() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 10);
    t.set_pos(1, 30);
    t.m.world_mut().turn.plan.forced = true; // a side move
    t.give_play(0, "TEST:tele_nosettle").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 15, "the Play-time staged teleport ran 5 ahead");
    assert!(!t.m.world().turn.main_moved, "it was a side move");
    // The next main move is a plain walk from the destination.
    t.dice(&[4]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(0),
        19,
        "the main move walked 4 from 15, not a teleport"
    );
    assert_eq!(t.m.world().st.plan.steps, 4);
}

// 规则书（美咲）: the teleport 「并触发结算」 resolves before the card's
// 「随后…支付500」 clause -- the replacement's body resumes after the settle.
#[test]
fn g04_replacement_body_resumes_after_the_settle() {
    let mut t = Table::vanilla(3);
    t.set_pos(0, 10);
    t.set_pos(1, 14);
    t.set_pos(2, 20);
    t.own(2, &[14]);
    let rent = data().tiles[14].rent[0];
    t.set_fire(0, 1, 1);
    t.give(0, &["HHW:（美咲）", "TEST:fireRoll"]);
    t.play(0, "TEST:fireRoll").unwrap();
    drain(&mut t);
    let mark = t.mark();
    t.dice(&[4]);
    t.roll(0).unwrap();
    assert!(t.counteract_offered("HHW:（美咲）"), "{}", t.dump_prompt());
    t.counteract(0, "HHW:（美咲）").unwrap();
    let k = t
        .option("PlayerId(1)")
        .unwrap_or_else(|| panic!("P1 not offered: {}", t.dump_prompt()));
    t.answer(0, k).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 14, "teleported onto P1");
    // The settle finished first (rent to the owner), then the body paid the
    // destination neighbours 500.
    assert_eq!(t.money(0), 10_000 - rent + 500);
    assert_eq!(t.money(1), 9_500);
    assert_eq!(t.money(2), 10_000 + rent);
    let keys = t.keys_since(mark);
    let rent_at = keys
        .iter()
        .position(|k| k == "log.rent")
        .unwrap_or_else(|| panic!("no settle rent: {keys:?}"));
    let body_at = keys
        .iter()
        .position(|k| k.contains("misaki_card_moved"))
        .unwrap_or_else(|| panic!("the body never resumed: {keys:?}"));
    assert!(
        rent_at < body_at,
        "settle precedes the body's pay: {keys:?}"
    );
}

// A counter on the `moveBefore` window writes the plan (`set_steps`) while the
// walk is already resolved; the walk pulls the write in.
const SAYO: &str = "skill:冰川纱夜:踏上荆棘之路的觉悟";

#[test]
fn g04_move_before_window_write_extends_the_walk() {
    let mut t = Table::new(&["冰川纱夜", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.set_fire(0, 10, 10);
    t.set_pos(0, 1);
    t.set_pos(1, 30);
    t.dice(&[5]);
    t.roll(0).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.text.key(), "ask.counteract.move_extension");
    assert_eq!(t.pos(0), 1, "the window opens before the first step");
    t.answer(0, 0).unwrap(); // 2 tiles
    t.counteract(0, SAYO).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 8, "5 rolled + 2 written in moveBefore");
    assert_eq!(t.charge(0), 4, "the 6-pot cost was paid");
}

//! G03 review regression: `ctx::rolls_by_player`'s fixed 64-byte
//! postcard buffer: a 64-entry history encodes to 65 bytes, the host writes
//! nothing, and the wrapper returns an empty Vec.

mod common;
use common::*;

const TAKI: &str = "CRYCHIC:（立希）即便比不上...";
const KARAOKE: &str = "CRYCHIC:去唱卡拉ok吧";

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// Table with `prior` casts already in this turn, at player 0's open main-move
/// [反击] window, Taki in hand, constant dice 3. One prior cast is a real
/// karaoke cast (to have an entry to clone); the rest are clones of it.
fn setup(prior: usize) -> Table {
    let mut t = Table::new(&["椎名立希（CRYCHIC）", "高松灯（CRYCHIC）"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.give(0, &[TAKI, KARAOKE]);
    t.set_pos(0, 0);
    t.dice(&[3; 256]);
    t.play(0, KARAOKE).unwrap();
    drain(&mut t); // one karaoke cast in the history now
    if prior > 1 {
        let hist = &mut t.m.world_mut().turn.roll_history;
        let proto = *hist.last().unwrap();
        for _ in 0..prior - 1 {
            hist.push(proto);
        }
    }
    assert_eq!(t.m.world().turn.roll_history.len(), prior);
    t.roll(0).unwrap();
    assert!(t.counteract_offered(TAKI), "{}", t.dump_prompt());
    assert_eq!(t.m.world().turn.roll_history.len(), prior + 1);
    println!(
        "PROBE prior={prior} values={:?}",
        t.m.world()
            .turn
            .roll_history
            .iter()
            .map(|e| e.value)
            .collect::<Vec<_>>()
            .last()
            .copied()
    );
    t
}

#[test]
fn zz_probe_cap_63_still_reads() {
    // 61 before the main cast, 62 at the body top, 63 after the recast
    // -> 1+63 = 64 bytes: fits.
    let mut t = setup(61);
    t.counteract(0, TAKI).unwrap();
    println!("PROBE 63-entry dump: {}", t.dump_prompt());
    assert!(
        t.dump_prompt().contains("taki_again"),
        "63 entries decode, the repeat must prompt: {}",
        t.dump_prompt()
    );
}

#[test]
fn zz_probe_cap_64_goes_empty() {
    // 62 before the main cast, 63 at the body top, 64 after the recast
    // -> 1+64 = 65 bytes > 64: nothing is written, the wrapper answers [],
    // and the repeat is invisible.
    let mut t = setup(62);
    t.counteract(0, TAKI).unwrap();
    assert!(
        !t.dump_prompt().contains("taki_again"),
        "64 entries vanish, the repeat must not prompt: {}",
        t.dump_prompt()
    );
}

#[test]
fn zz_probe_cap_truncates_the_constant_table_loop() {
    // Constant dice: every face repeats, so the loop's only stop is its own
    // 100-reroll guard -- but the 64-byte cap kills it at ~64 casts.
    let mut t = setup(1);
    t.counteract(0, TAKI).unwrap();
    let mut prompts = 0;
    while t.prompt().is_some() && t.dump_prompt().contains("taki_again") {
        prompts += 1;
        assert!(prompts < 150, "ran away past the guard");
        t.answer(0, 0).unwrap();
    }
    println!(
        "PROBE constant table: {prompts} reroll asks, {} history entries",
        t.m.world().turn.roll_history.len()
    );
    assert!(
        prompts < 100,
        "the 100 guard should allow more than the cap: {prompts}"
    );
    drain(&mut t);
}

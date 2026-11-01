//! G03 review regression: model `roll_pick` records `unused` only on keep-old
//! and takes it from the previous chosen cast, unlike production.

use rules_kani::host::{arm_roll_move, arm_roll_pick, arm_roll_recast, arm_roll_unused_face};
use rules_kani::world;

fn fresh() -> (i32, Vec<i32>) {
    world::reset_world(world::World::new());
    let id = world::with_world(|w| w.next_roll_id);
    // A roll with two casts: the recast's cast (index 1) is the chosen one.
    arm_roll_move([0, 0, 0, 0, 0, 0]);
    arm_roll_recast([id, 0, 0, 0, 0, 0]);
    let faces = world::with_world(|w| w.rolls[0].casts.iter().map(|c| c.value).collect());
    (id, faces)
}

#[test]
fn model_keep_new_records_nothing() {
    let (id, faces) = fresh();
    // Production: `kill the n-1 pick` re-keeps the cast the recast chose
    // (index 1) and still records the old face (cast 0).
    arm_roll_pick([id, 1, 0, 0, 0, 0]);
    assert_eq!(
        world::with_world(|w| w.rolls[0].chosen),
        1,
        "the recast's cast stays chosen"
    );
    assert_eq!(
        arm_roll_unused_face([id, 0, 0, 0, 0, 0]) as i32,
        -1,
        "model left unused=-1 on keep-new (production records the old face {})",
        faces[0]
    );
}

#[test]
fn model_keep_old_records_the_chosen_cast() {
    let (id, faces) = fresh();
    arm_roll_pick([id, 0, 0, 0, 0, 0]);
    assert_eq!(
        arm_roll_unused_face([id, 0, 0, 0, 0, 0]) as i32,
        faces[1],
        "keep-old records the recast's face (matches production)"
    );
}

#[test]
fn model_source_raise_compares_codes_not_ranks() {
    let (id, _) = fresh();
    // Production `cast` uses `RollSource::rank()`: code 1 (charge, rank 3)
    // outranks code 2 (card, rank 1), so source stays 1.
    arm_roll_recast([id, 1, 0, 0, 0, 0]);
    arm_roll_recast([id, 2, 0, 0, 0, 0]);
    let (src, cast_src) = world::with_world(|w| (w.rolls[0].source, w.rolls[0].casts[3].source));
    assert_eq!(
        (src, cast_src),
        (1, 1),
        "model kept the numerically larger card code (production keeps charge 1 by rank)"
    );
}

#[test]
fn model_second_pick_uses_the_old_choice_not_the_latest_pair() {
    // I5: `unused_face` is the other of the *latest pair* on every pick.
    // Production with n=3, pick 0 then pick 1 records casts[2]; the model
    // records the previous chosen cast (casts[0]).
    let (id, _) = fresh();
    arm_roll_recast([id, 0, 0, 0, 0, 0]); // now n = 3
    let faces = world::with_world(|w| {
        w.rolls[0]
            .casts
            .iter()
            .map(|c| c.value)
            .collect::<Vec<i32>>()
    });
    arm_roll_pick([id, 0, 0, 0, 0, 0]);
    arm_roll_pick([id, 1, 0, 0, 0, 0]);
    let unused = arm_roll_unused_face([id, 0, 0, 0, 0, 0]) as i32;
    assert_eq!(
        unused, faces[0],
        "model records the old choice ({}) where production records the pair's other {}",
        faces[0], faces[2]
    );
}

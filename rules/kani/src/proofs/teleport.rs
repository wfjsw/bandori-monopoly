//! Harness (b): status/teleport-style card body.
//!
//! `PPP:（多惠）寻找更好的声音` (roster):
//! > 「传送到"江户川乐器店"并获得一个[火罐]」
//!
//! Effect: teleport to the named tile, +1 [火罐]. Frame: exactly those two.

use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, TILE_SHOP, MAX_PLAYERS};

fn symbolic_play_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume((2..=4).contains(&w.n_players));
    for p in 0..MAX_PLAYERS {
        w.pos[p] = kani::any();
        kani::assume((0..8).contains(&w.pos[p]));
        w.charge[p] = kani::any();
        // The book gives +1 [火罐] "获得一个[火罐]"; the global hold cap
        // (rulebook-doc.md:240, host charge_max) clamps a gain at the cap, so
        // the clause's exact effect is only defined below the cap. At-cap
        // behaviour is the cap rule, proven separately.
        kani::assume((0..w.charge_max[p]).contains(&w.charge[p]));
        w.out[p] = false;
    }
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    w.trig_player = player;
    w.tile_shop_present = true;
    w
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]
#[kani::unwind(80)]
pub fn tae_sound_teleport_and_fire_iff() {
    let w = symbolic_play_world();
    let player = w.trig_player;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(invoke::tae_sound_play(player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    kani::cover!(pre.charge[player as usize] == 0, "pre fire 0");
    kani::cover!(pre.pos[player as usize] != TILE_SHOP, "pre not on shop");

    crate::kassert!(
        spec::tae_sound_effects_ok(effects, player, TILE_SHOP),
        "teleport+charge: effect log must be exactly Teleport(shop), GainFire(1)"
    );
    crate::kassert!(
        spec::tae_sound_post_ok(&post, player, TILE_SHOP, pre.charge[player as usize]),
        "teleport+charge: post must be on shop with fire+1"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(post.stay == pre.stay, "frame: stay");
    for p in 0..MAX_PLAYERS {
        if p != player as usize {
            crate::kassert!(post.pos[p] == pre.pos[p], "frame: other players unmoved");
            crate::kassert!(post.charge[p] == pre.charge[p], "frame: other players' fire");
        }
    }
}

/// MUTANT: reference demands an extra +1 fire. Card must FAIL to match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]
#[kani::unwind(80)]
#[kani::should_panic]
pub fn tae_sound_mutant_extra_fire() {
    let w = symbolic_play_world();
    let player = w.trig_player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::tae_sound_play(player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    crate::kassert!(
        spec::tae_sound_effects_ok_mutant(&logbuf[..log_n], player, TILE_SHOP),
        "mutant: card must not match the extra-fire spec"
    );
}

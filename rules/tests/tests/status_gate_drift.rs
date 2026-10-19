//! The ruleset's CEL-only `On::Gate` conditions are the **whole** source of
//! truth for the capability gates. There is no engine-side mirror. This test
//! pins that `rules.gate_open` agrees with a local pure function that
//! duplicates the `status:*` hosts' CEL logic on every layer combination.
//! (`docs/STATUS-HOSTS.md`.)

mod common;

use common::*;
use game_core::engine::{CardRules, Gate};

/// Duplicate of the `status:*` hosts' CEL gate conditions (`rules/status`):
///
/// * stay holds Move: `state(actor,'stay') > 0 && state(actor,'unstoppable') == 0`
/// * stunned holds Act/Pay/Move: `state(actor,'stun') + state(actor,'stunStart') > 0`
/// * exiled holds Act/Pay/Move/OnBoard/Targetable: `state(actor,'exile') > 0`
///
/// A gate is closed when **any** holding condition is true. The ruleset
/// declares every gate below, so `gate_open` answers `Some`.
fn want(
    stay: i32,
    stun: i32,
    stun_start: i32,
    exile: i32,
    unstoppable: i32,
    gate: Gate,
) -> Option<bool> {
    let stay_holds_move = stay > 0 && unstoppable == 0;
    let stunned = stun + stun_start > 0;
    let exiled = exile > 0;
    let closed = match gate {
        Gate::Act => stunned || exiled,
        Gate::Pay => stunned || exiled,
        Gate::Move => stay_holds_move || stunned || exiled,
        Gate::OnBoard => exiled,
        Gate::Targetable => exiled,
        // Exclusive / Play ride different declarations; not pinned here.
        _ => false,
    };
    Some(!closed)
}

#[test]
fn cel_gates_are_the_whole_answer() {
    let rules = rules();
    let mut t = Table::vanilla(2);
    use bandori_std::state_key::{EXILE, STAY, STUN, STUN_START, UNSTOPPABLE};
    for stay in 0..3 {
        for stun in 0..2 {
            for stun_start in 0..2 {
                for exile in 0..2 {
                    for unstoppable in 0..2 {
                        t.set_state(0, STAY, stay);
                        t.set_state(0, STUN, stun);
                        t.set_state(0, STUN_START, stun_start);
                        t.set_state(0, EXILE, exile);
                        t.set_state(0, UNSTOPPABLE, unstoppable);
                        let w = t.m.world();
                        for gate in [
                            Gate::Act,
                            Gate::Pay,
                            Gate::Move,
                            Gate::OnBoard,
                            Gate::Targetable,
                        ] {
                            let got = rules.gate_open(w, gate, 0);
                            let expect = want(stay, stun, stun_start, exile, unstoppable, gate);
                            assert_eq!(
                                got, expect,
                                "{gate:?} with stay={stay} stun={stun} stunStart={stun_start} \
                                 exile={exile} unstoppable={unstoppable}"
                            );
                        }
                    }
                }
            }
        }
    }
}
//! Host-side pilot check (no Kani needed): the three migrated guards are in
//! the generated table and evaluate to the expected predicate on a small grid.
//!
//! The machine-checked half lives in `src/proofs/{cond_ops,money_shave,wrong_guard}.rs`.

use rules_cond_ir::ctx::{IrCtx, SetId};
use rules_cond_ir::vars::VarId;

#[path = "../src/generated/mod.rs"]
mod generated;
#[path = "../src/ir_fill.rs"]
mod ir_fill;

// The #[path] includes pull in world via ir_fill; stub the bits ir_fill needs.
mod world {
    pub const MAX_PLAYERS: usize = 4;
    pub const TOK_PP_POS: &str = "P✽P粉丝(正)";
    pub const TOK_PP_NEG: &str = "P✽P粉丝(反)";
    pub struct World {
        pub n_players: i32,
        pub money: [i32; MAX_PLAYERS],
        pub out: [bool; MAX_PLAYERS],
        pub pos: [i32; MAX_PLAYERS],
        pub fire: [i32; MAX_PLAYERS],
        pub stay: [i32; MAX_PLAYERS],
        pub fans_pos: [i32; MAX_PLAYERS],
        pub fans_neg: [i32; MAX_PLAYERS],
        pub hand_resonance: [i32; MAX_PLAYERS],
        pub trig_player: i32,
        pub trig_value: i32,
    }
}

#[test]
fn pilot_guards_are_in_the_generated_table() {
    for s in [
        ir_fill::src::AYA_PAY_ADD,
        ir_fill::src::IS_CIRCLE,
        ir_fill::src::CHAIN_PAY,
    ] {
        assert!(
            generated::ALL.iter().any(|g| g.src == s),
            "pilot guard missing from generated table: {s}"
        );
    }
}

#[test]
fn aya_pay_add_guard_evaluates() {
    let prog = ir_fill::guard_by_src(ir_fill::src::AYA_PAY_ADD);
    for (actor, owner, value, expect) in [
        (0i64, 0i64, 1i64, true),
        (0, 0, 0, false),
        (0, 0, -1, false),
        (1, 0, 5, false),
        (0, 1, 5, false),
        (2, 2, 999, true),
    ] {
        let mut c = IrCtx::default();
        c.n_players = 4;
        c.set_scalar(VarId::Actor, actor);
        c.set_scalar(VarId::Owner, owner);
        c.set_scalar(VarId::Value, value);
        assert_eq!(prog.eval_bool(&c), expect, "actor={actor} owner={owner} value={value}");
    }
}

#[test]
fn is_circle_guard_evaluates() {
    let prog = ir_fill::guard_by_src(ir_fill::src::IS_CIRCLE);
    for (actor, owner, tile, on_circle, expect) in [
        (0i64, 0i64, 1i64, true, true),
        (0, 0, 2, true, false),
        (1, 0, 1, true, false),
        (0, 0, 1, false, false),
    ] {
        let mut c = IrCtx::default();
        c.n_players = 4;
        c.set_scalar(VarId::Actor, actor);
        c.set_scalar(VarId::Owner, owner);
        c.set_scalar(VarId::TileId, tile);
        if on_circle {
            c.set_insert(SetId::Circle, 1);
        }
        assert_eq!(
            prog.eval_bool(&c),
            expect,
            "actor={actor} owner={owner} tile={tile} on_circle={on_circle}"
        );
    }
}

#[test]
fn chain_pay_guard_evaluates() {
    let prog = ir_fill::guard_by_src(ir_fill::src::CHAIN_PAY);
    for (actor, owner, value, target, has_pay, expect) in [
        (0i64, 0i64, 1i64, 1i64, true, true),
        (0, 0, 0, 1, true, false),
        (0, 0, 1, 0, true, false), // target == owner
        (0, 0, 1, -1, true, false), // target < 0
        (1, 0, 1, 1, true, false), // actor != owner
        (0, 0, 1, 1, false, false), // no Pay link
    ] {
        let mut c = IrCtx::default();
        c.n_players = 4;
        c.set_scalar(VarId::Actor, actor);
        c.set_scalar(VarId::Owner, owner);
        c.set_scalar(VarId::Value, value);
        c.set_scalar(VarId::Target, target);
        if has_pay {
            c.set_insert(SetId::ChainKinds, 9);
        }
        assert_eq!(
            prog.eval_bool(&c),
            expect,
            "actor={actor} owner={owner} value={value} target={target} has_pay={has_pay}"
        );
    }
}
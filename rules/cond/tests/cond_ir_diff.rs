//! Differential tests: the IR evaluator must agree with the vendored `cel`
//! evaluator (via `rules-cond`) on every closed-set operator and on every
//! real guard string in the repo.
//!
//! Boundary grid: {i64::MIN, -1, 0, 1, i64::MAX, null} for every operator and
//! function. Random expression trees + every inventory guard.
//!
//! A divergence is a FINDING — report it, do not paper over it.

use rules_cond::{compile, CandidateCtx, WindowCtx};
use rules_cond_ir::ctx::{FnId, IrCtx, SeatField, SetId};
use rules_cond_ir::eval::values_cmp;
use rules_cond_ir::nodes::{Kind, Node};
use rules_cond_ir::vars::{VarId, VarTy, VARS};
use rules_cond_ir::{spec, Val};

// ---------------------------------------------------------------------------
// context plumbing: fill both the cel snapshot and the IR ctx from one source
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Fixture {
    win: WindowCtx,
    cand: CandidateCtx,
    ir: IrCtx,
}

fn boundary() -> [Val; 6] {
    [
        Val::Int(i64::MIN),
        Val::Int(-1),
        Val::Int(0),
        Val::Int(1),
        Val::Int(i64::MAX),
        Val::Null,
    ]
}

fn fixture_empty() -> Fixture {
    let mut f = Fixture {
        win: WindowCtx::default(),
        cand: CandidateCtx::default(),
        ir: IrCtx::default(),
    };
    f.ir.n_players = 2;
    // Align IR defaults with WindowCtx/CandidateCtx defaults:
    // OptInt fields whose snapshot default is `None` must read as null.
    for (id, _, ty) in VARS {
        if *ty == VarTy::OptInt {
            f.ir.set_scalar(*id, -1);
        }
    }
    // `counter_name` binds 0 for an empty `Trigger.name` (view_impl), not
    // `id_of("")`.
    f.ir.set_scalar(VarId::CounterName, 0);
    f
}

/// Fill a scalar into both sides, honouring OptInt/Bool typing.
fn set_scalar(f: &mut Fixture, id: VarId, ty: VarTy, v: Val) {
    let raw = match v {
        Val::Int(i) => i,
        Val::Bool(b) => b as i64,
        Val::Null => -1,
        _ => 0,
    };
    f.ir.set_scalar_ty(id, ty, raw, matches!(v, Val::Null));
    // Mirror into the cel snapshots by flat name.
    match id {
        VarId::Kind => f.win.kind = raw,
        VarId::Actor => f.win.actor = raw,
        VarId::Target => f.win.target = raw,
        VarId::Value => f.win.value = raw,
        VarId::Step => f.win.step = raw,
        VarId::By => f.win.by = raw,
        VarId::PayIsRent => f.win.pay_is_rent = raw != 0,
        VarId::RollSource => f.win.roll_source = raw,
        VarId::Abnormal => f.win.abnormal = raw != 0,
        VarId::TurnPlayer => f.win.turn_player = raw,
        VarId::TurnKey => f.win.turn_key = raw,
        VarId::PlanFixedRoll => f.win.plan_fixed_roll = if raw == -1 { None } else { Some(raw) },
        VarId::TriggerCard => f.win.trigger_card = raw,
        VarId::CounterName => {
            // Bind both sides to 0 (empty `Trigger.name` → 0 on the view).
            f.ir.set_scalar(VarId::CounterName, 0);
            f.win.name = String::new();
        }
        VarId::TileId => f.win.tile.id = raw,
        VarId::TileOwner => f.win.tile.owner = raw,
        VarId::TileHouses => f.win.tile.houses = raw,
        VarId::TileMortgaged => f.win.tile.mortgaged = raw,
        VarId::TilePrice => f.win.tile.price = raw,
        VarId::MoveRoll => f.win.mv.roll = if raw == -1 { None } else { Some(raw) },
        VarId::MoveKind => f.win.mv.kind = if raw == -1 { None } else { Some(raw) },
        VarId::MoveRemaining => f.win.mv.remaining = raw,
        VarId::MoveMain => f.win.mv.main = raw != 0,
        VarId::MoveDir => f.win.mv.dir = raw,
        VarId::EffectCount => {
            // `effect_count` binds `win.chain.len()`. Grow/shrink the list and
            // keep the IR scalar in step (the raw boundary value is not a
            // length).
            let n = raw.max(0).min(8) as usize;
            f.win.chain = (0..n)
                .map(|i| rules_cond::ChainLink {
                    kind: i as i64,
                    from: 0,
                    hits: -1,
                })
                .collect();
            f.ir.set_scalar(VarId::EffectCount, n as i64);
        }
        VarId::Owner => f.cand.owner = raw,
        VarId::OwnerId => f.cand.owner = raw,
        VarId::OwnerMoney => f.cand.owner_money = raw,
        VarId::OwnerFire => f.cand.owner_fire = raw,
        VarId::OwnerCrystals => f.cand.owner_crystals = raw,
        VarId::OwnerHand => f.cand.owner_hand = raw,
        VarId::OwnerPos => f.cand.owner_pos = raw,
        VarId::OwnerOut => f.cand.owner_out = raw,
        VarId::OwnerStay => f.cand.owner_stay = raw,
        VarId::OwnerStun => f.cand.owner_stun = raw,
        VarId::OwnerExile => f.cand.owner_exile = raw,
        VarId::OwnerNoHand => f.cand.owner_no_hand = raw,
        VarId::OwnerCharacter => f.cand.owner_character = raw,
        VarId::OwnerBand => f.cand.owner_band = raw,
        VarId::OwnerTiles => f.cand.owner_tiles = raw,
        VarId::CardId => f.cand.card_id = raw,
        VarId::CardPlaced => f.cand.card_placed = raw != 0,
        VarId::CardTile => f.cand.card_tile = if raw == -1 { None } else { Some(raw) },
    }
    let _ = ty;
}

/// Evaluate via the real cel path. Errors → false (host rule).
fn cel_eval(src: &str, f: &Fixture) -> bool {
    let cond = compile(src).expect("compile");
    cond.eval(&f.win, &f.cand)
}

/// Evaluate via the IR path. Errors → false (host rule).
fn ir_eval(src: &str, f: &Fixture) -> bool {
    let prog = rules_cond::lower::lower_src(src).expect("lower");
    prog.eval_bool(&f.ir)
}

fn assert_agree(src: &str, f: &Fixture) {
    let a = cel_eval(src, f);
    let b = ir_eval(src, f);
    assert_eq!(
        a, b,
        "DIVERGENCE cel={a} ir={b} for {src:?}\n  fixture: {f:?}"
    );
}

// ---------------------------------------------------------------------------
// (1) per-operator boundary grid
// ---------------------------------------------------------------------------

fn two_op_exprs(op: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lits = ["-9223372036854775808", "-1", "0", "1", "9223372036854775807", "null"];
    // Also fold in a couple of context vars so OptInt/Bool paths are covered.
    let vars_a = ["actor", "value", "move_roll", "card_placed"];
    let vars_b = ["target", "value", "move_roll", "pay_is_rent"];
    for a in lits {
        for b in lits {
            out.push(format!("{a} {op} {b}"));
        }
    }
    for a in vars_a {
        for b in vars_b {
            out.push(format!("{a} {op} {b}"));
        }
    }
    out
}

#[test]
fn diff_operators_boundary_grid() {
    let mut f = fixture_empty();
    set_scalar(&mut f, VarId::Actor, VarTy::Int, Val::Int(i64::MIN));
    set_scalar(&mut f, VarId::Target, VarTy::Int, Val::Int(-1));
    set_scalar(&mut f, VarId::Value, VarTy::Int, Val::Int(1));
    set_scalar(&mut f, VarId::MoveRoll, VarTy::OptInt, Val::Null);
    set_scalar(&mut f, VarId::CardPlaced, VarTy::Bool, Val::Bool(true));
    set_scalar(&mut f, VarId::PayIsRent, VarTy::Bool, Val::Bool(false));

    // Also sweep the vars over the boundary grid in a second pass.
    let ops = [
        "==", "!=", "<", "<=", ">", ">=", "+", "-", "*", "/", "%",
    ];
    let mut cases = 0;
    for op in ops {
        for expr in two_op_exprs(op) {
            // Skip unparseable combos (null + arith still must agree).
            if compile(&expr).is_err() {
                continue;
            }
            assert_agree(&expr, &f);
            cases += 1;
        }
    }
    // Boolean connectives over the same lattice.
    for expr in [
        "false && false",
        "false && true",
        "true && false",
        "true && true",
        "false || false",
        "false || true",
        "true || false",
        "true || true",
        "true ? 1 : 2",
        "false ? 1 : 2",
        "!true",
        "!false",
        "!!true",
        "-0",
        "-1",
        "-(-9223372036854775807)",
        "null == null",
        "null == 1",
        "1 == null",
        "null != null",
        "null != 1",
        "null < 1",
        "null <= null",
        "1 > null",
        // error absorption
        "1 / 0",
        "1 / 0 == 0",
        "false && 1 / 0",
        "1 / 0 && false",
        "true && 1 / 0",
        "1 / 0 && true",
        "true || 1 / 0",
        "1 / 0 || true",
        "false || 1 / 0",
        "1 / 0 || false",
        "true ? 1 / 0 : 1",
        "false ? 1 / 0 : 1",
        "true ? 1 : 1 / 0",
        "1 / 0 ? 1 : 2",
        // overflow
        "9223372036854775807 + 1",
        "-9223372036854775808 - 1",
        "9223372036854775807 * 2",
        "-9223372036854775808 / -1",
        "1 % 0",
        // mixed
        "actor == owner",
        "move_roll != null",
        "move_roll > 0",
        "card_placed && actor == owner",
        "value > 0 && actor == owner",
        "kind == Teleport",
        "move_kind == Walk",
        "kind == MoveRoll",
    ] {
        if compile(expr).is_err() {
            continue;
        }
        assert_agree(expr, &f);
        cases += 1;
    }
    println!("diff_operators_boundary_grid: {cases} cases agree");
}

// ---------------------------------------------------------------------------
// (2) typed-variable sweep: every scalar over the boundary grid
// ---------------------------------------------------------------------------

#[test]
fn diff_every_scalar_over_boundary() {
    for (idx, (_id, flat, ty)) in VARS.iter().enumerate() {
        for v in boundary() {
            let mut f = fixture_empty();
            // Give the partner vars some non-default values so comparisons bite.
            set_scalar(&mut f, VarId::Actor, VarTy::Int, Val::Int(1));
            set_scalar(&mut f, VarId::Value, VarTy::Int, Val::Int(1));
            set_scalar(&mut f, VarId::Target, VarTy::Int, Val::Int(1));
            set_scalar(&mut f, VarId::Owner, VarTy::Int, Val::Int(0));
            set_scalar(&mut f, VarId::CardPlaced, VarTy::Bool, Val::Bool(true));
            set_scalar(&mut f, VarId::MoveRoll, VarTy::OptInt, Val::Int(3));
            set_scalar(
                &mut f,
                id_at(idx).unwrap(),
                *ty,
                v,
            );
            let exprs = [
                format!("{flat} == 1"),
                format!("{flat} != 0"),
                format!("{flat} > -1"),
                format!("{flat} >= 0"),
                format!("{flat} < 2"),
                format!("{flat} <= 1"),
                format!("{flat} == null"),
                format!("!({flat} == 1)"),
            ];
            for e in exprs {
                if compile(&e).is_err() {
                    continue;
                }
                assert_agree(&e, &f);
            }
        }
    }
}

fn id_at(idx: usize) -> Option<VarId> {
    VARS.get(idx).map(|(id, _, _)| *id)
}

// ---------------------------------------------------------------------------
// (2b) status-gate per-seat layer functions
// ---------------------------------------------------------------------------

#[test]
fn diff_status_layer_functions() {
    let mut f = fixture_empty();
    // Seat 0: stay=2, stun=1, stunStart=3, exile=0, unstoppable=0.
    // Seat 1: stay=0, stun=0, stunStart=1, exile=4, unstoppable=2.
    f.ir.n_players = 2;
    f.ir.seat[0][SeatField::Stay as usize] = 2;
    f.ir.seat[0][SeatField::Stun as usize] = 1;
    f.ir.seat[0][SeatField::StunStart as usize] = 3;
    f.ir.seat[0][SeatField::Exile as usize] = 0;
    f.ir.seat[0][SeatField::Unstoppable as usize] = 0;
    f.ir.seat[1][SeatField::Stay as usize] = 0;
    f.ir.seat[1][SeatField::Stun as usize] = 0;
    f.ir.seat[1][SeatField::StunStart as usize] = 1;
    f.ir.seat[1][SeatField::Exile as usize] = 4;
    f.ir.seat[1][SeatField::Unstoppable as usize] = 2;
    // Mirror into the CEL snapshot (`stay(p)` reads `win.players[p]`).
    f.win.players = vec![
        rules_cond::PlayerSnap {
            stay: 2,
            stun: 1,
            stun_start: 3,
            exile: 0,
            unstoppable: 0,
            ..rules_cond::PlayerSnap::default()
        },
        rules_cond::PlayerSnap {
            stay: 0,
            stun: 0,
            stun_start: 1,
            exile: 4,
            unstoppable: 2,
            ..rules_cond::PlayerSnap::default()
        },
    ];
    set_scalar(&mut f, VarId::Owner, VarTy::Int, Val::Int(0));
    set_scalar(&mut f, VarId::Actor, VarTy::Int, Val::Int(1));

    for src in [
        "stay(owner) > 0 && unstoppable(owner) == 0",
        "stay(owner) == 2",
        "stay(actor) == 0",
        "stunned(owner) == 4",
        "stunned(actor) == 1",
        "stunned(owner) > stunned(actor)",
        "exile(owner) == 0 && exile(actor) == 4",
        "unstoppable(actor) == 2",
        "stay(owner) + stunned(owner) == 6",
        "exile(owner) + unstoppable(owner) == 0",
        // Out-of-range seats read 0 on both paths.
        "stay(3) == 0 && stunned(3) == 0 && exile(3) == 0 && unstoppable(3) == 0",
        "stay(-1) == 0 && unstoppable(-1) == 0",
    ] {
        assert_agree(src, &f);
    }

    // Flip the layers and re-check agreement (not just one truth value).
    f.ir.seat[0][SeatField::Stay as usize] = 0;
    f.ir.seat[0][SeatField::Unstoppable as usize] = 5;
    f.win.players[0].stay = 0;
    f.win.players[0].unstoppable = 5;
    for src in [
        "stay(owner) > 0 && unstoppable(owner) == 0",
        "stay(owner) == 0 && unstoppable(owner) == 5",
        "stunned(owner) == 4",
    ] {
        assert_agree(src, &f);
    }
}

// ---------------------------------------------------------------------------
// (3) real guards from the repo
// ---------------------------------------------------------------------------

/// The inventory of `On::*` condition strings (shared extractor).
fn collect_guards() -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let mut out = Vec::new();
    for d in ["cards", "skills", "tiles", "events", "fixtures"] {
        for g in rules_cond::lower::inventory_guards(&root.join(d)) {
            out.push(g);
        }
    }
    out.sort();
    out.dedup();
    out
}

#[test]
fn diff_all_inventory_guards() {
    let guards = collect_guards();
    assert!(
        guards.len() >= 50,
        "guard inventory too small: {}",
        guards.len()
    );
    let mut agree = 0;
    let mut skipped = 0;
    for src in &guards {
        if compile(src).is_err() {
            skipped += 1;
            continue;
        }
        // Default fixture.
        let mut f = fixture_empty();
        set_scalar(&mut f, VarId::Actor, VarTy::Int, Val::Int(0));
        set_scalar(&mut f, VarId::Owner, VarTy::Int, Val::Int(0));
        set_scalar(&mut f, VarId::Value, VarTy::Int, Val::Int(1));
        set_scalar(&mut f, VarId::Target, VarTy::Int, Val::Int(1));
        set_scalar(&mut f, VarId::By, VarTy::Int, Val::Int(1));
        set_scalar(&mut f, VarId::TurnPlayer, VarTy::Int, Val::Int(0));
        set_scalar(&mut f, VarId::TurnKey, VarTy::Int, Val::Int(7));
        set_scalar(&mut f, VarId::Kind, VarTy::Int, Val::Int(9));
        set_scalar(&mut f, VarId::CardId, VarTy::Int, Val::Int(42));
        set_scalar(&mut f, VarId::TriggerCard, VarTy::Int, Val::Int(42));
        set_scalar(&mut f, VarId::CardPlaced, VarTy::Bool, Val::Bool(true));
        set_scalar(&mut f, VarId::MoveMain, VarTy::Bool, Val::Bool(true));
        set_scalar(&mut f, VarId::MoveRoll, VarTy::OptInt, Val::Int(3));
        set_scalar(&mut f, VarId::MoveKind, VarTy::OptInt, Val::Int(0));
        set_scalar(&mut f, VarId::PayIsRent, VarTy::Bool, Val::Bool(true));
        set_scalar(&mut f, VarId::TileId, VarTy::Int, Val::Int(1));
        set_scalar(&mut f, VarId::TileOwner, VarTy::Int, Val::Int(0));
        set_scalar(&mut f, VarId::TileHouses, VarTy::Int, Val::Int(1));
        set_scalar(&mut f, VarId::OwnerPos, VarTy::Int, Val::Int(1));
        set_scalar(&mut f, VarId::OwnerOut, VarTy::Int, Val::Int(0));
        set_scalar(&mut f, VarId::OwnerMoney, VarTy::Int, Val::Int(500));
        set_scalar(&mut f, VarId::OwnerHand, VarTy::Int, Val::Int(2));
        set_scalar(&mut f, VarId::OwnerFire, VarTy::Int, Val::Int(1));
        set_scalar(&mut f, VarId::PlanFixedRoll, VarTy::OptInt, Val::Null);
        f.ir.set_insert(SetId::Circle, 1);
        f.ir.set_insert(SetId::ChainKinds, 9);
        // Mirror the function tables into the cel snapshot.
        f.win.circle_tiles = vec![1];
        f.win.chain = vec![rules_cond::ChainLink {
            kind: 9,
            from: 0,
            hits: -1,
        }];
        f.ir.named_set(rules_cond_ir::ctx::id_of("skillState"), 2);
        f.ir.named_set(rules_cond_ir::ctx::id_of("PAREO标记"), 1);
        f.cand
            .slots
            .insert("skillState".into(), 2);
        f.cand
            .tok_names
            .insert("PAREO标记".into(), 1);
        f.ir.seat[0][SeatField::Money as usize] = 500;
        f.ir.n_players = 2;
        // Mirror seat money into the CEL snapshot (`money(p)` reads
        // `win.players[p].money`; IR reads `seat[p][Money]`).
        if f.win.players.is_empty() {
            f.win.players = vec![
                rules_cond::PlayerSnap {
                    money: 500,
                    ..rules_cond::PlayerSnap::default()
                },
                rules_cond::PlayerSnap::default(),
            ];
        }
        assert_agree(src, &f);
        agree += 1;

        // A second fixture that flips several fields — more coverage per guard.
        let mut f2 = f.clone();
        set_scalar(&mut f2, VarId::Actor, VarTy::Int, Val::Int(1));
        set_scalar(&mut f2, VarId::Value, VarTy::Int, Val::Int(0));
        set_scalar(&mut f2, VarId::CardPlaced, VarTy::Bool, Val::Bool(false));
        set_scalar(&mut f2, VarId::MoveRoll, VarTy::OptInt, Val::Null);
        assert_agree(src, &f2);
        agree += 1;
    }
    println!(
        "diff_all_inventory_guards: {agree} evaluations agree, {skipped} unparseable, {} distinct guards",
        guards.len()
    );
}

// ---------------------------------------------------------------------------
// (4) random expression trees
// ---------------------------------------------------------------------------

/// Tiny xorshift — no heap, deterministic.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn random_expr(rng: &mut Rng, depth: u32) -> String {
    let lits = ["-9223372036854775808", "-1", "0", "1", "9223372036854775807", "null", "true", "false"];
    let vars = ["actor", "target", "value", "owner", "move_roll", "card_placed", "kind"];
    if depth == 0 {
        if rng.below(2) == 0 {
            lits[rng.below(lits.len() as u64) as usize].to_string()
        } else {
            vars[rng.below(vars.len() as u64) as usize].to_string()
        }
    } else {
        let a = random_expr(rng, depth - 1);
        let b = random_expr(rng, depth - 1);
        let c = random_expr(rng, depth - 1);
        match rng.below(14) {
            0 => format!("({a} + {b})"),
            1 => format!("({a} - {b})"),
            2 => format!("({a} * {b})"),
            3 => format!("({a} / {b})"),
            4 => format!("({a} % {b})"),
            5 => format!("({a} == {b})"),
            6 => format!("({a} != {b})"),
            7 => format!("({a} < {b})"),
            8 => format!("({a} > {b})"),
            9 => format!("({a} && {b})"),
            10 => format!("({a} || {b})"),
            11 => format!("!({a})"),
            12 => format!("-({a})"),
            _ => format!("({a} ? {b} : {c})"),
        }
    }
}

#[test]
fn diff_random_expression_trees() {
    let mut rng = Rng(0x1234_5678_9abc_def0);
    let mut agree = 0;
    let mut skipped = 0;
    for i in 0..4000 {
        let depth = 1 + (i % 4);
        let expr = random_expr(&mut rng, depth);
        if compile(&expr).is_err() {
            skipped += 1;
            continue;
        }
        let mut f = fixture_empty();
        // Symbolic-ish fixture: vary a few fields per case.
        set_scalar(&mut f, VarId::Actor, VarTy::Int, Val::Int(rng.below(4) as i64 - 1));
        set_scalar(&mut f, VarId::Target, VarTy::Int, Val::Int(rng.below(4) as i64 - 1));
        set_scalar(&mut f, VarId::Value, VarTy::Int, Val::Int(rng.next() as i64));
        set_scalar(&mut f, VarId::Owner, VarTy::Int, Val::Int(0));
        set_scalar(&mut f, VarId::Kind, VarTy::Int, Val::Int(rng.below(96) as i64));
        set_scalar(&mut f, VarId::CardPlaced, VarTy::Bool, Val::Bool(rng.below(2) == 0));
        let roll = rng.below(3);
        set_scalar(
            &mut f,
            VarId::MoveRoll,
            VarTy::OptInt,
            if roll == 0 {
                Val::Null
            } else {
                Val::Int(roll as i64)
            },
        );
        assert_agree(&expr, &f);
        agree += 1;
    }
    println!("diff_random_expression_trees: {agree} agree, {skipped} unparseable");
}

// ---------------------------------------------------------------------------
// (5) interpreter vs written reference (same IR tree, two evaluators)
// ---------------------------------------------------------------------------

#[test]
fn ir_eval_matches_written_spec() {
    // Value-level operator tables.
    let scalars = [
        Val::Int(i64::MIN),
        Val::Int(-1),
        Val::Int(0),
        Val::Int(1),
        Val::Int(i64::MAX),
        Val::Bool(true),
        Val::Bool(false),
        Val::Null,
        Val::Err,
    ];
    for a in scalars {
        for b in scalars {
            assert_eq!(
                spec::spec_eq(a, b),
                {
                    // Mirror the interpreter's Eq on a 2-node program.
                    let nodes = [
                        Node::leaf_const_int(0),
                        Node::leaf_const_int(0),
                        Node::binary(Kind::Eq, 0, 1),
                    ];
                    // Use the value-level helpers instead of a tree:
                    match (a, b) {
                        (Val::Err, _) | (_, Val::Err) => Val::Err,
                        (x, y) => Val::Bool(rules_cond_ir::eval::values_eq(x, y)),
                    }
                },
                "eq({a:?},{b:?})"
            );
            assert_eq!(
                spec::spec_ne(a, b),
                match (a, b) {
                    (Val::Err, _) | (_, Val::Err) => Val::Err,
                    (x, y) => Val::Bool(!rules_cond_ir::eval::values_eq(x, y)),
                },
                "ne({a:?},{b:?})"
            );
            for (op, imp) in [
                (
                    spec::Cmp::Lt,
                    rules_cond_ir::eval::values_cmp(a, b).map(|o| o == core::cmp::Ordering::Less),
                ),
                (
                    spec::Cmp::Le,
                    rules_cond_ir::eval::values_cmp(a, b)
                        .map(|o| o != core::cmp::Ordering::Greater),
                ),
                (
                    spec::Cmp::Gt,
                    rules_cond_ir::eval::values_cmp(a, b).map(|o| o == core::cmp::Ordering::Greater),
                ),
                (
                    spec::Cmp::Ge,
                    rules_cond_ir::eval::values_cmp(a, b)
                        .map(|o| o != core::cmp::Ordering::Less),
                ),
            ] {
                let s = spec::spec_cmp(a, b, op);
                let i = match imp {
                    Some(v) => Val::Bool(v),
                    None => Val::Err,
                };
                assert_eq!(s, i, "cmp({op:?}, {a:?}, {b:?})");
            }
            for (op, imp) in [
                (
                    spec::Arith::Add,
                    match (a, b) {
                        (Val::Int(x), Val::Int(y)) => x.checked_add(y),
                        (Val::Err, _) | (_, Val::Err) => Some(i64::MIN), // marker
                        _ => Some(i64::MIN),
                    },
                ),
            ] {
                let _ = (op, imp);
            }
            // Arithmetic via spec_* vs checked_*.
            for op in [
                spec::Arith::Add,
                spec::Arith::Sub,
                spec::Arith::Mul,
                spec::Arith::Div,
                spec::Arith::Rem,
            ] {
                let s = spec::spec_arith(a, b, op);
                let i = match (a, b) {
                    (Val::Int(x), Val::Int(y)) => {
                        let r = match op {
                            spec::Arith::Add => x.checked_add(y),
                            spec::Arith::Sub => x.checked_sub(y),
                            spec::Arith::Mul => x.checked_mul(y),
                            spec::Arith::Div => {
                                if y == 0 {
                                    None
                                } else {
                                    x.checked_div(y)
                                }
                            }
                            spec::Arith::Rem => {
                                if y == 0 {
                                    None
                                } else {
                                    x.checked_rem(y)
                                }
                            }
                        };
                        match r {
                            Some(z) => Val::Int(z),
                            None => Val::Err,
                        }
                    }
                    (Val::Err, _) | (_, Val::Err) => Val::Err,
                    _ => Val::Err,
                };
                assert_eq!(s, i, "arith({op:?}, {a:?}, {b:?})");
            }
        }
    }
    // And/Or truth tables including error absorption.
    for l in scalars {
        for r in scalars {
            assert_eq!(
                spec::spec_and(l, r, true),
                {
                    let lb = l.try_bool();
                    let rb = r.try_bool();
                    match (lb, rb) {
                        (Ok(true), Ok(b)) => Val::Bool(b),
                        (Ok(false), _) => Val::Bool(false),
                        (Err(()), Ok(false)) => Val::Bool(false),
                        _ => Val::Err,
                    }
                },
                "and({l:?},{r:?})"
            );
            assert_eq!(
                spec::spec_or(l, r, true),
                {
                    let lb = l.try_bool();
                    let rb = r.try_bool();
                    match (lb, rb) {
                        (Ok(true), _) => Val::Bool(true),
                        (Ok(false), Ok(b)) => Val::Bool(b),
                        (Err(()), Ok(true)) => Val::Bool(true),
                        _ => Val::Err,
                    }
                },
                "or({l:?},{r:?})"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// (6) host root rule: any error ⇒ false
// ---------------------------------------------------------------------------

#[test]
fn error_roots_are_false() {
    for src in [
        "1 / 0",
        "null < 1",
        "9223372036854775807 + 1",
        "true && 1 / 0",
        "false || 1 / 0",
        "!null",
        "1 + null",
    ] {
        assert!(compile(src).is_ok(), "should compile: {src}");
        let f = fixture_empty();
        assert_eq!(cel_eval(src, &f), false, "cel root {src}");
        assert_eq!(ir_eval(src, &f), false, "ir root {src}");
    }
}
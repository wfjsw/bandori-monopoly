//! Criterion-free benchmark for the G1 condition evaluator (docs/GUARDS.md
//! §4.1, §6). Run with:
//!
//! ```text
//! CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
//!   cargo run --release -p rules-cond --example bench_cond
//! ```
//!
//! Prints ns/eval for the four typical conditions, with a `WindowScope` reused
//! across candidates vs a rebuilt one per call. Target: well under the ~45 µs
//! wasm instantiate the prefilter replaces.

use std::time::Instant;

use rules_cond::{compile, CandidateCtx, ChainLink, Cond, MoveSnap, PlayerSnap, TileSnap, WindowCtx, WindowScope};

fn window() -> WindowCtx {
    WindowCtx {
        kind: 0,
        actor: 1,
        target: -1,
        tile: TileSnap {
            id: 3,
            owner: 0,
            houses: 2,
            mortgaged: 0,
            price: 400,
        },
        value: 6000,
        step: 4,
        by: 1,
        pay_is_rent: false,
        mv: MoveSnap {
            roll: Some(5),
            kind: Some(rules_cond::mv::Walk),
            remaining: 5,
            main: true,
        },
        roll_source: 0,
        abnormal: false,
        chain: vec![
            ChainLink { kind: rules_cond::trig::Pay, from: 1, hits: 0 },
            ChainLink { kind: rules_cond::trig::Effect, from: 2, hits: -1 },
        ],
        turn_player: 1,
        turn_key: 7,
        trigger_card: 0,
        players: (0..4)
            .map(|i| PlayerSnap {
                money: 500 + i * 100,
                character: i,
                band: i % 2,
                ..PlayerSnap::default()
            })
            .collect(),
        tile_ids: [("festival".to_string(), 9)].into_iter().collect(),
        circle_tiles: vec![0],
        ring_tiles: vec![],
        live_house_tiles: vec![],
        buyable_tiles: vec![],
    }
}

fn candidate(owner: i64) -> CandidateCtx {
    CandidateCtx {
        owner,
        owner_money: 500 + owner * 100,
        owner_fire: 2,
        owner_character: owner,
        owner_band: owner % 2,
        card_id: 42,
        slots: [("asUsualTurn".to_string(), 3)].into_iter().collect(),
        tok_names: [("水母标记".to_string(), 1)].into_iter().collect(),
        ..CandidateCtx::default()
    }
}

const SAMPLES: u64 = 200_000;

fn bench(name: &str, cond: &Cond, win: &WindowCtx, reuse: bool) -> f64 {
    // Warm-up.
    for _ in 0..1_000 {
        let _ = cond.eval(win, &candidate(0));
    }
    let scope = WindowScope::new(win);
    let mut sink = 0u64;
    let t0 = Instant::now();
    for i in 0..SAMPLES {
        let cand = candidate((i % 4) as i64);
        let ok = if reuse {
            scope.eval(cond, &cand)
        } else {
            cond.eval(win, &cand)
        };
        sink = sink.wrapping_add(ok as u64);
    }
    let dt = t0.elapsed();
    let ns = dt.as_nanos() as f64 / SAMPLES as f64;
    println!(
        "{name:44} {ns:9.1} ns/eval   ({}, sink={sink})",
        if reuse { "scope reused " } else { "scope rebuilt" }
    );
    ns
}

fn main() {
    let win = window();
    let cases: &[(&str, &str)] = &[
        ("actor == owner", "actor == owner"),
        (
            "actor != owner && owner.money >= 500",
            "actor != owner && owner.money >= 500",
        ),
        (
            "effect.hits(owner) && value >= 5000",
            "effect.hits(owner) && value >= 5000",
        ),
        (
            "tile.owner == owner && tile.houses > 0",
            "tile.owner == owner && tile.houses > 0",
        ),
        (
            "move.kind == Walk && move.roll != null",
            "move.kind == Walk && move.roll != null",
        ),
        ("effect.has(Pay)", "effect.has(Pay)"),
        (
            "slot('asUsualTurn') != turn_key",
            "slot('asUsualTurn') != turn_key",
        ),
        ("money(owner) >= 500", "money(owner) >= 500"),
    ];

    println!("rules-cond G1 eval benchmark  (n = {SAMPLES} per cell)\n");
    println!("target: << 45 000 ns (wasmi wasm instantiate, GUARDS.md §1)\n");

    let mut sum_reuse = 0.0;
    let mut sum_rebuild = 0.0;
    for (name, src) in cases {
        let cond = compile(src).expect(src);
        sum_reuse += bench(name, &cond, &win, true);
        sum_rebuild += bench(name, &cond, &win, false);
        println!();
    }

    println!(
        "mean: scope reused {:.1} ns/eval, scope rebuilt {:.1} ns/eval  ({}x)",
        sum_reuse / cases.len() as f64,
        sum_rebuild / cases.len() as f64,
        sum_rebuild / sum_reuse
    );

    // One-shot compile cost, for the load-time budget.
    let src = "actor != owner && owner.money >= 500";
    let t0 = Instant::now();
    for _ in 0..10_000 {
        let _ = compile(src).unwrap();
    }
    println!(
        "compile once: {:.1} µs/call (load-time; done once per guarded entry)",
        t0.elapsed().as_nanos() as f64 / 10_000.0 / 1000.0
    );

    // WindowScope construction cost (once per window, ~250 windows/game).
    let t0 = Instant::now();
    for _ in 0..10_000 {
        let _ = WindowScope::new(&win);
    }
    println!(
        "WindowScope::new: {:.1} µs/call (once per trigger window)",
        t0.elapsed().as_nanos() as f64 / 10_000.0 / 1000.0
    );

    // Lazy vs eager (docs/GUARDS.md §4.2b): fetch `money(seat)` through a
    // `&dyn CondView` at call time versus reading the pre-baked `_money` map
    // the window scope installs. The lazy path is the "no eager filling"
    // alternative; if it does not beat the ~1.2 µs/eval eager mean, keep
    // eager (a CEL Context wants owned 'static values, so lazy still has to
    // thread a pointer through a side channel).
    {
        use rules_cond::view::CondView;
        let cand0 = candidate(0);
        let probe = rules_cond::SnapshotView {
            win: &win,
            cand: &cand0,
        };
        // Warm-up.
        for _ in 0..1_000 {
            rules_cond::eval::with_live_view(&probe, || {
                std::hint::black_box(rules_cond::eval::lazy_money(0))
            });
        }
        let mut sink = 0i64;
        let t0 = Instant::now();
        for i in 0..SAMPLES {
            let seat = (i % 4) as i64;
            let v = rules_cond::eval::with_live_view(&probe, || {
                rules_cond::eval::lazy_money(seat)
            });
            sink = sink.wrapping_add(v);
        }
        println!(
            "lazy dyn-CondView money(seat)                {:9.1} ns/call  (sink={sink})",
            t0.elapsed().as_nanos() as f64 / SAMPLES as f64
        );
        // Same fetch through the baked WindowScope path (what production does).
        let scope = WindowScope::new(&win);
        let cond = compile("money(owner) >= 500").unwrap();
        let mut sink = 0u64;
        let t0 = Instant::now();
        for i in 0..SAMPLES {
            let c = candidate((i % 4) as i64);
            sink = sink.wrapping_add(scope.eval(&cond, &c) as u64);
        }
        println!(
            "eager WindowScope money(owner) >= 500       {:9.1} ns/eval  (sink={sink})",
            t0.elapsed().as_nanos() as f64 / SAMPLES as f64
        );
    }

    // Runtime-only load path (GUARDS.md §8): the browser never parses. It
    // decodes a postcard `Cond` and evaluates it -- the eval half is the
    // same code as above, the decode half replaces `compile`.
    #[cfg(feature = "wire")]
    {
        let src = "actor != owner && owner.money >= 500";
        let cond = compile(src).unwrap();
        let bytes = cond.to_bytes(false);
        let t0 = Instant::now();
        for _ in 0..10_000 {
            let _ = rules_cond::Cond::from_bytes(&bytes).unwrap();
        }
        println!(
            "from_bytes (runtime-only load): {:.1} µs/call  ({} B lean wire)",
            t0.elapsed().as_nanos() as f64 / 10_000.0 / 1000.0,
            bytes.len()
        );
        let loaded = rules_cond::Cond::from_bytes(&bytes).unwrap();
        assert_eq!(cond.expr(), loaded.expr());
        let scope = WindowScope::new(&win);
        let mut sum = 0.0;
        for _ in 0..SAMPLES {
            let t = Instant::now();
            let r = scope.eval(&loaded, &candidate(0));
            std::hint::black_box(r);
            sum += t.elapsed().as_nanos() as f64;
        }
        println!(
            "from_bytes + WindowScope::eval: {:.1} ns/eval (runtime-only path)",
            sum / SAMPLES as f64
        );
    }
}
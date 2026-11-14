//! Native pin of the fire-kernel window contract (`MARKERS-KANI-01`).
//!
//! `bandori_std::fire::{gain,spend}` must open the `markerGain` /
//! `ChargePaying` + `markerSpend` windows **before** the pots move, with the
//! host wire spelling of the counter (`"charge"`) and the *actual* payload
//! (`+landed` / `-n`). This is the cheap, concrete companion to the Kani
//! harnesses in `rules/kani/src/proofs/markers.rs`: it pins the same spec
//! predicates on the native host model, where a symbolically-invisible name or
//! ordering mistake shows up in seconds.
//!
//! Run with `cargo test -p rules-kani --test conf_fire_windows` (the file is
//! `#[ignore]`d with the other conformance suites).

use std::sync::{Mutex, MutexGuard, OnceLock};

use card_sdk::Msg;
use game_rules::native_shims::{install_host, take_host};
use rules_kani::spec::markers as spec;
use rules_kani::world::{self as kworld, World as KWorld, WINDOW_GAIN, WINDOW_RAISE, WINDOW_SPEND};

// Generated `HostOps -> rules_kani::host::dispatch` bridge (same file
// `tests/conf_model.rs` includes; build.rs writes it on every build).
mod sym_ops {
    include!(concat!(env!("OUT_DIR"), "/conf_sym_ops.rs"));
}
use sym_ops::SymOps;

fn model_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// A `World` is over a megabyte; run bodies on a wide stack.
fn big(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("test thread");
}

/// 2-seat world with a live `charge` row on `user` (`pre` / `cap`). The row is
/// what makes the seat writable without a bound persona skill.
fn fire_world(user: i32, pre: i32, cap: i32) -> KWorld {
    let mut w = KWorld::new();
    w.n_players = 2;
    w.trig_player = user;
    w.pos = [10, 20, 0, 0];
    w.charge[user as usize] = pre;
    w.charge_max[user as usize] = cap;
    w
}

fn install(w: KWorld) {
    card_sdk::native::clear();
    rules_kani::host::install();
    kworld::reset_world(w);
    // Card bodies reach the model through the native `ruleset_*` shims; those
    // need a `HostOps` installed (the generated bridge calls `dispatch`).
    let _ = install_host(Box::new(SymOps));
}

fn uninstall() {
    let _ = take_host();
}

/// The full grid the `fire_gain_iff` proof quantifies over, run concretely:
/// both seats, `pre ∈ 0..=2`, `cap ∈ pre+1..=pre+3`, `n ∈ 1..=4`. The window
/// must be the clamped actual on every cell (the Kani counterpart is the
/// same property, symbolically).
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn gain_window_grid_matches_the_spec() {
    big(|| {
        let _g = model_lock();
        for user in 0..2 {
            for pre in 0..=2 {
                for cap in (pre + 1)..=(pre + 3) {
                    for n in 1..=4 {
                        install(fire_world(user, pre, cap));
                        let landed = bandori_std::fire::gain(user, n, &Msg::new("proof")).unwrap();
                        let want = spec::fire_gain_actual(n, pre, cap);
                        let (wins, wins_n, charge) = kworld::with_world(|w| {
                            (w.windows, w.windows_n, w.charge[user as usize])
                        });
                        assert_eq!(landed, want, "user {user} pre {pre} cap {cap} n {n}");
                        assert!(want >= 1, "room is at least one");
                        assert_eq!(charge, pre + want, "landed exactly the actual");
                        assert!(
                            spec::fire_one_window(
                                &wins[..wins_n],
                                WINDOW_GAIN,
                                user,
                                bandori_std::fire::KEY,
                                want
                            ),
                            "window on user {user} pre {pre} cap {cap} n {n}"
                        );
                        uninstall();
                    }
                }
            }
        }
    });
}

/// `gain` under the cap lands exactly `n`: one `GainFire`, one `markerGain`
/// window of `+n` named `charge`, nothing else.
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn gain_opens_one_window_for_the_actual() {
    big(|| {
        let _g = model_lock();
        install(fire_world(0, 1, 4));
        let landed = bandori_std::fire::gain(0, 2, &Msg::new("proof")).unwrap();
        assert_eq!(landed, 2, "gain lands the unclamped request");
        let (wins, wins_n, log, log_n) = kworld::with_world(|w| {
            (w.windows, w.windows_n, w.log, w.log_n)
        });
        assert_eq!(wins_n, 1, "exactly one window");
        assert!(
            spec::fire_one_window(&wins[..wins_n], WINDOW_GAIN, 0, bandori_std::fire::KEY, 2),
            "the window carries kind/player/name/actual"
        );
        assert!(
            spec::fire_gain_log_ok(&log[..log_n], 0, 2),
            "exactly one GainFire of the actual"
        );
        uninstall();
    });
}

/// A clamped gain opens the window for the **actual**, not the request.
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn gain_at_cap_window_is_the_clamped_actual() {
    big(|| {
        let _g = model_lock();
        install(fire_world(1, 2, 3));
        let landed = bandori_std::fire::gain(1, 5, &Msg::new("proof")).unwrap();
        assert_eq!(landed, 1, "gain is partial at the cap");
        let (wins, wins_n, log, log_n) =
            kworld::with_world(|w| (w.windows, w.windows_n, w.log, w.log_n));
        assert_eq!(wins_n, 1, "exactly one window");
        assert!(
            spec::fire_one_window(&wins[..wins_n], WINDOW_GAIN, 1, bandori_std::fire::KEY, 1),
            "the window carries the clamped actual"
        );
        assert!(
            spec::fire_gain_log_ok(&log[..log_n], 1, 1),
            "exactly one GainFire of the actual"
        );
        uninstall();
    });
}

/// A seat with no live row and no persona skill holds no pots: the write is a
/// silent no-op (`World::new`'s demo `charge_max = 3` must be cleared — a
/// declared seat is a *row*, not the struct default).
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn gain_on_a_seat_with_no_row_is_a_noop() {
    big(|| {
        let _g = model_lock();
        install(fire_world(0, 0, 0));
        assert_eq!(bandori_std::fire::gain(0, 2, &Msg::new("proof")).unwrap(), 0);
        let (wins_n, log_n, narration_n) =
            kworld::with_world(|w| (w.windows_n, w.log_n, w.narration_n));
        assert_eq!((wins_n, log_n, narration_n), (0, 0, 0), "silent no-op");
        uninstall();
    });
}

/// At the cap the refusal narrates once and opens no window and no effect.
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn gain_at_cap_is_a_narrated_noop() {
    big(|| {
        let _g = model_lock();
        install(fire_world(0, 3, 3));
        assert_eq!(bandori_std::fire::gain(0, 2, &Msg::new("proof")).unwrap(), 0);
        let (wins_n, log_n, narration_n, charge) = kworld::with_world(|w| {
            (w.windows_n, w.log_n, w.narration_n, w.charge[0])
        });
        assert_eq!((wins_n, log_n, charge), (0, 0, 3), "nothing moved");
        assert_eq!(narration_n, 1, "the refusal is narrated once");
        uninstall();
    });
}

/// `spend` opens `ChargePaying` over the whole cost, then `markerSpend` of
/// `-n` — both before the pot moves.
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn spend_opens_raise_then_spend_windows_before_the_write() {
    big(|| {
        let _g = model_lock();
        install(fire_world(0, 3, 5));
        assert!(bandori_std::fire::spend(0, 2, &Msg::new("proof")).unwrap());
        let (wins, wins_n, log, log_n, charge) =
            kworld::with_world(|w| (w.windows, w.windows_n, w.log, w.log_n, w.charge[0]));
        assert_eq!(charge, 1, "the pool paid the cost");
        assert!(
            spec::fire_spend_windows_ok(&wins[..wins_n], 0, 2, "r:bandori:78"),
            "raise over the whole cost, then markerSpend -n"
        );
        assert_eq!(wins[0].kind, WINDOW_RAISE);
        assert_eq!(wins[1].kind, WINDOW_SPEND);
        // The bare host arm writes silently; the engine wrapper
        // (`wasm_rules::spend_marker`) posts the 效果适用 `AddToken` line. The
        // Kani dispatcher mirrors that with `fire_spend_logged`; the native
        // bridge goes straight to the arm, so the log stays empty here.
        assert_eq!(log_n, 0, "bare host arm posts no effect line");
        uninstall();
    });
}

/// `spend` of a short pool still opens both windows and refuses (returns
/// false, pot unchanged).
#[test]
#[ignore = "opt-in: kani conformance suite; run with `-- --ignored`"]
fn spend_short_pool_opens_windows_then_refuses() {
    big(|| {
        let _g = model_lock();
        install(fire_world(0, 1, 5));
        assert!(!bandori_std::fire::spend(0, 3, &Msg::new("proof")).unwrap());
        let (wins, wins_n, charge) =
            kworld::with_world(|w| (w.windows, w.windows_n, w.charge[0]));
        assert_eq!(charge, 1, "an all-or-nothing spend moves nothing");
        assert_eq!(wins_n, 2, "raise + markerSpend opened before the write");
        assert!(wins[1].value == -3, "the spend window carries -n");
        uninstall();
    });
}

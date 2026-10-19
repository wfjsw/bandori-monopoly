//! The [`CrossState`] primitives every process shares: locks (with TTL),
//! presence keys (with TTL), pub/sub, and the record-log bookkeeping.
//!
//! Run against the in-memory dummy store. If a `redis-server` binary is
//! available, the same suite also runs against a throwaway instance (see
//! [`redis_live`]).

use std::sync::Arc;
use std::time::Duration;

use server::store::{self, dummy, CrossState, RecMeta};

/// A shared dummy store is what two `Server`s hold when they share state.
fn dummy() -> dummy::Store {
    dummy::Store::new()
}

#[test]
fn lock_acquire_is_exclusive_until_released() {
    let s = dummy();
    assert!(s.lock_acquire("match:R", "a", 10_000).unwrap());
    assert!(!s.lock_acquire("match:R", "b", 10_000).unwrap(), "held");
    assert!(s.lock_renew("match:R", "a", 10_000).unwrap(), "owner renews");
    assert!(!s.lock_renew("match:R", "b", 10_000).unwrap(), "not the owner");
    s.lock_release("match:R", "b").unwrap();
    assert!(!s.lock_acquire("match:R", "b", 10_000).unwrap(), "stale release is a no-op");
    s.lock_release("match:R", "a").unwrap();
    assert!(s.lock_acquire("match:R", "b", 10_000).unwrap(), "free after release");
}

#[test]
fn lock_expires_on_its_ttl() {
    let s = dummy();
    assert!(s.lock_acquire("tick:R", "a", 30).unwrap());
    assert!(!s.lock_acquire("tick:R", "b", 30).unwrap());
    std::thread::sleep(Duration::from_millis(60));
    assert!(s.lock_acquire("tick:R", "b", 30).unwrap(), "TTL freed the lock");
    // Renewing an expired lock fails -- it may already belong to somebody else.
    assert!(!s.lock_renew("tick:R", "a", 30).unwrap());
}

#[test]
fn with_lock_runs_only_under_the_lock_and_releases() {
    let s = dummy();
    let a = store::with_lock(&s, "room:R", "a", 10_000, Duration::from_millis(200), || {
        assert!(
            !s.lock_acquire("room:R", "b", 10_000).unwrap(),
            "held for the body"
        );
        Ok(42)
    })
    .unwrap();
    assert_eq!(a, 42);
    assert!(s.lock_acquire("room:R", "b", 10_000).unwrap(), "released after");
}

#[test]
fn with_lock_fails_rather_than_running_unlocked() {
    let s = dummy();
    assert!(s.lock_acquire("room:R", "other", 10_000).unwrap());
    let out: Result<(), String> =
        store::with_lock(&s, "room:R", "me", 10_000, Duration::from_millis(80), || {
            panic!("must not run unlocked");
        });
    assert!(out.is_err(), "lock failure is an error, never a fallthrough");
}

#[test]
fn presence_keys_expire_and_count() {
    let s = dummy();
    s.presence_set("R", 1, "idle", 200).unwrap();
    s.presence_set("R", 1, "s1", 200).unwrap();
    s.presence_set("R", 1, "s2", 200).unwrap();
    s.presence_set("R", 2, "s1", 200).unwrap();
    assert_eq!(s.presence_count("R", 1).unwrap(), 3);
    assert_eq!(s.presence_count("R", 2).unwrap(), 1);
    s.presence_del("R", 1, "s1").unwrap();
    assert_eq!(s.presence_count("R", 1).unwrap(), 2);
    s.presence_clear("R", 1).unwrap();
    assert_eq!(s.presence_count("R", 1).unwrap(), 0);
    assert_eq!(s.presence_count("R", 2).unwrap(), 1, "other members untouched");
    // TTL: the last key of member 2 goes away on its own.
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(s.presence_count("R", 2).unwrap(), 0, "expired");
}

#[test]
fn presence_set_refreshes_a_live_key() {
    let s = dummy();
    s.presence_set("R", 1, "s", 80).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    s.presence_set("R", 1, "s", 80).unwrap(); // keep-alive
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(s.presence_count("R", 1).unwrap(), 1, "refreshed, still live");
    std::thread::sleep(Duration::from_millis(120));
    assert_eq!(s.presence_count("R", 1).unwrap(), 0, "and expires after");
}

#[test]
fn publish_reaches_every_subscriber() {
    let s = dummy();
    let a = s.subscribe("notes").unwrap();
    let b = s.subscribe("notes").unwrap();
    s.publish("notes", "hello").unwrap();
    let ma = a.try_recv().unwrap();
    let mb = b.try_recv().unwrap();
    assert_eq!(ma.payload, "hello");
    assert_eq!(mb.channel, "notes");
    assert_eq!(mb.payload, "hello");
    // A dropped receiver is pruned, not a panic.
    drop(a);
    s.publish("notes", "again").unwrap();
    assert_eq!(b.try_recv().unwrap().payload, "again");
}

#[test]
fn rec_meta_round_trips() {
    let s = dummy();
    assert!(s.rec_meta_get("R").unwrap().is_none());
    let meta = RecMeta {
        buf: Some((2, 7)),
        inputs: 11,
        ticks: 130,
        last_ckpt_at: 3,
        gaps: true,
        members: vec![1, 2],
        done: false,
        dirty: true,
    };
    s.rec_meta_put("R", &meta).unwrap();
    assert_eq!(s.rec_meta_get("R").unwrap().unwrap(), meta);
    s.rec_meta_del("R").unwrap();
    assert!(s.rec_meta_get("R").unwrap().is_none());
}

#[test]
fn markers_are_ownerless_ttl_stamps() {
    let s = dummy();
    assert!(!s.marker_live("botwait:R:1").unwrap());
    s.marker_set("botwait:R:1", 40).unwrap();
    assert!(s.marker_live("botwait:R:1").unwrap());
    s.marker_set("botwait:R:1", 40).unwrap(); // refresh
    std::thread::sleep(Duration::from_millis(25));
    assert!(s.marker_live("botwait:R:1").unwrap(), "refreshed");
    std::thread::sleep(Duration::from_millis(40));
    assert!(!s.marker_live("botwait:R:1").unwrap(), "expired");
}

/// Two processes claiming the same bot seat: exactly one wins, and the
/// cooldown after a release spaces the next probe.
#[test]
fn bot_seat_claims_are_cross_process() {
    use server::botsvc::{claim_seat, release_seat};
    let s = dummy();
    assert!(claim_seat(&s, "a", "R", 1), "first claim");
    assert!(!claim_seat(&s, "b", "R", 1), "second process");
    assert!(claim_seat(&s, "b", "R", 2), "other seats are independent");
    release_seat(&s, "a", "R", 1, false);
    assert!(!claim_seat(&s, "b", "R", 1), "cooldown after release");
    std::thread::sleep(Duration::from_millis(250));
    assert!(claim_seat(&s, "b", "R", 1), "cooldown expired");
}

/// Two handles over the same dummy state -- what two `Server`s share.
#[test]
fn dummy_store_is_shareable_across_handles() {
    let a = dummy();
    let b = a.clone();
    let info = game_core::net::RoomInfo {
        id: "R1".into(),
        name: "shareable".into(),
        ranked: false,
        max_players: 6,
        locked: false,
        playing: false,
        theme: String::new(),
        weights: Default::default(),
        clock: Default::default(),
        members: vec![],
        fair: None,
    };
    a.room_put(&server::store::RoomRecord {
        info,
        password: String::new(),
        next_member: 1,
        tokens: vec![],
        fair: None,
    })
    .unwrap();
    assert_eq!(b.room_get("R1").unwrap().unwrap().info.id, "R1");
    assert!(b.lock_acquire("k", "b", 1_000).unwrap());
    assert!(!a.lock_acquire("k", "a", 1_000).unwrap(), "same lock table");
}

// ---------------------------------------------------------------- redis live

/// The same primitives against a real Redis, when one can be started. Skipped
/// (loudly) when `redis-server` is not on the PATH -- the Redis impl then remains
/// untested live and only its code shape is reviewed.
#[test]
fn redis_live_primitives() {
    use server::store::redis;
    use std::process::{Child, Command};
    use std::time::Instant;

    let Some(exe) = which("redis-server") else {
        eprintln!("redis-server not on PATH -- Redis store untested live");
        return;
    };
    let port = 20_000 + (std::process::id() % 10_000) as u16;
    let mut child: Child = Command::new(exe)
        .arg("--port")
        .arg(port.to_string())
        .arg("--save")
        .arg("")
        .arg("--appendonly")
        .arg("no")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn redis-server");
    let url = format!("redis://127.0.0.1:{port}");
    // Wait for the port to answer.
    let deadline = Instant::now() + Duration::from_secs(5);
    let store = loop {
        if let Ok(s) = redis::Store::connect(&url) {
            break Arc::new(s);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("redis-server did not come up on {port}");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let s: &dyn CrossState = store.as_ref();

    assert!(s.lock_acquire("match:R", "a", 5_000).unwrap());
    assert!(!s.lock_acquire("match:R", "b", 5_000).unwrap());
    assert!(s.lock_renew("match:R", "a", 5_000).unwrap());
    assert!(!s.lock_renew("match:R", "b", 5_000).unwrap());
    s.lock_release("match:R", "b").unwrap();
    assert!(!s.lock_acquire("match:R", "b", 5_000).unwrap(), "stale release");
    s.lock_release("match:R", "a").unwrap();
    assert!(s.lock_acquire("match:R", "b", 5_000).unwrap());

    s.presence_set("R", 1, "s1", 120).unwrap();
    s.presence_set("R", 1, "idle", 120).unwrap();
    assert_eq!(s.presence_count("R", 1).unwrap(), 2);
    s.presence_del("R", 1, "s1").unwrap();
    assert_eq!(s.presence_count("R", 1).unwrap(), 1);
    s.presence_clear("R", 1).unwrap();
    assert_eq!(s.presence_count("R", 1).unwrap(), 0);

    let rx = s.subscribe("notes").unwrap();
    s.publish("notes", "ping").unwrap();
    let got = rx.recv_timeout(Duration::from_secs(2)).expect("pub/sub round-trip");
    assert_eq!(got.payload, "ping");

    let meta = RecMeta {
        buf: Some((1, 2)),
        inputs: 3,
        ticks: 4,
        last_ckpt_at: 1,
        gaps: false,
        members: vec![1],
        done: false,
        dirty: false,
    };
    s.rec_meta_put("R", &meta).unwrap();
    assert_eq!(s.rec_meta_get("R").unwrap().unwrap(), meta);

    let _ = child.kill();
    let _ = child.wait();
}

fn which(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let p = dir.join(name);
            p.is_file().then_some(p)
        })
    })
}
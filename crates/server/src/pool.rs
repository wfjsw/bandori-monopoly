//! A warm pool of `rules-worker` processes.
//!
//! Workers are long-running and hold no match state between requests -- every
//! request carries the whole match -- so any worker can serve any room, there is
//! nothing to synchronise, and N processes give N-way parallelism. The pool
//! keeps them warm, hands each call to one, and restarts one that dies or
//! overruns its deadline.
//!
//! [`Pool::call`] is **blocking**. On the async runtime that means going through
//! `tokio::task::spawn_blocking` (see `state.rs`: critical sections are short and
//! never await). The deadline is what keeps a wedged worker from holding a room
//! lock forever -- it is killed and replaced instead.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use game_core::data::GameData;
use game_core::engine::CardRules;
use game_core::msg::Msg;
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use serde_json::{json, Value};

/// How long one round-trip may take before the worker is treated as wedged.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// One worker process plus the reader thread that owns its stdout.
struct Worker {
    child: Child,
    stdin: ChildStdin,
    /// Lines from the reader thread. The sender drops on exit, which is how a
    /// dead worker is noticed (`RecvTimeoutError::Disconnected`).
    rx: Receiver<String>,
}

impl Worker {
    fn spawn(
        exe: &std::path::Path,
        data: &std::path::Path,
        rules: &std::path::Path,
    ) -> Result<Self, String> {
        let mut child = Command::new(exe)
            .arg("--data")
            .arg(data)
            .arg("--rules")
            .arg(rules)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn {}: {e}", exe.display()))?;
        let stdin = child.stdin.take().ok_or("worker has no stdin")?;
        let stdout = child.stdout.take().ok_or("worker has no stdout")?;
        let (tx, rx) = mpsc::channel();
        // The reader thread lives as long as the worker; it exits on EOF, which
        // drops `tx` and closes the channel.
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(l) => {
                        if tx.send(l).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self { child, stdin, rx })
    }

    /// Write one request, read one response. Errors mean the worker is unusable.
    fn roundtrip(&mut self, req: &Value, timeout: Duration) -> Result<Value, String> {
        let line = serde_json::to_string(req).map_err(|e| e.to_string())?;
        writeln!(self.stdin, "{line}").map_err(|e| format!("write: {e}"))?;
        self.stdin.flush().map_err(|e| format!("flush: {e}"))?;
        let resp = self.rx.recv_timeout(timeout).map_err(|e| match e {
            RecvTimeoutError::Timeout => format!("worker did not answer within {timeout:?}"),
            RecvTimeoutError::Disconnected => "worker exited".to_string(),
        })?;
        serde_json::from_str(&resp).map_err(|e| format!("bad worker response: {e}"))
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// N warm workers, shared across rooms.
pub struct Pool {
    workers: Vec<Mutex<Option<Worker>>>,
    next: AtomicUsize,
    exe: PathBuf,
    data: PathBuf,
    rules: PathBuf,
    timeout: Duration,
    /// When set, requests run in this process through the same protocol the
    /// worker binary speaks. That is the tests, and a fallback when no worker
    /// binary is installed -- not the production path.
    local: Option<rules_worker::Ctx>,
}

impl Pool {
    /// Start `n` workers. `exe` is the `rules-worker` binary; `data`/`rules` are
    /// forwarded to it (they are the immutable game tables it loads once).
    pub fn start(n: usize, exe: PathBuf, data: PathBuf, rules: PathBuf) -> Arc<Self> {
        let n = n.max(1);
        let pool = Arc::new(Self {
            workers: (0..n).map(|_| Mutex::new(None)).collect(),
            next: AtomicUsize::new(0),
            exe,
            data,
            rules,
            timeout: DEFAULT_TIMEOUT,
            local: None,
        });
        for slot in &pool.workers {
            *slot.lock().unwrap() = Worker::spawn(&pool.exe, &pool.data, &pool.rules).ok();
        }
        let live = pool
            .workers
            .iter()
            .filter(|w| w.lock().unwrap().is_some())
            .count();
        eprintln!("rules pool: {live}/{n} workers from {}", pool.exe.display());
        if live == 0 {
            eprintln!("WARNING: no rule workers started -- matches cannot run");
        }
        pool
    }

    /// The `rules-worker` binary next to this executable, or `BM_WORKER`.
    pub fn default_exe() -> PathBuf {
        if let Ok(p) = std::env::var("BM_WORKER") {
            return PathBuf::from(p);
        }
        let mut p = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        p.pop();
        p.push(if cfg!(windows) {
            "rules-worker.exe"
        } else {
            "rules-worker"
        });
        p
    }

    /// The engine in this process -- no child workers. Tests and the fallback
    /// for a missing `rules-worker` binary use this; production uses [`Pool::start`].
    pub fn in_process(data: Arc<GameData>, rules: Arc<dyn CardRules>) -> Arc<Self> {
        Arc::new(Self {
            workers: vec![],
            next: AtomicUsize::new(0),
            exe: PathBuf::new(),
            data: PathBuf::new(),
            rules: PathBuf::new(),
            timeout: DEFAULT_TIMEOUT,
            local: Some(rules_worker::Ctx::new(data, rules)),
        })
    }

    /// One round-trip. Blocking; see the module docs for why that is deliberate.
    pub fn call(&self, req: Value) -> Result<Value, String> {
        if let Some(ctx) = &self.local {
            let v = rules_worker::handle(ctx, req);
            return if v.get("ok").and_then(Value::as_bool) == Some(false) {
                Err(v
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("worker error")
                    .to_string())
            } else {
                Ok(v)
            };
        }
        let i = self.next.fetch_add(1, Ordering::Relaxed) % self.workers.len();
        let mut slot = self.workers[i].lock().unwrap();
        let out = match slot.as_mut() {
            Some(w) => w.roundtrip(&req, self.timeout),
            None => Err("worker not running".to_string()),
        };
        match out {
            Ok(v) => {
                if v.get("ok").and_then(Value::as_bool) == Some(false) {
                    Err(v
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("worker error")
                        .to_string())
                } else {
                    Ok(v)
                }
            }
            Err(e) => {
                // Dead or wedged: throw it away and bring up a fresh one. The
                // caller retries; the match blob it holds is still valid.
                if let Some(w) = slot.as_mut() {
                    w.kill();
                }
                *slot = Worker::spawn(&self.exe, &self.data, &self.rules).ok();
                Err(e)
            }
        }
    }

    // -------------------------------------------------- typed operations
    //
    // Each is one request. `state` is `Match::save`; the `String` in the Ok is
    // the new blob the caller must store.

    pub fn new_match(
        &self,
        members: &[RoomMember],
        seed: u64,
        mode: i32,
        weights: &ScoreWeights,
    ) -> Result<String, String> {
        let v = self.call(json!({"op": "new", "members": members, "seed": seed, "mode": mode, "weights": weights}))?;
        v.get("state")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "worker returned no state".to_string())
    }

    /// `(new state, error message if the command was rejected)`. The state is
    /// returned either way: a rejected command may still have moved the match.
    pub fn act(
        &self,
        state: &str,
        member: i32,
        cmd: &NetMessage,
    ) -> Result<(String, Option<Msg>), String> {
        let v = self.call(json!({"op": "act", "state": state, "member": member, "cmd": cmd}))?;
        let new = v
            .get("state")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "worker returned no state".to_string())?;
        let err = match v.get("error") {
            None | Some(Value::Null) => None,
            Some(e) => Some(serde_json::from_value(e.clone()).map_err(|e| e.to_string())?),
        };
        Ok((new, err))
    }

    pub fn tick(&self, state: &str, dt: f32) -> Result<String, String> {
        let v = self.call(json!({"op": "tick", "state": state, "dt": dt}))?;
        v.get("state")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "worker returned no state".to_string())
    }

    pub fn quick_start(&self, state: &str) -> Result<String, String> {
        let v = self.call(json!({"op": "quick_start", "state": state}))?;
        v.get("state")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "worker returned no state".to_string())
    }

    pub fn finish(&self, state: &str) -> Result<String, String> {
        let v = self.call(json!({"op": "finish", "state": state}))?;
        v.get("state")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "worker returned no state".to_string())
    }

    pub fn member_left(
        &self,
        state: &str,
        member: i32,
        can_return: bool,
    ) -> Result<String, String> {
        let v = self.call(
            json!({"op": "member_left", "state": state, "member": member, "canReturn": can_return}),
        )?;
        v.get("state")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "worker returned no state".to_string())
    }

    pub fn member_back(&self, state: &str, member: i32) -> Result<String, String> {
        let v = self.call(json!({"op": "member_back", "state": state, "member": member}))?;
        v.get("state")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "worker returned no state".to_string())
    }

    pub fn view(&self, state: &str, member: i32) -> Result<Value, String> {
        let v = self.call(json!({"op": "view", "state": state, "member": member}))?;
        v.get("view")
            .cloned()
            .ok_or_else(|| "worker returned no view".to_string())
    }

    pub fn events(&self, state: &str, since: i32) -> Result<Vec<Value>, String> {
        let v = self.call(json!({"op": "events", "state": state, "since": since}))?;
        Ok(v.get("events")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    pub fn ended(&self, state: &str) -> Result<bool, String> {
        Ok(self
            .call(json!({"op": "ended", "state": state}))?
            .get("ended")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    pub fn changed(&self, state: &str) -> Result<bool, String> {
        Ok(self
            .call(json!({"op": "changed", "state": state}))?
            .get("changed")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }
}

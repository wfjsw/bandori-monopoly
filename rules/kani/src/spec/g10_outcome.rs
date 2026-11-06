//! G10 outcome models (`docs/rulebook/spec/abi/G10-effect-outcome.md` §6.1,
//! §6.2, §6.3, §6.5, §10.2).
//!
//! Card bodies stay PROTOCOL-LEVEL. What is modelled here is the decision each
//! host step reads:
//!
//! * [`settle_facts`] — §6.1's bit derivation: `cancelled` excludes a replaced
//!   body, and both exclude a paid one.
//! * [`paid_fact`] — §6.1's `paid` predicate: only a rent payment to the
//!   settle's owner for the settle's own tile.
//! * [`redirect_fact`] — §6.4's landing predicate: live, not the declared seat,
//!   not the actor.
//! * [`cause_on_event`] / [`cause_on_event_after`] — §6.2's propagation of the
//!   cause from a deferred record to both reads.
//! * [`defer`] — §6.2's once-per-`(event, player)` claim, with an owner-bound
//!   re-run.
//! * [`due`] — §6.2's release point (the due owner's later turn start, or the
//!   first turn start after that owner left).
//! * [`Store`] — §6.5's snapshot store: allocate, absolute idempotent restore,
//!   owner-scoped drop.
//! * [`JLog`] — §6.3.4's answer layout: an inserted answer takes its own slot
//!   and shifts every still-in-flight entry past it.
//! * [`reencounter`] — §6.3.5's re-encounter: an equal one applies nothing, a
//!   changed one restores `condition` first, and the cap is a clean skip.

/// `SettleFacts::cancelled` bit.
pub const CANCELLED: i32 = 1;
/// `SettleFacts::body_replaced` bit.
pub const BODY_REPLACED: i32 = 2;
/// `SettleFacts::paid` bit.
pub const PAID: i32 = 4;
/// The replace cap of §6.3.5.
pub const REPLACE_CAP: u8 = 4;

/// §6.1's facts bitset. `cancel_link` is the settle link's negation; a replaced
/// body is not also `cancelled`, and a paid settle ran its body.
pub fn settle_facts(cancel_link: bool, body_replaced: bool, paid: bool) -> i32 {
    let cancelled = cancel_link && !body_replaced;
    (cancelled as i32) * CANCELLED + (body_replaced as i32) * BODY_REPLACED + (paid as i32) * PAID
}

/// §6.1's `paid` predicate: a rent payment for the settle's own tile to the
/// settle's owner.
pub fn paid_fact(is_rent: bool, pay_tile: i32, settle_tile: i32, to: i32, owner: i32) -> bool {
    is_rent && settle_tile >= 0 && pay_tile == settle_tile && to == owner
}

/// §6.4's landing predicate, answering the redirect origin (`-1` = none).
pub fn redirect_fact(live: bool, to: i32, declared: i32, actor: i32) -> i32 {
    if live && to != declared && to != actor {
        declared
    } else {
        -1
    }
}

/// §6.2: the `event` trigger's cause is the record's own cause instance.
pub fn cause_on_event(record_cause: i32) -> i32 {
    record_cause
}

/// §6.2: the `eventAfter` read of the same record copies the same cause.
pub fn cause_on_event_after(record_cause: i32) -> i32 {
    record_cause
}

const MAX_DEFERRED: usize = 2;

/// One parked event record.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Rec {
    pub event: i32,
    pub player: i32,
    pub due_owner: i32,
    pub deferrer_uid: i32,
    pub after_turn_key: i32,
}

/// §6.2's claim. `1` = the running instance owns the record (a fresh pair
/// pushes one); `0` = someone else already has it or the store is full. A
/// re-run of the same instance answers `1` and pushes nothing.
pub fn defer(
    recs: &mut [Rec; MAX_DEFERRED],
    n: &mut usize,
    event: i32,
    player: i32,
    uid: i32,
    due_owner: i32,
    turn_key: i32,
) -> i64 {
    for r in recs.iter().take(*n) {
        if r.event == event && r.player == player {
            return (r.deferrer_uid == uid) as i64;
        }
    }
    if *n >= MAX_DEFERRED {
        return 0;
    }
    recs[*n] = Rec {
        event,
        player,
        due_owner,
        deferrer_uid: uid,
        after_turn_key: turn_key,
    };
    *n += 1;
    1
}

/// §6.2's release point: the due owner's turn start strictly after the record,
/// or the first turn start once that owner left.
pub fn due(r: &Rec, owner: i32, turn_key: i32, owner_out: bool) -> bool {
    owner_out || (r.due_owner == owner && r.after_turn_key < turn_key)
}

/// The six checkpointed families (§6.5). One seat and two tiles are enough for
/// the algebra; the store's own length limits are a separate concern.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Snap {
    pub pos: i32,
    pub money: i32,
    pub state: i32,
    pub owners: [i32; 2],
    pub houses: [i32; 2],
    pub mortgaged: [bool; 2],
}

/// A live world of the same shape.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct World {
    pub pos: i32,
    pub money: i32,
    pub state: i32,
    pub owners: [i32; 2],
    pub houses: [i32; 2],
    pub mortgaged: [bool; 2],
}

impl World {
    pub fn snap(&self) -> Snap {
        Snap {
            pos: self.pos,
            money: self.money,
            state: self.state,
            owners: self.owners,
            houses: self.houses,
            mortgaged: self.mortgaged,
        }
    }
    pub fn restore(&mut self, s: &Snap) {
        self.pos = s.pos;
        self.money = s.money;
        self.state = s.state;
        self.owners = s.owners;
        self.houses = s.houses;
        self.mortgaged = s.mortgaged;
    }
}

/// One stored key: its snap, its owners and the owner count.
type Key = (i32, Snap, [i32; 2], u8);

/// §6.5's store. Keys start at 1 (`0` = none); the last owner frees the key.
pub struct Store {
    keys: [Option<Key>; 3],
    next_key: i32,
}

impl Store {
    pub fn new() -> Self {
        Store {
            keys: [None; 3],
            next_key: 1,
        }
    }
    /// Allocate under the running instance (`uid`). `0` when full.
    pub fn snapshot(&mut self, w: &World, uid: i32) -> i32 {
        let Some(slot) = self.keys.iter().position(|k| k.is_none()) else {
            return 0;
        };
        let key = self.next_key.max(1);
        self.next_key = key + 1;
        let mut owners = [-1; 2];
        owners[0] = uid;
        self.keys[slot] = Some((key, w.snap(), owners, 1));
        key
    }
    fn slot(&self, key: i32) -> Option<usize> {
        self.keys
            .iter()
            .position(|k| k.as_ref().is_some_and(|k| k.0 == key))
    }
    /// Absolute, idempotent write-back; `1` on a key `uid` owns, `0` otherwise.
    pub fn restore(&mut self, key: i32, w: &mut World, uid: i32) -> i32 {
        let Some(i) = self.slot(key) else { return 0 };
        let Some((_, snap, owners, owners_n)) = self.keys[i] else {
            return 0;
        };
        if !owners[..owners_n as usize].contains(&uid) {
            return 0;
        }
        w.restore(&snap);
        1
    }
    /// Forget `uid`'s ownership; the last owner frees the key.
    pub fn drop_key(&mut self, key: i32, uid: i32) {
        let Some(i) = self.slot(key) else { return };
        let Some((_, _, mut owners, mut owners_n)) = self.keys[i] else {
            return;
        };
        let mut kept = 0u8;
        for o in 0..owners_n as usize {
            if owners[o] != uid {
                owners[kept as usize] = owners[o];
                kept += 1;
            }
        }
        owners_n = kept;
        if kept == 0 {
            self.keys[i] = None;
        } else {
            self.keys[i] = Some((key, self.keys[i].unwrap().1, owners, owners_n));
        }
    }
}

/// Room in [`JLog`]. Fixed arrays keep CBMC off the `Vec` allocator: the
/// designed layout is at most a handful of entries deep.
pub const JLOG_CAP: usize = 4;

/// §6.3.4's answer layout: in-flight entry slots and the answer prefix.
pub struct JLog {
    /// Live answers (the completed prefix).
    pub answers: [i32; JLOG_CAP],
    /// `answers[..answers_n]` are live.
    pub answers_n: usize,
    /// In-flight entry slots, in request order. The invariant on entry is
    /// every slot `>= answers_n` -- requests land where the cursor is.
    pub journal: [usize; JLOG_CAP],
    /// `journal[..journal_n]` are live.
    pub journal_n: usize,
}

impl Default for JLog {
    fn default() -> Self {
        JLog {
            answers: [0; JLOG_CAP],
            answers_n: 0,
            journal: [0; JLOG_CAP],
            journal_n: 0,
        }
    }
}

/// Answer the in-flight entry at `index` with `answer` (§6.3.4). The entry
/// takes its own slot; every entry still in flight at or after it moves past.
pub fn push_answer(l: &mut JLog, index: usize, answer: i32) {
    let slot = l.journal[index];
    // Drop the answered entry.
    let mut i = index;
    while i + 1 < l.journal_n {
        l.journal[i] = l.journal[i + 1];
        i += 1;
    }
    l.journal_n -= 1;
    // Every entry still in flight at or after the answered slot moves past it.
    let mut i = 0;
    while i < l.journal_n {
        if l.journal[i] >= slot {
            l.journal[i] += 1;
        }
        i += 1;
    }
    // The answer takes its own place: the cursor, or the answered slot when
    // that already sits deeper.
    let at = slot.min(l.answers_n);
    let mut i = l.answers_n;
    while i > at {
        l.answers[i] = l.answers[i - 1];
        i -= 1;
    }
    l.answers[at] = answer;
    l.answers_n += 1;
}

/// §6.3.5's re-encounter. `same` = the recomputed request equals the completed
/// row's. `condition` is that row's pre-state, `applied_world` the world its
/// application produced, `overlay` the guest overlay the request runs under.
/// Answers `(further_applies, world)`.
pub fn reencounter(
    same: bool,
    replaces_so_far: u8,
    condition: i32,
    applied_world: i32,
    overlay: i32,
) -> (u8, i32) {
    if same {
        // Consumed at its own cursor; nothing re-runs.
        return (0, applied_world);
    }
    if replaces_so_far >= REPLACE_CAP {
        // The cap: restore the last completed application, a clean skip.
        return (0, condition);
    }
    // The re-application starts from the row's pre-state, never from the
    // half-applied world the body recomputed past.
    (1, condition + overlay)
}

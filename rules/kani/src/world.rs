//! Small symbolic world model for the Kani pilot.
//!
//! Deliberately tiny and fixed-capacity so CBMC can unwind every loop.
//! Game vocabulary lives here (this crate is game-specific), not in card-sdk.


pub const MAX_PLAYERS: usize = 4;
pub const MAX_LOG: usize = 24;
pub const MAX_CARDS: usize = 4;
pub const MAX_NARRATION: usize = 8;
pub const MAX_ANSWERS: usize = 8;

/// Interned card-id strings the model knows about. Bounded so we never
/// allocate a `String` inside a proof of the pick loop.
pub const CARD_AYA: &str = "PP:[丸山彩]憧憬的前方";
pub const CARD_RESONANCE: &str = "PP:[衍生]共鸣";
pub const CARD_OTHER: &str = "PP:[标题偶像]";
pub const CARD_EXTRA: &str = "PP:[强韧之花]";

pub fn card_id_str(id: u8) -> &'static str {
    match id {
        0 => CARD_AYA,
        1 => CARD_RESONANCE,
        2 => CARD_OTHER,
        3 => CARD_EXTRA,
        _ => CARD_OTHER,
    }
}

/// Token names the money-shave clause reads.
pub const TOK_PP_POS: &str = "P✽P粉丝(正)";
pub const TOK_PP_NEG: &str = "P✽P粉丝(反)";

/// The one board tile the teleport pilot resolves against.
pub const TILE_SHOP_NAME: &str = "江户川乐器店";
pub const TILE_SHOP: i32 = 3;
pub const TILE_COUNT: i32 = 8;

/// A requested game-state change. Narration (`ctx::log`) is recorded
/// separately -- the book's frame is about effects, not log lines.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effect {
    SetPayAmount { to: i32 },
    Teleport { player: i32, tile: i32 },
    GainFire { player: i32, n: i32 },
    GiveStay { player: i32, n: i32 },
    PlaceCard { player: i32, dest: i32, card: u8 },
    TakeFromPile { player: i32, pile: i32, card: u8 },
    AddToHand { player: i32, card: u8 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Narration {
    pub player: i32,
    pub key: u16,
}

/// Prompt request kinds (mirrors `card_sdk::abi::PromptKind` discriminants).
pub const PROMPT_CARD: i32 = 4;
pub const PROMPT_YESNO: i32 = 1;

pub struct World {
    pub n_players: i32,
    pub money: [i32; MAX_PLAYERS],
    pub out: [bool; MAX_PLAYERS],
    pub pos: [i32; MAX_PLAYERS],
    pub fire: [i32; MAX_PLAYERS],
    pub stay: [i32; MAX_PLAYERS],
    /// `P✽P粉丝(正)` / `(反)` counts.
    pub fans_pos: [i32; MAX_PLAYERS],
    pub fans_neg: [i32; MAX_PLAYERS],
    /// Copies of `PP:[衍生]共鸣` in hand (the only hand identity we need).
    pub hand_resonance: [i32; MAX_PLAYERS],
    /// Discard pile as a bitset over `MAX_CARDS` card ids (presence, not multiplicity).
    pub discard: [bool; MAX_CARDS],
    /// Hand as a bitset over card ids.
    pub hand: [bool; MAX_CARDS],
    /// Named-token fallback (non-PP names return 0 unless listed here).
    pub tile_shop_present: bool,

    // ---- trigger context (a pending [支付] / [消耗]) ----
    pub trig_player: i32,
    pub trig_value: i32,
    /// Last value written by `trig_set_pay_amount`.
    pub pay_amount: i32,

    // ---- prompt state ----
    pub opt_n: i32,
    pub next_answer: usize,
    pub answers: [i32; MAX_ANSWERS],
    /// When true, `ask` returns a fresh symbolic answer in range (Kani only).
    pub symbolic_answers: bool,

    // ---- effect / narration logs ----
    pub log: [Effect; MAX_LOG],
    pub log_n: usize,
    pub narration: [Narration; MAX_NARRATION],
    pub narration_n: usize,

    pub dest: i32,
    /// Scratch: last place_card uid.
    pub last_uid: i32,
}

impl World {
    pub fn new() -> Self {
        Self {
            n_players: 2,
            money: [0; MAX_PLAYERS],
            out: [false; MAX_PLAYERS],
            pos: [0; MAX_PLAYERS],
            fire: [0; MAX_PLAYERS],
            stay: [0; MAX_PLAYERS],
            fans_pos: [0; MAX_PLAYERS],
            fans_neg: [0; MAX_PLAYERS],
            hand_resonance: [0; MAX_PLAYERS],
            discard: [false; MAX_CARDS],
            hand: [false; MAX_CARDS],
            tile_shop_present: true,
            trig_player: 0,
            trig_value: 0,
            pay_amount: 0,
            opt_n: 0,
            next_answer: 0,
            answers: [0; MAX_ANSWERS],
            symbolic_answers: false,
            log: [Effect::SetPayAmount { to: 0 }; MAX_LOG],
            log_n: 0,
            narration: [Narration { player: 0, key: 0 }; MAX_NARRATION],
            narration_n: 0,
            dest: 0,
            last_uid: 1,
        }
    }

    pub fn push(&mut self, e: Effect) {
        if self.log_n < MAX_LOG {
            self.log[self.log_n] = e;
            self.log_n += 1;
        }
    }

    pub fn push_narration(&mut self, player: i32, key: u16) {
        if self.narration_n < MAX_NARRATION {
            self.narration[self.narration_n] = Narration { player, key };
            self.narration_n += 1;
        }
    }

    pub fn effect_slice(&self) -> &[Effect] {
        &self.log[..self.log_n]
    }

    /// Living players in `0..n_players` that are not `out`.
    pub fn living(&self, n: i32) -> impl Iterator<Item = i32> + '_ {
        let n = n.clamp(1, MAX_PLAYERS as i32);
        (0..n).filter(move |&p| !self.out[p as usize])
    }

    pub fn push_answer(&mut self, a: i32) {
        if self.next_answer < MAX_ANSWERS {
            self.answers[self.next_answer] = a;
            self.next_answer += 1;
        }
    }
}

// Single-threaded model. A `static mut` avoids `thread_local!`, which Kani
// only partially models (and which contributed to an ICE in kani-compiler
// 0.68.0 during the pilot).
static mut WORLD: Option<World> = None;

fn world_mut() -> &'static mut World {
    unsafe {
        if WORLD.is_none() {
            WORLD = Some(World::new());
        }
        WORLD.as_mut().unwrap_unchecked()
    }
}

pub fn with_world<R>(f: impl FnOnce(&World) -> R) -> R {
    f(world_mut())
}

pub fn with_world_mut<R>(f: impl FnOnce(&mut World) -> R) -> R {
    f(world_mut())
}

pub fn reset_world(w: World) {
    *world_mut() = w;
}

/// Snapshot for pre/post comparison. Effects and narration are the log.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Snapshot {
    pub n_players: i32,
    pub money: [i32; MAX_PLAYERS],
    pub out: [bool; MAX_PLAYERS],
    pub pos: [i32; MAX_PLAYERS],
    pub fire: [i32; MAX_PLAYERS],
    pub stay: [i32; MAX_PLAYERS],
    pub fans_pos: [i32; MAX_PLAYERS],
    pub fans_neg: [i32; MAX_PLAYERS],
    pub hand_resonance: [i32; MAX_PLAYERS],
    pub discard: [bool; MAX_CARDS],
    pub hand: [bool; MAX_CARDS],
    pub trig_player: i32,
    pub trig_value: i32,
    pub pay_amount: i32,
    pub log_n: usize,
}

pub fn snapshot() -> Snapshot {
    with_world(|w| Snapshot {
        n_players: w.n_players,
        money: w.money,
        out: w.out,
        pos: w.pos,
        fire: w.fire,
        stay: w.stay,
        fans_pos: w.fans_pos,
        fans_neg: w.fans_neg,
        hand_resonance: w.hand_resonance,
        discard: w.discard,
        hand: w.hand,
        trig_player: w.trig_player,
        trig_value: w.trig_value,
        pay_amount: w.pay_amount,
        log_n: w.log_n,
    })
}

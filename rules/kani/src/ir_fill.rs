//! Fill a [`rules_cond_ir::IrCtx`] from the **general** symbolic [`world::World`].
//!
//! This is the remaining trusted base of the single-source design: the
//! lowering from engine world → condition context fields. Production fills
//! the same fields via `game_rules::cond_pre::{fill_window, fill_candidate}`;
//! the differential tests pin the *evaluator*, and this module pins the
//! *binding*.
//!
//! ## Per-field map
//!
//! Every `IrCtx` slot is filled from the world, or deliberately left at its
//! documented default / symbolic. See `docs/rulebook/spec/COND-SINGLE-SOURCE.md`
//! for the same table.
//!
//! ### Scalars (`VarId`)
//!
//! | field | source |
//! |---|---|
//! | `kind` | `w.trig_kind` (override via [`TriggerOverlay`]) |
//! | `actor` | `w.trig_player` |
//! | `target` | `w.trig_target` |
//! | `value` | `w.trig_value` (override via overlay) |
//! | `step` | `0` (World has no step; overlay) |
//! | `by` | `-1` = null (no by-card in World; overlay) |
//! | `pay_is_rent` | `false` (World has no rent flag; overlay) |
//! | `roll_source` | `-1` (overlay) |
//! | `abnormal` | overlay (`false` = not the Abnormal window) |
//! | `turn_player` | `w.turn_player` |
//! | `turn_key` | `w.turn_key` |
//! | `plan_fixed_roll` | `w.plan_fixed_roll` (`-1` = null; overlay override) |
//! | `trigger_card` | `w.trig_card` name id (`0` = none; overlay) |
//! | `counter_name` | `w.counter_name` (`0` = none; overlay) |
//! | `tile_id` | `w.trig_tile` |
//! | `tile_owner` | `w.tiles[tile].owner` |
//! | `tile_houses` | `w.tiles[tile].houses` |
//! | `tile_mortgaged` | `w.tiles[tile].mortgaged` as 0/1 |
//! | `tile_price` | `board::TILES[tile].price` |
//! | `move_roll` / `move_kind` | `w.move_snap` (`-1` = null; overlay) |
//! | `move_remaining` | `w.move_snap.remaining` (`0` when no move) |
//! | `move_main` | `w.trig_main` (`Trigger::main`, not `mv.main`) |
//! | `move_dir` | `w.move_snap.dir` (`1` when no move; engine default) |
//! | `effect_count` | `ov.chain_n` (default 0) |
//! | `owner` / `owner_id` | the candidate seat |
//! | `owner_money` | `w.money[owner]` |
//! | `owner_fire` | `w.fire[owner]` |
//! | `owner_crystals` | `w.band_crystals[owner]` |
//! | `owner_hand` | `hand_size` view (resonance + non-slot-1 bits + `hand_n`) |
//! | `owner_pos` | `w.pos[owner]` |
//! | `owner_out` | `w.out[owner]` as 0/1 |
//! | `owner_stay` | `w.stay[owner]` |
//! | `owner_stun` | `w.key_get(owner, STUN)` |
//! | `owner_exile` | `w.key_get(owner, EXILE)` |
//! | `owner_no_hand` | `w.key_get(owner, NO_HAND)` |
//! | `owner_character` | `id_of(resolve(w.character_id[owner]))` (name hash, matching `fill_window`) |
//! | `owner_band` | `id_of(resolve(w.band_id[owner]))` |
//! | `owner_tiles` | count of `w.tiles[*].owner == owner` |
//! | `card_id` | argument (`id_of(card_name)`) |
//! | `card_placed` | argument |
//! | `card_tile` | `-2` = not placed, else the field-card tile (or `-1` with owner) |
//!
//! ### Per-seat (`SeatField`)
//!
//! Same sources as the `owner_*` scalars, for every seat `0..4`. `Hand` uses
//! the same hand-size view. `Gains` / `Targeted` / `Between` come from
//! `w.gains_this_turn` / `w.targeted_count` / `w.between_seat`:
//! * `gains_this_turn` is incremented by `Op::gain` (money credited).
//! * `targeted_count` is fixture-facing — host status ops do not bump it yet
//!   (set it on the world before filling).
//! * `between_seat` is the last between-witness (`-1` = none).
//!
//! ### Named counters
//!
//! Every `(NameId, i32)` in `w.player_keys[owner]` (tokens / slots / state)
//! is copied via `id_of` of the interned name. The well-known `P✽P粉丝(正)/(反)`
//! keys are also the pilot views `w.fans_pos/neg`. Candidate `card.counter`
//! pairs come from `ov.card_counters` (key = `id_of(name)`). Every board tile
//! name is registered (`tile_named`), matching `fill_window`'s `tile_ids`.
//! Missing names answer 0 (`tok` / `slot` / `card.counter` default) or -1
//! (`tile_named`), matching production.
//!
//! ### Sets
//!
//! | set | source |
//! |---|---|
//! | `Circle` | `board::TILES[i].kind == 0` |
//! | `Ring` | `board::TILES[i].kind == 3` |
//! | `LiveHouse` | buyable ∧ `group == 6` |
//! | `Buyable` | kind ∈ {property=1, ring=3} |
//! | `ChainKinds` / `ChainHits` | `TriggerOverlay.chain` |
//! | `BlockedBands` | `w.blocked_bands[..blocked_bands_n]` (band name ids) |
//!
//! ### Other
//!
//! * `n_players` = `w.n_players`
//! * `tile_count` = 60 (board)
//! * `bin` geometry (`players_on`, `next_dist`, `owned_within`, `on_path`) is
//!   filled sparsely from world positions / owners / `move_snap.roll`
//!   (`BIN_CAP` = 64; unanswered pairs read 0). Symbolic-arg guards need
//!   `conf_fill_diff`-style per-program filling.

use rules_cond_ir::ctx::{FnId, IrCtx, SeatField, SetId, NAMED_CAP};
use rules_cond_ir::vars::{VarId, VarTy, VARS};
use rules_cond_ir::Val;

use crate::board;
use crate::intern::{self, NameId};
use crate::world::{self, World};

// -------------------------------------------------------------------------
// Well-known name ids (must match the host's `id_of`)
// -------------------------------------------------------------------------

pub const TOK_PP_POS: &str = world::TOK_PP_POS;
pub const TOK_PP_NEG: &str = world::TOK_PP_NEG;
/// Player-state keys the host reads as `stun` / `exile` / `noHand`.
pub const KEY_STUN: &str = "stun";
pub const KEY_EXILE: &str = "exile";
pub const KEY_NO_HAND: &str = "noHand";

/// Board tile names (`tile_named('…')`), 0-based slot → spelling from
/// `data/board.json`. Production `fill_window` registers every one so a
/// condition's `tile_named` resolves; this table is the IR-side twin
/// (`World` carries no names). Key in `IrCtx::named` is `id_of(name)`.
pub const TILE_NAMES: [(i32, &str); 60] = [
    (0, "CiRCLE"),
    (1, "购物中心"),
    (2, "天文馆"),
    (3, "水族馆"),
    (4, "主要街道"),
    (5, "偶像经纪公司"),
    (6, "星空齿科"),
    (7, "江户川公园"),
    (8, "RiNG 1"),
    (9, "花咲川女子学院"),
    (10, "羽丘女子学院"),
    (11, "富士见坂"),
    (12, "中心学区"),
    (13, "月之森女子学院"),
    (14, "成为人类桥"),
    (15, "CiRCLE 咖啡厅"),
    (16, "RiNG 2"),
    (17, "周边学区"),
    (18, "白雪学园"),
    (19, "艺术学院高中"),
    (20, "瑟罗希亚国际学校"),
    (21, "DUB MUSIC EXPERIMENT"),
    (22, "加茂川中央中学"),
    (23, "东京外"),
    (24, "大阪中之岛公园"),
    (25, "小豆岛"),
    (26, "微笑号"),
    (27, "米歇尔公园"),
    (28, "弦卷集团"),
    (29, "弦卷豪宅"),
    (30, "江户川乐器店"),
    (31, "Live House"),
    (32, "便利店"),
    (33, "快餐店"),
    (34, "周边精选"),
    (35, "Bandori车站"),
    (36, "RiNG 3"),
    (37, "庆鹏女子大学"),
    (38, "四叶女子大学"),
    (39, "大学路"),
    (40, "武道馆"),
    (41, "梦开始的地方"),
    (42, "Space"),
    (43, "星星小巷"),
    (44, "星之鼓动山丘"),
    (45, "流星堂"),
    (46, "商店街"),
    (47, "羽泽咖啡厅"),
    (48, "山吹面包房"),
    (49, "银河拉面馆"),
    (50, "Live House Galaxy"),
    (51, "北泽精肉店"),
    (52, "旭汤澡堂"),
    (53, "RiNG 4"),
    (54, "六本木大厦"),
    (55, "CHUCHU的公寓"),
    (56, "飞鸟山公园"),
    (57, "旧古河庭园"),
    (58, "广町家画室"),
    (59, "高级住宅区"),
];

/// Optional extras the general `World` does not yet carry. Every field has a
/// documented default that matches `fill_window`'s "absent trigger" reading.
#[derive(Clone, Copy, Debug)]
pub struct TriggerOverlay {
    /// Override `w.trig_kind` when set.
    pub kind: Option<i32>,
    /// Override `w.trig_value` when set (e.g. a chained rewrite).
    pub value: Option<i64>,
    pub step: i64,
    /// `-1` = null.
    pub by: i64,
    pub pay_is_rent: bool,
    /// `-1` = null.
    pub roll_source: i64,
    /// `TriggerKind::Abnormal` window.
    pub abnormal: bool,
    /// `trigger_card` as `id_of(card_name)`; `0` = no card.
    pub trigger_card: i64,
    /// `counter_name` as `id_of(name)`; `0` = no name (Finding F4).
    pub counter_name: i64,
    /// `plan.fixed_roll`; negative = null.
    pub plan_fixed_roll: i64,
    /// `move.*`; `roll`/`kind` negative = null.
    pub move_roll: i64,
    pub move_kind: i64,
    pub move_remaining: i64,
    pub move_main: bool,
    pub move_dir: i64,
    /// Effect chain links `(kind, from, hits)` — `effect.has` / `effect.hits`.
    /// At most 4 (matches a short [支付]/[效果] chain; extend if a fixture needs more).
    pub chain: [(i64, i64, i64); 4],
    pub chain_n: u8,
    /// Candidate `card.counter('name')` pairs, key = `id_of(name)`.
    pub card_counters: [(i64, i64); 4],
    pub card_counters_n: u8,
}

impl Default for TriggerOverlay {
    fn default() -> Self {
        // Hand-rolled so the OptInt defaults are negative (= null), matching
        // `fill_window`'s absent-payload reading.
        Self {
            kind: None,
            value: None,
            step: 0,
            by: -1,
            pay_is_rent: false,
            roll_source: -1,
            abnormal: false,
            trigger_card: 0,
            counter_name: 0,
            plan_fixed_roll: -1,
            move_roll: -1,
            move_kind: -1,
            move_remaining: 0,
            move_main: false,
            move_dir: 1,
            chain: [(0, 0, 0); 4],
            chain_n: 0,
            card_counters: [(0, 0); 4],
            card_counters_n: 0,
        }
    }
}

impl TriggerOverlay {
    /// Append a chain link `(kind, from, hits)`. Extra links beyond 4 are dropped
    /// (bounded overlay; fixtures stay under the cap).
    pub fn push_chain(&mut self, kind: i64, from: i64, hits: i64) {
        let n = self.chain_n as usize;
        if n < 4 {
            self.chain[n] = (kind, from, hits);
            self.chain_n = (n + 1) as u8;
        }
    }

    /// Register a `card.counter('name')` value (key = `id_of(name)`).
    pub fn push_card_counter(&mut self, name: &str, v: i64) {
        let n = self.card_counters_n as usize;
        if n < 4 {
            self.card_counters[n] = (rules_cond_ir::ctx::id_of(name), v);
            self.card_counters_n = (n + 1) as u8;
        }
    }
}

// -------------------------------------------------------------------------
// Pilot guard sources (stable names; ids come from `guard_id!`)
// -------------------------------------------------------------------------

pub mod src {
    /// PP:[丸山彩]憧憬的前方 `On::Hook(PayAdd)`.
    pub const AYA_PAY_ADD: &str = "actor == owner && value > 0";
    /// A function-carrying guard (`is_circle`).
    pub const IS_CIRCLE: &str = "actor == owner && is_circle(tile.id)";
    /// A chain/kind-carrying guard (`effect.has(Pay)`).
    pub const CHAIN_PAY: &str =
        "actor == owner && effect.has(Pay) && target >= 0 && target != owner && value > 0";
}

/// Look up a generated guard by its CEL source (O(n) string scan).
///
/// **Do not call this under Kani** — 139 string compares is a CBMC
/// unwinding / OOM cliff. Use [`crate::guard_id!`] + [`eval_id`] instead.
/// Kept for host-side tests and the consistency check.
pub fn guard_by_src(src: &str) -> &'static rules_cond_ir::Program {
    for g in generated::ALL {
        if g.src == src {
            return &g.program;
        }
    }
    panic!("guard not in generated table; re-run gen_cond_ir");
}

/// Const-id lookup of the program (O(1) index). The Kani path.
pub fn guard_by_id(id: usize) -> &'static rules_cond_ir::Program {
    &generated::BY_ID[id].program
}

pub mod generated {
    pub use crate::generated::cond_irs::{
        eval_id, eval_id_val, guard_id_const, guard_id_opt, GuardIr, ALL, BY_ID, GUARD_COUNT,
    };
}

// -------------------------------------------------------------------------
// Fill
// -------------------------------------------------------------------------

fn hand_size_view(w: &World, p: usize) -> i64 {
    // Mirrors `host.rs` `Op::hand_size`: resonance + non-slot-1 bitset + pile.
    let mut n = w.hand_resonance[p];
    let mut s = 0usize;
    while s < 4 {
        if s != 1 && w.hand[s] {
            n += 1;
        }
        s += 1;
    }
    n as i64 + w.hand_n as i64
}

fn owned_tiles(w: &World, p: i32) -> i64 {
    let mut n = 0i64;
    let mut i = 0usize;
    while i < 60 {
        if w.tiles[i].owner == p {
            n += 1;
        }
        i += 1;
    }
    n
}

fn key_get_name(w: &World, p: i32, name: &str) -> i64 {
    let id = intern::intern(name);
    w.key_get(p, id) as i64
}

/// `id_of` over raw bytes (no UTF-8 validation — that is a Kani unwinding cliff).
/// Same FNV-1a as `rules_cond_ir::ctx::id_of`.
fn id_of_bytes(b: &[u8]) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in b.iter() {
        h ^= *x as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h as i64
}

/// `id_of(name)` for an interned [`NameId`] — hashes `resolve(id)` bytes.
/// Never call `resolve_str` under Kani (UTF-8 loop).
fn id_of_name_id(id: NameId) -> i64 {
    id_of_bytes(intern::resolve(id))
}

/// Fill every seat's [`SeatField`] row from the world.
fn fill_seat(c: &mut IrCtx, w: &World) {
    for p in 0..world::MAX_PLAYERS {
        c.seat[p][SeatField::Money as usize] = w.money[p] as i64;
        c.seat[p][SeatField::Fire as usize] = w.fire[p] as i64;
        c.seat[p][SeatField::Crystals as usize] = w.band_crystals[p] as i64;
        c.seat[p][SeatField::Hand as usize] = hand_size_view(w, p);
        c.seat[p][SeatField::Pos as usize] = w.pos[p] as i64;
        c.seat[p][SeatField::Out as usize] = w.out[p] as i64;
        c.seat[p][SeatField::Stay as usize] = w.stay[p] as i64;
        c.seat[p][SeatField::Stun as usize] = key_get_name(w, p as i32, KEY_STUN);
        c.seat[p][SeatField::Exile as usize] = key_get_name(w, p as i32, KEY_EXILE);
        c.seat[p][SeatField::NoHand as usize] = key_get_name(w, p as i32, KEY_NO_HAND);
        c.seat[p][SeatField::Character as usize] = id_of_name_id(NameId(w.character_id[p]));
        c.seat[p][SeatField::Band as usize] = id_of_name_id(NameId(w.band_id[p]));
        c.seat[p][SeatField::Tiles as usize] = owned_tiles(w, p as i32);
        // Turn trackers (World `gains_this_turn` / `targeted_count` /
        // `between_seat`). `gains_this_turn` is incremented by `Op::gain`;
        // `targeted_count` is a fixture-facing counter (host ops that "target"
        // a seat do not yet bump it — see module docs); `between_seat` is the
        // last between-witness, `-1` = none.
        c.seat[p][SeatField::Gains as usize] = w.gains_this_turn[p] as i64;
        c.seat[p][SeatField::Targeted as usize] = w.targeted_count[p] as i64;
        c.seat[p][SeatField::Between as usize] = w.between_seat[p] as i64;
    }
}

/// Board-derived membership sets (static, independent of the world).
fn fill_board_sets(c: &mut IrCtx, w: &World) {
    let mut i = 0usize;
    while i < 60 {
        let t = board::TILES[i];
        // Membership ids are the **0-based slot** (`tile.id` / `is_circle(t)`).
        // `TILES[i].index` is the 1-based board.json index — do not insert that.
        let id = i as i64;
        match t.kind {
            0 => {
                c.set_insert(SetId::Circle, id);
            }
            3 => {
                c.set_insert(SetId::Ring, id);
            }
            _ => {}
        }
        if t.kind == 1 || t.kind == 3 {
            c.set_insert(SetId::Buyable, id);
        }
        if (t.kind == 1 || t.kind == 3) && t.group == 6 {
            c.set_insert(SetId::LiveHouse, id);
        }
        i += 1;
    }
    // `BlockedBands` — band name ids the world currently blocks from skills.
    let mut b = 0usize;
    while b < w.blocked_bands_n as usize && b < 4 {
        let id = w.blocked_bands[b];
        if id != 0 {
            c.set_insert(SetId::BlockedBands, id as i64);
        }
        b += 1;
    }
}

/// Fill the geometry bin for the argument pairs the model can compute from
/// the world alone (`players_on`, `next_dist`, `owned_within`, `on_path`).
/// `BIN_CAP` is 64 so this is a **sparse** table: it answers the pairs we
/// precompute and 0 elsewhere. Guards whose args are symbolic need
/// `conf_fill_diff`-style per-program filling (see module docs).
fn fill_geometry_bins(c: &mut IrCtx, w: &World) {
    let n = 60i64;
    let n_players = w.n_players as i64;
    let pos = |s: i64| -> i64 {
        if s < 0 || s as usize >= world::MAX_PLAYERS {
            -1
        } else {
            w.pos[s as usize] as i64
        }
    };
    let out = |s: i64| -> bool {
        s < 0 || s as usize >= world::MAX_PLAYERS || w.out[s as usize]
    };
    let exile = |s: i64| -> i64 {
        if s < 0 || s as usize >= world::MAX_PLAYERS {
            1
        } else {
            key_get_name(w, s as i32, KEY_EXILE)
        }
    };
    let owner_of = |t: i64| -> i64 {
        if t < 0 || t as usize >= 60 {
            -1
        } else {
            w.tiles[t as usize].owner as i64
        }
    };
    let dist = |x: i64, y: i64| -> i64 {
        if n <= 0 {
            return 0;
        }
        let d = ((y - x) % n + n) % n;
        d.min(n - d)
    };
    // players_on(tile, except) for every occupied tile × every seat.
    let mut t = 0i64;
    while t < n {
        let mut except = -1i64;
        while except < n_players {
            let v = (0..n_players)
                .filter(|&s| s != except && !out(s) && exile(s) == 0 && pos(s) == t)
                .count() as i64;
            if v != 0 {
                c.bin_set(t, except, v);
            }
            except += 1;
        }
        t += 1;
    }
    // next_dist(p, dir) for every seat × {-1, 1}.
    let mut p = 0i64;
    while p < n_players {
        for dir in [-1i64, 1i64] {
            let mut best = i64::MAX;
            let mut o = 0i64;
            while o < n_players {
                if o != p && !out(o) {
                    let q = pos(o);
                    if q >= 0 {
                        let fwd = if dir >= 0 {
                            ((q - pos(p)) % n + n) % n
                        } else {
                            ((pos(p) - q) % n + n) % n
                        };
                        if fwd > 0 && fwd < best {
                            best = fwd;
                        }
                    }
                }
                o += 1;
            }
            let v = if best == i64::MAX { -1 } else { best };
            if v != 0 {
                c.bin_set(p, dir, v);
            }
        }
        p += 1;
    }
    // owned_within(p, radius) for radius 1..=6 (inventory uses small radii).
    let mut p = 0i64;
    while p < n_players {
        let ppos = pos(p);
        if ppos >= 0 {
            let mut radius = 1i64;
            while radius <= 6 {
                let v = (0..n)
                    .filter(|&ti| owner_of(ti) == p && dist(ppos, ti) <= radius)
                    .count() as i64;
                if v != 0 {
                    c.bin_set(p, radius, v);
                }
                radius += 1;
            }
        }
        p += 1;
    }
    // on_path(me, them) from `move_snap.roll` (steps = |roll|).
    let roll = w.move_snap.roll as i64;
    if roll >= 0 {
        let steps = roll.abs();
        let mut me = 0i64;
        while me < n_players {
            let mut them = 0i64;
            while them < n_players {
                if me != them {
                    let q = pos(them);
                    if q >= 0 {
                        let mut hits = 0i64;
                        let mut i = 1i64;
                        while i <= steps {
                            let ti = ((q + i) % n + n) % n;
                            if owner_of(ti) == me {
                                hits += 1;
                            }
                            i += 1;
                        }
                        if hits != 0 {
                            c.bin_set(me, them, hits);
                        }
                    }
                }
                them += 1;
            }
            me += 1;
        }
    }
}

/// Copy the owner's named counters (tokens / slots / state) into `IrCtx::named`.
fn fill_named(c: &mut IrCtx, w: &World, owner: i32) {
    if owner < 0 || owner as usize >= world::MAX_PLAYERS {
        return;
    }
    let map = &w.player_keys[owner as usize];
    let n = map.n as usize;
    let mut i = 0usize;
    let mut filled = 0usize;
    while i < n && filled < NAMED_CAP {
        let slot = map.slots[i];
        if slot.key != NameId::INVALID {
            let name_id = id_of_name_id(slot.key);
            c.named_set(name_id, slot.value as i64);
            filled += 1;
        }
        i += 1;
    }
    // Pilot fan views (same keys, guaranteed present even if the map is empty).
    c.named_set(
        rules_cond_ir::ctx::id_of(TOK_PP_POS),
        w.fans_pos[owner as usize] as i64,
    );
    c.named_set(
        rules_cond_ir::ctx::id_of(TOK_PP_NEG),
        w.fans_neg[owner as usize] as i64,
    );
}

/// Fill an [`IrCtx`] from the general world + candidate card.
///
/// * `owner` — the candidate seat (`card.placed`'s owner / the probed player)
/// * `card` — `id_of(card_name)` (use [`rules_cond_ir::ctx::id_of`])
/// * `card_placed` — whether the candidate is on the field
/// * `card_tile` — tile the candidate occupies (`-2` if not placed)
/// * `ov` — optional trigger extras the World does not carry
pub fn fill_from_world(
    w: &World,
    owner: i32,
    card: i64,
    card_placed: bool,
    card_tile: i64,
    ov: &TriggerOverlay,
) -> IrCtx {
    let mut c = IrCtx::default();
    c.n_players = w.n_players as i64;
    c.tile_count = 60;

    // OptInt defaults: any negative reads as Null. Start there, then overwrite.
    for (id, _, ty) in VARS {
        if *ty == VarTy::OptInt {
            c.set_scalar(*id, -1);
        }
    }

    // ---- window scalars ----
    // Overlay wins only when explicitly set (non-default); otherwise the
    // World's trigger payload is the source. See module docs.
    let kind = ov.kind.unwrap_or(w.trig_kind) as i64;
    c.set_scalar(VarId::Kind, kind);
    c.set_scalar(VarId::Actor, w.trig_player as i64);
    c.set_scalar(VarId::Target, w.trig_target as i64);
    let value = ov.value.unwrap_or(w.trig_value as i64);
    c.set_scalar(VarId::Value, value);
    c.set_scalar(VarId::Step, ov.step);
    c.set_scalar(VarId::By, ov.by);
    let pay_is_rent = if ov.pay_is_rent {
        true
    } else {
        w.trig_pay_is_rent
    };
    c.set_scalar(VarId::PayIsRent, pay_is_rent as i64);
    let roll_source = if ov.roll_source >= 0 {
        ov.roll_source
    } else {
        w.trig_roll_source as i64
    };
    c.set_scalar(VarId::RollSource, roll_source);
    c.set_scalar(VarId::Abnormal, ov.abnormal as i64);
    c.set_scalar(VarId::TurnPlayer, w.turn_player as i64);
    c.set_scalar(VarId::TurnKey, w.turn_key as i64);
    // `plan_fixed_roll` / `trigger_card` / `counter_name` now live on World;
    // the overlay remains the override for fixtures that have no World twin.
    let plan_fixed_roll = if ov.plan_fixed_roll != -1 {
        ov.plan_fixed_roll
    } else {
        w.plan_fixed_roll as i64
    };
    c.set_scalar(VarId::PlanFixedRoll, plan_fixed_roll);
    let trigger_card = if ov.trigger_card != 0 {
        ov.trigger_card
    } else {
        w.trig_card as i64
    };
    c.set_scalar(VarId::TriggerCard, trigger_card);
    let counter_name = if ov.counter_name != 0 {
        ov.counter_name
    } else {
        w.counter_name as i64
    };
    c.set_scalar(VarId::CounterName, counter_name);

    // ---- tile scalars ----
    let tile = w.trig_tile;
    c.set_scalar(VarId::TileId, tile as i64);
    if tile >= 0 && (tile as usize) < 60 {
        c.set_scalar(VarId::TileOwner, w.tiles[tile as usize].owner as i64);
        c.set_scalar(VarId::TileHouses, w.tiles[tile as usize].houses as i64);
        c.set_scalar(
            VarId::TileMortgaged,
            w.tiles[tile as usize].mortgaged as i64,
        );
        c.set_scalar(VarId::TilePrice, board::TILES[tile as usize].price as i64);
    } else {
        c.set_scalar(VarId::TileOwner, -1);
        c.set_scalar(VarId::TileHouses, 0);
        c.set_scalar(VarId::TileMortgaged, 0);
        c.set_scalar(VarId::TilePrice, 0);
    }

    // ---- move scalars (World `move_snap` / `trig_main`; overlay override) ----
    // Sentinels match `hostfns::trig_move_*`: no move → kind/roll = -1,
    // remaining = 0, dir = 1, and `move_main` is `Trigger::main`.
    let mv = &w.move_snap;
    let (move_roll, move_kind, move_remaining, move_main, move_dir) = if mv.present() {
        (
            mv.roll as i64,
            mv.kind as i64,
            mv.remaining as i64,
            w.trig_main as i64,
            mv.dir as i64,
        )
    } else {
        (-1, -1, 0, w.trig_main as i64, 1)
    };
    // Overlay still wins when it left its documented default behind.
    c.set_scalar(
        VarId::MoveRoll,
        if ov.move_roll != -1 { ov.move_roll } else { move_roll },
    );
    c.set_scalar(
        VarId::MoveKind,
        if ov.move_kind != -1 {
            ov.move_kind
        } else {
            move_kind
        },
    );
    c.set_scalar(
        VarId::MoveRemaining,
        if ov.move_remaining != 0 {
            ov.move_remaining
        } else {
            move_remaining
        },
    );
    c.set_scalar(
        VarId::MoveMain,
        if ov.move_main { 1 } else { move_main },
    );
    c.set_scalar(
        VarId::MoveDir,
        if ov.move_dir != 1 { ov.move_dir } else { move_dir },
    );

    // ---- chain (overlay; World has no chain list) ----
    let chain_n = (ov.chain_n as usize).min(4);
    c.set_scalar(VarId::EffectCount, chain_n as i64);
    let mut ci = 0usize;
    while ci < chain_n {
        let (kind, from, hits) = ov.chain[ci];
        c.set_insert(SetId::ChainKinds, kind);
        if hits >= 0 {
            c.set_insert(SetId::ChainHits, hits);
        }
        let _ = from;
        ci += 1;
    }

    // ---- candidate scalars ----
    let op = if owner >= 0 { owner as usize } else { 0 };
    c.set_scalar(VarId::Owner, owner as i64);
    c.set_scalar(VarId::OwnerId, owner as i64);
    c.set_scalar(VarId::OwnerMoney, w.money[op] as i64);
    c.set_scalar(VarId::OwnerFire, w.fire[op] as i64);
    c.set_scalar(VarId::OwnerCrystals, w.band_crystals[op] as i64);
    c.set_scalar(VarId::OwnerHand, hand_size_view(w, op));
    c.set_scalar(VarId::OwnerPos, w.pos[op] as i64);
    c.set_scalar(VarId::OwnerOut, w.out[op] as i64);
    c.set_scalar(VarId::OwnerStay, w.stay[op] as i64);
    c.set_scalar(VarId::OwnerStun, key_get_name(w, owner, KEY_STUN));
    c.set_scalar(VarId::OwnerExile, key_get_name(w, owner, KEY_EXILE));
    c.set_scalar(VarId::OwnerNoHand, key_get_name(w, owner, KEY_NO_HAND));
    c.set_scalar(
        VarId::OwnerCharacter,
        id_of_name_id(NameId(w.character_id[op])),
    );
    c.set_scalar(
        VarId::OwnerBand,
        id_of_name_id(NameId(w.band_id[op])),
    );
    c.set_scalar(VarId::OwnerTiles, owned_tiles(w, owner));
    c.set_scalar(VarId::CardId, card);
    c.set_scalar(VarId::CardPlaced, card_placed as i64);
    // `card.tile` is a plain Int: -1 = with owner, -2 = not placed (Finding F3).
    c.set_scalar(
        VarId::CardTile,
        if card_placed { card_tile } else { -2 },
    );

    fill_seat(&mut c, w);
    fill_board_sets(&mut c, w);
    fill_named(&mut c, w, owner);
    fill_geometry_bins(&mut c, w);
    // `tile_named('…')` — every board name, matching `fill_window`'s `tile_ids`.
    let mut t = 0usize;
    while t < TILE_NAMES.len() {
        let (id, name) = TILE_NAMES[t];
        c.named_set(rules_cond_ir::ctx::id_of(name), id as i64);
        t += 1;
    }
    // Candidate `card.counter('name')` pairs (key = `id_of(name)`).
    let mut k = 0usize;
    let n_counters = (ov.card_counters_n as usize).min(4);
    while k < n_counters {
        let (name_id, v) = ov.card_counters[k];
        c.named_set(name_id, v);
        k += 1;
    }
    c
}

/// PayAdd-shaped fill: actor = triggering seat, owner = card owner,
/// value = pay amount.
///
/// This is a **light** fill (window + candidate scalars + per-seat money/out
/// for the poorest scan). It skips the 60-entry `tile_named` bulk fill and
/// board sets — CBMC does not finish those inside a proof harness. The full
/// [`fill_from_world`] is pinned by `tests/conf_fill_diff.rs` on the host.
pub fn fill_pay_add(w: &World, owner: i32) -> IrCtx {
    let mut c = IrCtx::default();
    c.n_players = w.n_players as i64;
    c.tile_count = 60;
    // OptInt slots that the PayAdd guard family may read. No VARS walk —
    // iterating the 43-entry table is a CBMC cliff inside a proof.
    c.set_scalar(VarId::PlanFixedRoll, -1);
    c.set_scalar(VarId::MoveRoll, -1);
    c.set_scalar(VarId::MoveKind, -1);
    c.set_scalar(VarId::Actor, w.trig_player as i64);
    c.set_scalar(VarId::Owner, owner as i64);
    c.set_scalar(VarId::OwnerId, owner as i64);
    c.set_scalar(VarId::Value, w.trig_value as i64);
    c.set_scalar(VarId::Target, w.trig_target as i64);
    c.set_scalar(VarId::TurnPlayer, w.turn_player as i64);
    c.set_scalar(VarId::TurnKey, w.turn_key as i64);
    let op = if owner >= 0 { owner as usize } else { 0 };
    c.set_scalar(VarId::OwnerMoney, w.money[op] as i64);
    c.set_scalar(VarId::OwnerFire, w.fire[op] as i64);
    c.set_scalar(VarId::OwnerOut, w.out[op] as i64);
    c.set_scalar(VarId::OwnerStay, w.stay[op] as i64);
    c.set_scalar(VarId::OwnerPos, w.pos[op] as i64);
    let mut p = 0usize;
    while p < world::MAX_PLAYERS {
        c.seat[p][SeatField::Money as usize] = w.money[p] as i64;
        c.seat[p][SeatField::Out as usize] = w.out[p] as i64;
        p += 1;
    }
    // Fan tokens the shave clause reads.
    c.named_set(
        rules_cond_ir::ctx::id_of(TOK_PP_POS),
        w.fans_pos[op] as i64,
    );
    c.named_set(
        rules_cond_ir::ctx::id_of(TOK_PP_NEG),
        w.fans_neg[op] as i64,
    );
    c
}

/// Fill a context for the `is_circle` pilot: a placed card on `tile`.
pub fn fill_is_circle(w: &World, owner: i32, tile: i64) -> IrCtx {
    let mut c = fill_from_world(w, owner, 0, true, tile, &TriggerOverlay::default());
    c.set_scalar(VarId::TileId, tile);
    if tile >= 0 && (tile as usize) < 60 {
        c.set_scalar(VarId::TileOwner, w.tiles[tile as usize].owner as i64);
        c.set_scalar(VarId::TileHouses, w.tiles[tile as usize].houses as i64);
        c.set_scalar(
            VarId::TileMortgaged,
            w.tiles[tile as usize].mortgaged as i64,
        );
        c.set_scalar(VarId::TilePrice, board::TILES[tile as usize].price as i64);
    }
    c
}

/// Fill a context for the `effect.has(Pay)` pilot.
pub fn fill_chain_pay(w: &World, owner: i32) -> IrCtx {
    let mut c = fill_from_world(w, owner, 0, true, -1, &TriggerOverlay::default());
    // Chain link: kind = Pay (9), hits = owner.
    c.set_insert(SetId::ChainKinds, 9);
    c.set_insert(SetId::ChainHits, owner as i64);
    c.set_scalar(VarId::EffectCount, 1);
    let _ = (FnId::Tok, SetId::Circle);
    c
}

/// Evaluate a program as a condition (host root rule).
pub fn eval_bool(p: &rules_cond_ir::Program, c: &IrCtx) -> bool {
    p.eval_bool(c)
}

/// Value-level helper for the proofs.
pub fn eval_val(p: &rules_cond_ir::Program, c: &IrCtx) -> Val {
    p.eval(c)
}

/// Evaluate the guard at const `id` against `c`.
pub fn eval_id(id: usize, c: &IrCtx) -> bool {
    generated::eval_id(id, c)
}
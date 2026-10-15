//! Generic class registration for `tile_in` (ABI v56).
//!
//! Class-membership props (`class:<class>` / `classFor:<p>:<class>`) are
//! checked **first** by the caller -- they are the per-player override
//! mechanism, not more hard-coded host logic. The static class *definitions*
//! are a ruleset-supplied table ([`set_class_defs`]); the engine keeps no
//! class names or colour-group numbers of its own.
//!
//! Kind checks are **not** classes -- they are `tile_is` / `TileData.kind`.

use game_core::data::TileData;
use game_core::state::prop;
use std::sync::RwLock;

/// `(name, requires_buyable, matching colour groups)`.
pub type ClassDef = (String, bool, Vec<i32>);

static CLASS_DEFS: RwLock<Vec<ClassDef>> = RwLock::new(Vec::new());

/// Seed the class table from the ruleset (at load). The engine never names a
/// class; empty means only the generic `class*` props answer.
pub fn set_class_defs(defs: Vec<ClassDef>) {
    *CLASS_DEFS.write().unwrap() = defs;
}

/// Registered class names (ruleset data).
pub fn class_names() -> Vec<String> {
    CLASS_DEFS
        .read()
        .unwrap()
        .iter()
        .map(|(n, _, _)| n.clone())
        .collect()
}

fn class_def(class: &str) -> Option<ClassDef> {
    CLASS_DEFS
        .read()
        .unwrap()
        .iter()
        .find(|(n, _, _)| n == class)
        .cloned()
}

/// `tile_is` -- data tile kind by wire spelling (ruleset binding).
/// Unknown tile -> false. No class vocabulary here (use [`class_matches`]).
pub fn kind_is(data: Option<&TileData>, kind: &str) -> bool {
    data.is_some_and(|t| t.kind.as_str() == kind)
}

/// `tile_attr` -- generic i32 field read. `kind` / `color` strings are answered
/// by `tile_is` / future string attrs, not this API.
///
/// Missing-tile defaults match the old per-field readers: `group`/`tier`/
/// `area`/`index` -> `-1`, `price`/`house`/`rent` -> `0`.
pub fn attr_of(data: Option<&TileData>, attr: &str) -> i32 {
    let Some(t) = data else {
        return match attr {
            "group" | "tier" | "area" | "index" => -1,
            _ => 0,
        };
    };
    match attr {
        "group" => t.group,
        "tier" => t.tier,
        "price" => t.price,
        // House/build cost per level (`TileData::house`), not the standing count.
        "house" => t.house,
        // `"004"` -> 4; non-integer -> -1.
        "area" => t.area.parse::<i32>().unwrap_or(-1),
        "rent" => t.rent.len() as i32,
        "index" => t.index,
        _ => 0,
    }
}

/// `tile_in` -- class membership. Props first, then the class table below.
///
/// `prop` reads a tile prop (`CardWorld::tile_prop`); default 0 when absent.
pub fn tile_in(
    data: Option<&TileData>,
    player_id: i32,
    class: &str,
    prop: impl Fn(&str) -> i32,
) -> bool {
    // Generic class-membership marks/props FIRST (the per-player override).
    if prop(&format!("class:{class}")) != 0 {
        return true;
    }
    if player_id >= 0 && prop(&format!("classFor:{player_id}:{class}")) != 0 {
        return true;
    }
    class_matches(data, player_id, class, prop)
}

/// The ruleset class definitions (static half of `tile_in`, after the generic
/// `class*` props). Matching is table-driven: a class is true when the tile is
/// buyable (if the definition requires it) and its colour group is one of the
/// definition's groups (or the player counts the tile as that group via
/// `anyColor` / `colorFor`).
pub fn class_matches(
    data: Option<&TileData>,
    player_id: i32,
    class: &str,
    prop: impl Fn(&str) -> i32,
) -> bool {
    let Some(t) = data else {
        return false;
    };
    if let Some((_, buyable, groups)) = class_def(class) {
        if buyable && !t.is_buyable() {
            return false;
        }
        if groups.is_empty() {
            return true;
        }
        return groups.iter().any(|&g| is_color(player_id, t, g, &prop));
    }
    // Generic colour class: `color:<g>`.
    let Some(g) = class.strip_prefix("color:") else {
        return false;
    };
    match g.parse::<i32>() {
        Ok(g) => is_color(player_id, t, g, &prop),
        Err(_) => false,
    }
}

/// Colour-set membership (`docs/PURCHASE.md`): the tile's own `group`,
/// `prop::ALL_COLORS`, `prop::ANY_COLOR` (「该格获得所有颜色」), or this
/// player's `colorFor:<p>` naming `group` / `ALL_COLORS`.
///
/// When `player_id < 0`, per-player `colorFor` is ignored (still honour
/// `ANY_COLOR` and the tile's own group).
pub fn is_color(player_id: i32, t: &TileData, group: i32, prop: &impl Fn(&str) -> i32) -> bool {
    is_color_lazy(player_id, t, group, prop(prop::ANY_COLOR) != 0, || {
        prop(&format!("{}{}", prop::COLOR_FOR_PREFIX, player_id))
    })
}

/// [`is_color`] with the two prop reads supplied by the caller: `any` is
/// `prop::ANY_COLOR != 0`, `color_for` reads `colorFor:<player>` (only called
/// when it can matter).
fn is_color_lazy(
    player_id: i32,
    t: &TileData,
    group: i32,
    any: bool,
    color_for: impl FnOnce() -> i32,
) -> bool {
    if t.group == group || t.group == prop::ALL_COLORS || any {
        return true;
    }
    if player_id < 0 {
        return false;
    }
    let color_for = color_for();
    color_for == group || color_for == prop::ALL_COLORS
}

/// Whether a tile prop key can change a class answer: the per-player override
/// marks (`class:*` / `classFor:*`), `anyColor` and `colorFor:*`.
pub fn is_class_prop(key: &str) -> bool {
    key.starts_with("class") || key == prop::ANY_COLOR || key.starts_with(prop::COLOR_FOR_PREFIX)
}

/// [`class_rows_plain`] for a live world, or `false` when the tile carries a
/// class prop (the caller then probes [`tile_in`] one by one).
pub fn class_rows(
    world: &game_core::engine::World,
    data: Option<&TileData>,
    tile: i32,
    classes: &[String],
    out: &mut Vec<(i64, i64, String)>,
) -> bool {
    if world.tile_prop_keys(tile).any(is_class_prop) {
        return false;
    }
    class_rows_plain(data, tile as i64, world.st.players.len() as i32, classes, out);
    true
}

/// Every `(player, tile, class)` triple `tile_in` answers true for, over
/// `players = -1, 0..player_count`, **assuming the tile carries no class prop**
/// (see [`is_class_prop`]; the caller checks). Same answers as probing
/// [`tile_in`] with every prop reading 0, but allocation-free per probe.
pub fn class_rows_plain(
    data: Option<&TileData>,
    tile: i64,
    player_count: i32,
    classes: &[String],
    out: &mut Vec<(i64, i64, String)>,
) {
    let Some(t) = data else {
        return;
    };
    for p in std::iter::once(-1).chain(0..player_count) {
        for c in classes {
            let hit = if let Some((_, buyable, groups)) = class_def(c) {
                if buyable && !t.is_buyable() {
                    false
                } else if groups.is_empty() {
                    true
                } else {
                    groups.iter().any(|&g| is_color_lazy(p, t, g, false, || 0))
                }
            } else {
                match c.strip_prefix("color:").and_then(|g| g.parse::<i32>().ok()) {
                    Some(g) => is_color_lazy(p, t, g, false, || 0),
                    None => false,
                }
            };
            if hit {
                out.push((p as i64, tile, c.clone()));
            }
        }
    }
}
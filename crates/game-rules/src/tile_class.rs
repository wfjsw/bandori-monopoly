//! THE ruleset's class registration for `tile_in` (ABI v56).
//!
//! Generic class-membership props (`class:<class>` / `classFor:<p>:<class>`)
//! are checked **first** by the caller -- they are the per-player override
//! mechanism, not more hard-coded host logic. What lives here is the static
//! class definition table that replaces the old game-named predicates
//! (`is_buyable` / `is_shop` / `is_live_house` / `is_live_house_for` /
//! `is_color`).
//!
//! Kind checks are **not** classes -- they are `tile_is` / `TileData.kind`.

use game_core::data::TileData;
use game_core::state::prop;

/// `tile_is` -- data `TileKind` by wire spelling (`TileKind::as_str`).
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
/// `class*` props). Matching today's hard-coded predicates exactly:
///
/// * `"buyable"` -> `TileData::is_buyable()` (property|ring) -- old `is_buyable`
/// * `"shop"` -> buyable && group == 10 -- old `is_shop`
/// * `"livehouse"` -> buyable && group == 6, or for one player buyable &&
///   colour 6 -- old `is_live_house` / `is_live_house_for`
/// * `"color:<g>"` -> old `is_color` (own group / `ALL_COLORS` / `anyColor` /
///   `colorFor:<p>`)
/// * unknown -> false
pub fn class_matches(
    data: Option<&TileData>,
    player_id: i32,
    class: &str,
    prop: impl Fn(&str) -> i32,
) -> bool {
    let Some(t) = data else {
        return false;
    };
    match class {
        "buyable" => t.is_buyable(),
        "shop" => t.is_buyable() && t.group == 10,
        "livehouse" => {
            if player_id < 0 {
                t.is_buyable() && t.group == 6
            } else {
                t.is_buyable() && is_color(player_id, t, 6, &prop)
            }
        }
        _ => {
            let Some(g) = class.strip_prefix("color:") else {
                return false;
            };
            match g.parse::<i32>() {
                Ok(g) => is_color(player_id, t, g, &prop),
                Err(_) => false,
            }
        }
    }
}

/// Old `is_color` logic (`docs/PURCHASE.md`): the tile's own `group`,
/// `prop::ALL_COLORS`, `prop::ANY_COLOR` (「该格获得所有颜色」), or this
/// player's `colorFor:<p>` naming `group` / `ALL_COLORS`.
///
/// When `player_id < 0`, per-player `colorFor` is ignored (still honour
/// `ANY_COLOR` and the tile's own group).
pub fn is_color(player_id: i32, t: &TileData, group: i32, prop: &impl Fn(&str) -> i32) -> bool {
    let any = prop(prop::ANY_COLOR) != 0;
    if t.group == group || t.group == prop::ALL_COLORS || any {
        return true;
    }
    if player_id < 0 {
        return false;
    }
    let color_for = prop(&format!("{}{}", prop::COLOR_FOR_PREFIX, player_id));
    color_for == group || color_for == prop::ALL_COLORS
}
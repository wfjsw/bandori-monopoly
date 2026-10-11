//! Static board from `data/board.json` (60 tiles). Generated for Kani; do not edit by hand.
//! kind codes: 0=circle 1=property 2=agent 3=ring 4=cafe 5=edogawa 6=ryuseido
//!
//! **Tile indices are 0-based** — the same convention as
//! `game_core::engine::ops` (`tile_named` → `Vec::position`, `tile_price` →
//! `data.tiles.get(tile)`). `TILES[i].index` keeps the 1-based `board.json`
//! `index` field for human cross-reference only; do **not** pass it to a host
//! op. `tile_slot` is therefore the identity on `0..60`.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TileData {
    /// 1-based `board.json` index (cross-reference only).
    pub index: i32,
    pub kind: u8,
    pub group: i32,
    pub price: i32,
    pub house_cost: i32,
    /// Rent at 0 / 1 / 2 / 3+ houses (`purchase::table_rent` clamps at 3).
    /// Empty tables (circle / agent / cafe / shop / ring) are all-zero —
    /// `table_rent` answers 0; RiNG rent is special-cased to `price` in
    /// `WorldOps::rent_of` (see `host::rent_of_tile`).
    pub rent: [i32; 4],
}

pub const TILES: [TileData; 60] = [
    TileData { index: 1, kind: 0, group: 0, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 2, kind: 1, group: 1, price: 3000, house_cost: 2000, rent: [300, 1360, 3220, 6680] },
    TileData { index: 3, kind: 1, group: 7, price: 2600, house_cost: 1500, rent: [260, 1080, 2500, 5120] },
    TileData { index: 4, kind: 1, group: 7, price: 3200, house_cost: 2000, rent: [320, 1400, 3280, 6760] },
    TileData { index: 5, kind: 2, group: 1, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 6, kind: 1, group: 1, price: 1600, house_cost: 1000, rent: [160, 680, 1600, 3320] },
    TileData { index: 7, kind: 1, group: 1, price: 1600, house_cost: 1000, rent: [160, 680, 1600, 3320] },
    TileData { index: 8, kind: 1, group: 1, price: 1400, house_cost: 1000, rent: [140, 640, 1540, 3240] },
    TileData { index: 9, kind: 3, group: 6, price: 1000, house_cost: 500, rent: [0, 0, 0, 0] },
    TileData { index: 10, kind: 1, group: 2, price: 1800, house_cost: 1000, rent: [180, 720, 1660, 3400] },
    TileData { index: 11, kind: 1, group: 2, price: 2200, house_cost: 1500, rent: [220, 1000, 2380, 4960] },
    TileData { index: 12, kind: 1, group: 7, price: 600, house_cost: 500, rent: [60, 280, 700, 1520] },
    TileData { index: 13, kind: 2, group: 2, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 14, kind: 1, group: 2, price: 3200, house_cost: 2000, rent: [320, 1400, 3280, 6760] },
    TileData { index: 15, kind: 1, group: 7, price: 800, house_cost: 500, rent: [80, 320, 760, 1600] },
    TileData { index: 16, kind: 4, group: 0, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 17, kind: 3, group: 6, price: 1000, house_cost: 500, rent: [0, 0, 0, 0] },
    TileData { index: 18, kind: 2, group: 3, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 19, kind: 1, group: 3, price: 2800, house_cost: 1500, rent: [280, 1120, 2560, 5200] },
    TileData { index: 20, kind: 1, group: 3, price: 1600, house_cost: 1000, rent: [160, 680, 1600, 3320] },
    TileData { index: 21, kind: 1, group: 3, price: 3000, house_cost: 2000, rent: [300, 1360, 3220, 6680] },
    TileData { index: 22, kind: 1, group: 6, price: 2400, house_cost: 1500, rent: [240, 1040, 2440, 5040] },
    TileData { index: 23, kind: 1, group: 3, price: 1200, house_cost: 500, rent: [120, 400, 880, 1760] },
    TileData { index: 24, kind: 2, group: 4, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 25, kind: 1, group: 4, price: 2400, house_cost: 1500, rent: [240, 1040, 2440, 5040] },
    TileData { index: 26, kind: 1, group: 4, price: 1000, house_cost: 500, rent: [100, 360, 820, 1680] },
    TileData { index: 27, kind: 1, group: 5, price: 3000, house_cost: 2000, rent: [300, 1360, 3220, 6680] },
    TileData { index: 28, kind: 1, group: 5, price: 1800, house_cost: 1000, rent: [180, 720, 1660, 3400] },
    TileData { index: 29, kind: 2, group: 5, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 30, kind: 1, group: 5, price: 3600, house_cost: 2000, rent: [360, 1480, 3400, 6920] },
    TileData { index: 31, kind: 5, group: 0, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 32, kind: 2, group: 6, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 33, kind: 1, group: 7, price: 1200, house_cost: 500, rent: [120, 400, 880, 1760] },
    TileData { index: 34, kind: 1, group: 7, price: 2400, house_cost: 1500, rent: [240, 1040, 2440, 5040] },
    TileData { index: 35, kind: 2, group: 7, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 36, kind: 1, group: 7, price: 800, house_cost: 500, rent: [80, 320, 760, 1600] },
    TileData { index: 37, kind: 3, group: 6, price: 1000, house_cost: 500, rent: [0, 0, 0, 0] },
    TileData { index: 38, kind: 1, group: 8, price: 2800, house_cost: 1500, rent: [280, 1120, 2560, 5200] },
    TileData { index: 39, kind: 1, group: 8, price: 2800, house_cost: 1500, rent: [280, 1120, 2560, 5200] },
    TileData { index: 40, kind: 2, group: 8, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 41, kind: 1, group: 6, price: 3400, house_cost: 2000, rent: [340, 1440, 3340, 6840] },
    TileData { index: 42, kind: 2, group: 9, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 43, kind: 1, group: 6, price: 1200, house_cost: 500, rent: [120, 400, 880, 1760] },
    TileData { index: 44, kind: 1, group: 9, price: 1400, house_cost: 1000, rent: [140, 640, 1540, 3240] },
    TileData { index: 45, kind: 1, group: 9, price: 600, house_cost: 500, rent: [60, 280, 700, 1520] },
    TileData { index: 46, kind: 6, group: 0, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 47, kind: 2, group: 10, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
    TileData { index: 48, kind: 1, group: 10, price: 2200, house_cost: 1500, rent: [220, 1000, 2380, 4960] },
    TileData { index: 49, kind: 1, group: 10, price: 2000, house_cost: 1000, rent: [200, 760, 1720, 3480] },
    TileData { index: 50, kind: 1, group: 10, price: 2000, house_cost: 1000, rent: [200, 760, 1720, 3480] },
    TileData { index: 51, kind: 1, group: 6, price: 1400, house_cost: 1000, rent: [140, 640, 1540, 3240] },
    TileData { index: 52, kind: 1, group: 10, price: 2000, house_cost: 1000, rent: [200, 760, 1720, 3480] },
    TileData { index: 53, kind: 1, group: 10, price: 1800, house_cost: 1000, rent: [180, 720, 1660, 3400] },
    TileData { index: 54, kind: 3, group: 6, price: 1000, house_cost: 500, rent: [0, 0, 0, 0] },
    TileData { index: 55, kind: 1, group: 11, price: 2200, house_cost: 1500, rent: [220, 1000, 2380, 4960] },
    TileData { index: 56, kind: 1, group: 11, price: 2600, house_cost: 1500, rent: [260, 1080, 2500, 5120] },
    TileData { index: 57, kind: 1, group: 7, price: 1000, house_cost: 500, rent: [100, 360, 820, 1680] },
    TileData { index: 58, kind: 1, group: 11, price: 3400, house_cost: 2000, rent: [340, 1440, 3340, 6840] },
    TileData { index: 59, kind: 1, group: 11, price: 2600, house_cost: 1500, rent: [260, 1080, 2500, 5120] },
    TileData { index: 60, kind: 2, group: 11, price: 0, house_cost: 0, rent: [0, 0, 0, 0] },
];

/// Real-engine tile index (0-based, `0..60`) → table slot. Returns -1 if out of range.
///
/// This is the identity on valid indices: `TILES[t]` is the tile the engine
/// addresses as `t`. (Earlier revisions wrongly treated the argument as the
/// 1-based `board.json` index — that was a model bug, fixed here.)
pub fn tile_slot(index: i32) -> i32 {
    if (0..60).contains(&index) {
        index
    } else {
        -1
    }
}

/// Board-index lookup. Pass the **real-engine 0-based** index (`0..60`).
/// `data_at_board_index` kept the old 1-based name; prefer [`data_at`].
pub fn data_at(index: i32) -> Option<&'static TileData> {
    let s = tile_slot(index);
    if s < 0 {
        None
    } else {
        Some(&TILES[s as usize])
    }
}

/// 1-based `board.json` `index` field → data (cross-reference helper only).
pub fn data_at_board_index(board_json_index: i32) -> Option<&'static TileData> {
    data_at(board_json_index - 1)
}

/// Kind helper: 1 = property (buyable / buildable).
pub fn is_property(t: &TileData) -> bool {
    t.kind == 1
}

pub fn is_shop(t: &TileData) -> bool {
    t.kind == 5
}

pub fn is_ring(t: &TileData) -> bool {
    t.kind == 3
}

pub fn is_circle(t: &TileData) -> bool {
    t.kind == 0
}

pub fn is_agent(t: &TileData) -> bool {
    t.kind == 2
}

/// `purchase::table_rent` — rent at `houses` (clamped into the 0..=3 table).
/// Empty table ⇒ 0. RiNG is **not** handled here (`WorldOps::rent_of` returns
/// the tile's `price` for RiNG); see `host::rent_of_tile`.
pub fn table_rent(tile: &TileData, houses: i32) -> i32 {
    let h = houses.max(0) as usize;
    tile.rent[h.min(3)]
}
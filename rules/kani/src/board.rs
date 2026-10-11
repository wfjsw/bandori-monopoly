//! Static board from `data/board.json` (60 tiles). Generated for Kani; do not edit by hand.
//! kind codes: 0=circle 1=property 2=agent 3=ring 4=cafe 5=edogawa 6=ryuseido

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TileData {
    pub index: i32,
    pub kind: u8,
    pub group: i32,
    pub price: i32,
    pub house_cost: i32,
}

pub const TILES: [TileData; 60] = [
    TileData { index: 1, kind: 0, group: 0, price: 0, house_cost: 0 },
    TileData { index: 2, kind: 1, group: 1, price: 3000, house_cost: 2000 },
    TileData { index: 3, kind: 1, group: 7, price: 2600, house_cost: 1500 },
    TileData { index: 4, kind: 1, group: 7, price: 3200, house_cost: 2000 },
    TileData { index: 5, kind: 2, group: 1, price: 0, house_cost: 0 },
    TileData { index: 6, kind: 1, group: 1, price: 1600, house_cost: 1000 },
    TileData { index: 7, kind: 1, group: 1, price: 1600, house_cost: 1000 },
    TileData { index: 8, kind: 1, group: 1, price: 1400, house_cost: 1000 },
    TileData { index: 9, kind: 3, group: 6, price: 1000, house_cost: 500 },
    TileData { index: 10, kind: 1, group: 2, price: 1800, house_cost: 1000 },
    TileData { index: 11, kind: 1, group: 2, price: 2200, house_cost: 1500 },
    TileData { index: 12, kind: 1, group: 7, price: 600, house_cost: 500 },
    TileData { index: 13, kind: 2, group: 2, price: 0, house_cost: 0 },
    TileData { index: 14, kind: 1, group: 2, price: 3200, house_cost: 2000 },
    TileData { index: 15, kind: 1, group: 7, price: 800, house_cost: 500 },
    TileData { index: 16, kind: 4, group: 0, price: 0, house_cost: 0 },
    TileData { index: 17, kind: 3, group: 6, price: 1000, house_cost: 500 },
    TileData { index: 18, kind: 2, group: 3, price: 0, house_cost: 0 },
    TileData { index: 19, kind: 1, group: 3, price: 2800, house_cost: 1500 },
    TileData { index: 20, kind: 1, group: 3, price: 1600, house_cost: 1000 },
    TileData { index: 21, kind: 1, group: 3, price: 3000, house_cost: 2000 },
    TileData { index: 22, kind: 1, group: 6, price: 2400, house_cost: 1500 },
    TileData { index: 23, kind: 1, group: 3, price: 1200, house_cost: 500 },
    TileData { index: 24, kind: 2, group: 4, price: 0, house_cost: 0 },
    TileData { index: 25, kind: 1, group: 4, price: 2400, house_cost: 1500 },
    TileData { index: 26, kind: 1, group: 4, price: 1000, house_cost: 500 },
    TileData { index: 27, kind: 1, group: 5, price: 3000, house_cost: 2000 },
    TileData { index: 28, kind: 1, group: 5, price: 1800, house_cost: 1000 },
    TileData { index: 29, kind: 2, group: 5, price: 0, house_cost: 0 },
    TileData { index: 30, kind: 1, group: 5, price: 3600, house_cost: 2000 },
    TileData { index: 31, kind: 5, group: 0, price: 0, house_cost: 0 },
    TileData { index: 32, kind: 2, group: 6, price: 0, house_cost: 0 },
    TileData { index: 33, kind: 1, group: 7, price: 1200, house_cost: 500 },
    TileData { index: 34, kind: 1, group: 7, price: 2400, house_cost: 1500 },
    TileData { index: 35, kind: 2, group: 7, price: 0, house_cost: 0 },
    TileData { index: 36, kind: 1, group: 7, price: 800, house_cost: 500 },
    TileData { index: 37, kind: 3, group: 6, price: 1000, house_cost: 500 },
    TileData { index: 38, kind: 1, group: 8, price: 2800, house_cost: 1500 },
    TileData { index: 39, kind: 1, group: 8, price: 2800, house_cost: 1500 },
    TileData { index: 40, kind: 2, group: 8, price: 0, house_cost: 0 },
    TileData { index: 41, kind: 1, group: 6, price: 3400, house_cost: 2000 },
    TileData { index: 42, kind: 2, group: 9, price: 0, house_cost: 0 },
    TileData { index: 43, kind: 1, group: 6, price: 1200, house_cost: 500 },
    TileData { index: 44, kind: 1, group: 9, price: 1400, house_cost: 1000 },
    TileData { index: 45, kind: 1, group: 9, price: 600, house_cost: 500 },
    TileData { index: 46, kind: 6, group: 0, price: 0, house_cost: 0 },
    TileData { index: 47, kind: 2, group: 10, price: 0, house_cost: 0 },
    TileData { index: 48, kind: 1, group: 10, price: 2200, house_cost: 1500 },
    TileData { index: 49, kind: 1, group: 10, price: 2000, house_cost: 1000 },
    TileData { index: 50, kind: 1, group: 10, price: 2000, house_cost: 1000 },
    TileData { index: 51, kind: 1, group: 6, price: 1400, house_cost: 1000 },
    TileData { index: 52, kind: 1, group: 10, price: 2000, house_cost: 1000 },
    TileData { index: 53, kind: 1, group: 10, price: 1800, house_cost: 1000 },
    TileData { index: 54, kind: 3, group: 6, price: 1000, house_cost: 500 },
    TileData { index: 55, kind: 1, group: 11, price: 2200, house_cost: 1500 },
    TileData { index: 56, kind: 1, group: 11, price: 2600, house_cost: 1500 },
    TileData { index: 57, kind: 1, group: 7, price: 1000, house_cost: 500 },
    TileData { index: 58, kind: 1, group: 11, price: 3400, house_cost: 2000 },
    TileData { index: 59, kind: 1, group: 11, price: 2600, house_cost: 1500 },
    TileData { index: 60, kind: 2, group: 11, price: 0, house_cost: 0 },
];

/// 1-based board index → table slot (index-1). Returns -1 if out of range.
pub fn tile_slot(index: i32) -> i32 {
    if index >= 1 && index <= 60 { index - 1 } else { -1 }
}

/// Board-index lookup (1-based, as `ctx::tile_named` resolves).
pub fn data_at_board_index(index: i32) -> Option<&'static TileData> {
    let s = tile_slot(index);
    if s < 0 {
        None
    } else {
        Some(&TILES[s as usize])
    }
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
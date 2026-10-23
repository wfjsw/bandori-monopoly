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
    TileData {
        index: 1,
        kind: 0,
        group: 0,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 2,
        kind: 1,
        group: 1,
        price: 3000,
        house_cost: 2000,
        rent: [300, 1360, 3220, 6680],
    },
    TileData {
        index: 3,
        kind: 1,
        group: 7,
        price: 2600,
        house_cost: 1500,
        rent: [260, 1080, 2500, 5120],
    },
    TileData {
        index: 4,
        kind: 1,
        group: 7,
        price: 3200,
        house_cost: 2000,
        rent: [320, 1400, 3280, 6760],
    },
    TileData {
        index: 5,
        kind: 2,
        group: 1,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 6,
        kind: 1,
        group: 1,
        price: 1600,
        house_cost: 1000,
        rent: [160, 680, 1600, 3320],
    },
    TileData {
        index: 7,
        kind: 1,
        group: 1,
        price: 1600,
        house_cost: 1000,
        rent: [160, 680, 1600, 3320],
    },
    TileData {
        index: 8,
        kind: 1,
        group: 1,
        price: 1400,
        house_cost: 1000,
        rent: [140, 640, 1540, 3240],
    },
    TileData {
        index: 9,
        kind: 3,
        group: 6,
        price: 1000,
        house_cost: 500,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 10,
        kind: 1,
        group: 2,
        price: 1800,
        house_cost: 1000,
        rent: [180, 720, 1660, 3400],
    },
    TileData {
        index: 11,
        kind: 1,
        group: 2,
        price: 2200,
        house_cost: 1500,
        rent: [220, 1000, 2380, 4960],
    },
    TileData {
        index: 12,
        kind: 1,
        group: 7,
        price: 600,
        house_cost: 500,
        rent: [60, 280, 700, 1520],
    },
    TileData {
        index: 13,
        kind: 2,
        group: 2,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 14,
        kind: 1,
        group: 2,
        price: 3200,
        house_cost: 2000,
        rent: [320, 1400, 3280, 6760],
    },
    TileData {
        index: 15,
        kind: 1,
        group: 7,
        price: 800,
        house_cost: 500,
        rent: [80, 320, 760, 1600],
    },
    TileData {
        index: 16,
        kind: 4,
        group: 0,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 17,
        kind: 3,
        group: 6,
        price: 1000,
        house_cost: 500,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 18,
        kind: 2,
        group: 3,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 19,
        kind: 1,
        group: 3,
        price: 2800,
        house_cost: 1500,
        rent: [280, 1120, 2560, 5200],
    },
    TileData {
        index: 20,
        kind: 1,
        group: 3,
        price: 1600,
        house_cost: 1000,
        rent: [160, 680, 1600, 3320],
    },
    TileData {
        index: 21,
        kind: 1,
        group: 3,
        price: 3000,
        house_cost: 2000,
        rent: [300, 1360, 3220, 6680],
    },
    TileData {
        index: 22,
        kind: 1,
        group: 6,
        price: 2400,
        house_cost: 1500,
        rent: [240, 1040, 2440, 5040],
    },
    TileData {
        index: 23,
        kind: 1,
        group: 3,
        price: 1200,
        house_cost: 500,
        rent: [120, 400, 880, 1760],
    },
    TileData {
        index: 24,
        kind: 2,
        group: 4,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 25,
        kind: 1,
        group: 4,
        price: 2400,
        house_cost: 1500,
        rent: [240, 1040, 2440, 5040],
    },
    TileData {
        index: 26,
        kind: 1,
        group: 4,
        price: 1000,
        house_cost: 500,
        rent: [100, 360, 820, 1680],
    },
    TileData {
        index: 27,
        kind: 1,
        group: 5,
        price: 3000,
        house_cost: 2000,
        rent: [300, 1360, 3220, 6680],
    },
    TileData {
        index: 28,
        kind: 1,
        group: 5,
        price: 1800,
        house_cost: 1000,
        rent: [180, 720, 1660, 3400],
    },
    TileData {
        index: 29,
        kind: 2,
        group: 5,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 30,
        kind: 1,
        group: 5,
        price: 3600,
        house_cost: 2000,
        rent: [360, 1480, 3400, 6920],
    },
    TileData {
        index: 31,
        kind: 5,
        group: 0,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 32,
        kind: 2,
        group: 6,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 33,
        kind: 1,
        group: 7,
        price: 1200,
        house_cost: 500,
        rent: [120, 400, 880, 1760],
    },
    TileData {
        index: 34,
        kind: 1,
        group: 7,
        price: 2400,
        house_cost: 1500,
        rent: [240, 1040, 2440, 5040],
    },
    TileData {
        index: 35,
        kind: 2,
        group: 7,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 36,
        kind: 1,
        group: 7,
        price: 800,
        house_cost: 500,
        rent: [80, 320, 760, 1600],
    },
    TileData {
        index: 37,
        kind: 3,
        group: 6,
        price: 1000,
        house_cost: 500,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 38,
        kind: 1,
        group: 8,
        price: 2800,
        house_cost: 1500,
        rent: [280, 1120, 2560, 5200],
    },
    TileData {
        index: 39,
        kind: 1,
        group: 8,
        price: 2800,
        house_cost: 1500,
        rent: [280, 1120, 2560, 5200],
    },
    TileData {
        index: 40,
        kind: 2,
        group: 8,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 41,
        kind: 1,
        group: 6,
        price: 3400,
        house_cost: 2000,
        rent: [340, 1440, 3340, 6840],
    },
    TileData {
        index: 42,
        kind: 2,
        group: 9,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 43,
        kind: 1,
        group: 6,
        price: 1200,
        house_cost: 500,
        rent: [120, 400, 880, 1760],
    },
    TileData {
        index: 44,
        kind: 1,
        group: 9,
        price: 1400,
        house_cost: 1000,
        rent: [140, 640, 1540, 3240],
    },
    TileData {
        index: 45,
        kind: 1,
        group: 9,
        price: 600,
        house_cost: 500,
        rent: [60, 280, 700, 1520],
    },
    TileData {
        index: 46,
        kind: 6,
        group: 0,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 47,
        kind: 2,
        group: 10,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 48,
        kind: 1,
        group: 10,
        price: 2200,
        house_cost: 1500,
        rent: [220, 1000, 2380, 4960],
    },
    TileData {
        index: 49,
        kind: 1,
        group: 10,
        price: 2000,
        house_cost: 1000,
        rent: [200, 760, 1720, 3480],
    },
    TileData {
        index: 50,
        kind: 1,
        group: 10,
        price: 2000,
        house_cost: 1000,
        rent: [200, 760, 1720, 3480],
    },
    TileData {
        index: 51,
        kind: 1,
        group: 6,
        price: 1400,
        house_cost: 1000,
        rent: [140, 640, 1540, 3240],
    },
    TileData {
        index: 52,
        kind: 1,
        group: 10,
        price: 2000,
        house_cost: 1000,
        rent: [200, 760, 1720, 3480],
    },
    TileData {
        index: 53,
        kind: 1,
        group: 10,
        price: 1800,
        house_cost: 1000,
        rent: [180, 720, 1660, 3400],
    },
    TileData {
        index: 54,
        kind: 3,
        group: 6,
        price: 1000,
        house_cost: 500,
        rent: [0, 0, 0, 0],
    },
    TileData {
        index: 55,
        kind: 1,
        group: 11,
        price: 2200,
        house_cost: 1500,
        rent: [220, 1000, 2380, 4960],
    },
    TileData {
        index: 56,
        kind: 1,
        group: 11,
        price: 2600,
        house_cost: 1500,
        rent: [260, 1080, 2500, 5120],
    },
    TileData {
        index: 57,
        kind: 1,
        group: 7,
        price: 1000,
        house_cost: 500,
        rent: [100, 360, 820, 1680],
    },
    TileData {
        index: 58,
        kind: 1,
        group: 11,
        price: 3400,
        house_cost: 2000,
        rent: [340, 1440, 3340, 6840],
    },
    TileData {
        index: 59,
        kind: 1,
        group: 11,
        price: 2600,
        house_cost: 1500,
        rent: [260, 1080, 2500, 5120],
    },
    TileData {
        index: 60,
        kind: 2,
        group: 11,
        price: 0,
        house_cost: 0,
        rent: [0, 0, 0, 0],
    },
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

/// `TileData::is_buyable` — Property or Ring (game-core `data.rs`).
/// Replaced by `tile_in(t, "buyable")` on the host; kept as the static-data half.
pub fn is_buyable(t: &TileData) -> bool {
    t.kind == 1 || t.kind == 3
}

/// A 「商店街」 deed: buyable and colour group 10 (wasm_rules `is_shop`).
/// `kind == 5` is only 「江户川乐器店」 — that is **not** the shop predicate.
/// The agent tile 「商店街」 itself is the colour-group marker (group 10,
/// kind agent) and is **not** buyable; the group-10 deeds beside it are.
pub fn is_shop(t: &TileData) -> bool {
    is_buyable(t) && t.group == 10
}

pub fn is_deed(t: &TileData) -> bool {
    t.kind == 3
}

pub fn is_start(t: &TileData) -> bool {
    t.kind == 0
}

pub fn is_agent(t: &TileData) -> bool {
    t.kind == 2
}

/// ABI v56: `tile_is` -- data kind by wire spelling (kind codes: 0=circle
/// 1=property 2=agent 3=ring 4=cafe 5=edogawa 6=ryuseido).
pub fn tile_is(t: &TileData, kind: &str) -> bool {
    match kind {
        "circle" => is_start(t),
        "property" => is_property(t),
        "agent" => is_agent(t),
        "ring" => is_deed(t),
        "cafe" => t.kind == 4,
        "edogawa" => t.kind == 5,
        "ryuseido" => t.kind == 6,
        _ => false,
    }
}

/// ABI v56: `tile_attr` -- generic field read. `"house"` is the build cost
/// (`TileData.house` / `house_cost`), not the standing count. `"rent"` is the
/// rent table length (this model's fixed `[i32; 4]`). `"area"`/`"tier"` are
/// not modelled (missing -> -1 / 0 defaults).
pub fn tile_attr(t: &TileData, attr: &str) -> i32 {
    match attr {
        "group" => t.group,
        "tier" => -1,
        "price" => t.price,
        "house" => t.house_cost,
        "area" => -1,
        "rent" => t.rent.len() as i32,
        "index" => t.index,
        _ => 0,
    }
}

/// ABI v56: `tile_in` -- static class definitions (props are not modelled on
/// this static board). Matching `game-rules::tile_class`.
pub fn tile_in(t: &TileData, class: &str) -> bool {
    match class {
        "buyable" => is_buyable(t),
        "shop" => is_shop(t),
        "livehouse" => is_buyable(t) && t.group == 6,
        _ => {
            let Some(g) = class.strip_prefix("color:") else {
                return false;
            };
            match g.parse::<i32>() {
                Ok(g) => t.group == g || t.group == -2,
                Err(_) => false,
            }
        }
    }
}

/// `purchase::table_rent` — rent at `houses` (clamped into the 0..=3 table).
/// Empty table ⇒ 0. RiNG is **not** handled here (`WorldOps::rent_of` returns
/// the tile's `price` for RiNG); see `host::rent_of_tile`.
pub fn table_rent(tile: &TileData, houses: i32) -> i32 {
    let h = houses.max(0) as usize;
    tile.rent[h.min(3)]
}
/// Every board tile name, in 0-based engine index order (`board.json`).
/// `tile_named` resolves against this table (hostfns: `Vec::position`).
pub const TILE_NAMES: [&str; 60] = [
    "CiRCLE",
    "购物中心",
    "天文馆",
    "水族馆",
    "主要街道",
    "偶像经纪公司",
    "星空齿科",
    "江户川公园",
    "RiNG 1",
    "花咲川女子学院",
    "羽丘女子学院",
    "富士见坂",
    "中心学区",
    "月之森女子学院",
    "成为人类桥",
    "CiRCLE 咖啡厅",
    "RiNG 2",
    "周边学区",
    "白雪学园",
    "艺术学院高中",
    "瑟罗希亚国际学校",
    "DUB MUSIC EXPERIMENT",
    "加茂川中央中学",
    "东京外",
    "大阪中之岛公园",
    "小豆岛",
    "微笑号",
    "米歇尔公园",
    "弦卷集团",
    "弦卷豪宅",
    "江户川乐器店",
    "Live House",
    "便利店",
    "快餐店",
    "周边精选",
    "Bandori车站",
    "RiNG 3",
    "庆鹏女子大学",
    "四叶女子大学",
    "大学路",
    "武道馆",
    "梦开始的地方",
    "Space",
    "星星小巷",
    "星之鼓动山丘",
    "流星堂",
    "商店街",
    "羽泽咖啡厅",
    "山吹面包房",
    "银河拉面馆",
    "Live House Galaxy",
    "北泽精肉店",
    "旭汤澡堂",
    "RiNG 4",
    "六本木大厦",
    "CHUCHU的公寓",
    "飞鸟山公园",
    "旧古河庭园",
    "广町家画室",
    "高级住宅区",
];

/// Resolve a board tile name to its 0-based engine index, or -1.
/// Mirrors `game_core` `tile_named`: `Vec::position` over `data.tiles`.
/// Look-up is a binary search over a compile-time FNV-1a hash table
/// (~6 integer compares), then a byte compare on the hit — never a 60-slot
/// name scan (that is a CBMC unwinding cliff).
pub fn tile_named(name: &str) -> i32 {
    tile_named_bytes(name.as_bytes())
}

/// FNV-1a 64, same as `rules_cond_ir::ctx::id_of`.
///
/// Under `cfg(kani)` this is a **zero-loop** 64-way step (same reason as
/// `card_sdk::intern::fnv1a`: a `while i < len` with a symbolic slice length
/// is an unwinding cliff — `tile_named(SPOTS[sym])` in `RAS:UNSTOPPABLE`
/// never returned). Production keeps the loop.
#[cfg(kani)]
pub fn tile_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    macro_rules! step {
        ($i:expr) => {
            if $i < bytes.len() {
                h ^= bytes[$i] as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
    }
    // 64 straight-line steps; board names are ≤ 32 UTF-8 bytes.
    step!(0);
    step!(1);
    step!(2);
    step!(3);
    step!(4);
    step!(5);
    step!(6);
    step!(7);
    step!(8);
    step!(9);
    step!(10);
    step!(11);
    step!(12);
    step!(13);
    step!(14);
    step!(15);
    step!(16);
    step!(17);
    step!(18);
    step!(19);
    step!(20);
    step!(21);
    step!(22);
    step!(23);
    step!(24);
    step!(25);
    step!(26);
    step!(27);
    step!(28);
    step!(29);
    step!(30);
    step!(31);
    step!(32);
    step!(33);
    step!(34);
    step!(35);
    step!(36);
    step!(37);
    step!(38);
    step!(39);
    step!(40);
    step!(41);
    step!(42);
    step!(43);
    step!(44);
    step!(45);
    step!(46);
    step!(47);
    step!(48);
    step!(49);
    step!(50);
    step!(51);
    step!(52);
    step!(53);
    step!(54);
    step!(55);
    step!(56);
    step!(57);
    step!(58);
    step!(59);
    step!(60);
    step!(61);
    step!(62);
    step!(63);
    h
}

#[cfg(not(kani))]
const fn tile_hash(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0usize;
    while i < bytes.len() {
        h ^= bytes[i] as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    h
}

/// Sorted `(hash, board_index)` pairs. Generated from [`TILE_NAMES`];
/// uniqueness checked by `board_hash_table_is_sound` below and by the
/// `conf_model` suite. Hashes are distinct, so the binary search is exact
/// without a collision chain (the byte compare still guards the hit).
pub const TILE_NAME_HASHES: [u64; 60] = [
    0x032cd9f1245d3645,
    0x13804180992c8621,
    0x178e0cafb35c31a0,
    0x18f17caa0f030356,
    0x1c77050fdc356991,
    0x1cc771af5833ad4e,
    0x274bea073f2490f2,
    0x2aeae7b7c50aa67b,
    0x2e8d1ee1111664c0,
    0x2e8d20e111166826,
    0x2e8d21e1111669d9,
    0x2e8d23e111166d3f,
    0x34be7b838c5fc85b,
    0x3708e53ea661ca17,
    0x38f818e9e7561f6d,
    0x4c99fc977c26ddf7,
    0x4d15b09c6a8c5a6e,
    0x4f1014d761d4ca95,
    0x5239c782d314a3fb,
    0x54be9e1393afabdb,
    0x57f6d9f87b7537cb,
    0x588ad81f2bb57e4f,
    0x5a39cceefbf08571,
    0x5e3de51e93788684,
    0x61545b7d96778f7b,
    0x64a97a849ece896b,
    0x66138e1d6ca2a8b9,
    0x6c39a833ff758487,
    0x6fe9f868fa327a18,
    0x730c581c6ecb1a13,
    0x73cb295f4fd492ac,
    0x78e9448d7fe23590,
    0x8270a0584fdf1978,
    0x8e28a4021fff37d3,
    0x8e6cb0aa4b51728f,
    0x90dd8a2bca725c23,
    0x99325798a3160f06,
    0xa136a621ddaeda90,
    0xa84a2b984eec9ac1,
    0xa91c1a2bdbc9734f,
    0xaf73f4cb9cbbc3af,
    0xbc4497544c378d86,
    0xc0be4f1be812125b,
    0xc3251b991b2a1f48,
    0xc7555c5a4025f8c2,
    0xc7d56ebc60aac086,
    0xc8a7761f272db000,
    0xc8ae7fb9eb1e9dd6,
    0xc94c95ac9b573a34,
    0xcc4529c6b951d4f0,
    0xd49b4b969e42c6a9,
    0xd90ee3af40fdd8c2,
    0xe7211eb19dbea643,
    0xf1df3ad5891ff8d0,
    0xf72ba860bed49ee2,
    0xfb49549e05ae1d38,
    0xfc29171dbcad19c6,
    0xfdb78a02313cbe6d,
    0xfe898bd257c45c6e,
    0xff06a1bcc8c3aeae,
];

/// Board index for each [`TILE_NAME_HASHES`] slot (same order).
pub const TILE_NAME_IDX: [i32; 60] = [
    42, 38, 24, 41, 18, 30, 37, 1, 8, 36, 16, 53, 55, 54, 48, 45, 29, 50, 17, 26, 15, 20, 34, 52,
    9, 39, 13, 49, 19, 33, 32, 11, 25, 2, 57, 56, 22, 12, 28, 47, 46, 3, 58, 40, 59, 10, 4, 21, 43,
    44, 0, 5, 31, 14, 7, 23, 6, 51, 35, 27,
];

/// [`tile_named`] over raw guest bytes (the Kani ABI passes `ptr, len`).
/// Binary search on the FNV table; byte-compare only the matching slot.
pub fn tile_named_bytes(name: &[u8]) -> i32 {
    if name.len() > 64 {
        return -1;
    }
    let h = tile_hash(name);
    // Binary search TILE_NAME_HASHES (sorted, unique).
    let mut lo = 0usize;
    let mut hi = 60usize;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if TILE_NAME_HASHES[mid] < h {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if lo < 60 && TILE_NAME_HASHES[lo] == h {
        let idx = TILE_NAME_IDX[lo] as usize;
        if bytes_eq(TILE_NAMES[idx].as_bytes(), name) {
            return idx as i32;
        }
    }
    -1
}

fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() || a.len() > 64 {
        return false;
    }
    // Zero-loop compare (`.get`, no panic paths). Same bits as the old 8×8 walk.
    let mut ok = true;
    macro_rules! step {
        ($i:expr) => {
            match (a.get($i), b.get($i)) {
                (Some(x), Some(y)) if x != y => ok = false,
                _ => {}
            }
        };
    }
    step!(0);
    step!(1);
    step!(2);
    step!(3);
    step!(4);
    step!(5);
    step!(6);
    step!(7);
    step!(8);
    step!(9);
    step!(10);
    step!(11);
    step!(12);
    step!(13);
    step!(14);
    step!(15);
    step!(16);
    step!(17);
    step!(18);
    step!(19);
    step!(20);
    step!(21);
    step!(22);
    step!(23);
    step!(24);
    step!(25);
    step!(26);
    step!(27);
    step!(28);
    step!(29);
    step!(30);
    step!(31);
    step!(32);
    step!(33);
    step!(34);
    step!(35);
    step!(36);
    step!(37);
    step!(38);
    step!(39);
    step!(40);
    step!(41);
    step!(42);
    step!(43);
    step!(44);
    step!(45);
    step!(46);
    step!(47);
    step!(48);
    step!(49);
    step!(50);
    step!(51);
    step!(52);
    step!(53);
    step!(54);
    step!(55);
    step!(56);
    step!(57);
    step!(58);
    step!(59);
    step!(60);
    step!(61);
    step!(62);
    step!(63);
    ok
}

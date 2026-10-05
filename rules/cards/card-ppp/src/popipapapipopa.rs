//! `PPP:[衍生]Popipapapipopa` -- C# `CardPopipapapipopa` (MatchHost.cs:8352-8402):
//! placed card that banks [奇迹水晶] when its owner passes five named tiles and
//! spends them to shrink payments.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:[衍生]Popipapapipopa`）:
//! > [衍生]Popipapapipopa：
//! > [持续]
//! >
//! > （1）此卡拥有者每次[经过]“东京外”，“江户川乐器店”，“山吹面包房”，“星之鼓动山丘”或“流星堂”时为此卡添加1个[奇迹水晶]（上限10个）。
//! >
//! > （2）消耗或支付时可使用此卡多个奇迹水晶，每个将资金变动减少150（最少0）。
//!
//! A pure [持续] card: the C# has no `Play`, it is placed by `CardPipopa`
//! (`place_card` in `pipopa.rs`). Both continuous clauses are hooks the ABI does
//! not carry yet (TODO below).

use card_sdk::CardDef;

pub const POPIPAPAPIPOPA: CardDef = CardDef {
    id: "PPP:[衍生]Popipapapipopa",
    play: None,
    can_react: None,
    react: None,
    why_not: None,
};

/// C# `CardPopipapapipopa.Spots` -- the five tiles that feed a crystal.
#[allow(dead_code)]
const SPOTS: [&str; 5] = ["东京外", "江户川乐器店", "山吹面包房", "星之鼓动山丘", "流星堂"];

// TODO(ABI)（1）: 「此卡拥有者每次[经过]“东京外”，“江户川乐器店”，“山吹面包房”，
//   “星之鼓动山丘”或“流星堂”时为此卡添加1个[奇迹水晶]（上限10个）。」
//   -- needs the Fx.PassTile hook (C# `CardPopipapapipopa.PassTile`) and per-field-card
//   crystals (C# `Card.AddCrystals` / `MaxCrystals = 10`; the ABI has `band_crystals`
//   and seat tokens only, not crystals hanging off one placed card).
// TODO(ABI)（2）: 「消耗或支付时可使用此卡多个奇迹水晶，每个将资金变动减少150（最少0）。」
//   -- needs the Fx.PayChoose hook (C# `CardPopipapapipopa.PayChoose` -> `Use`,
//   `p.amount = max(0, p.amount - 150 * n)`) and the same per-card crystal counter
//   as (1). Picking the count is now expressible (`ctx::ask_number`); the hook and
//   the crystals are not.
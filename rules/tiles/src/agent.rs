//! `tile:agent` -- [地产商] 的 [结算].
//!
//! 规则书（data/rules.txt, 「基础[结算]规则」, line 107）:
//! > · [地产商]的[结算]为：若与该格子同色的所有[可购买格子]均已属于其他玩家则需向该格子同色的所有[可购买格子]从前到后依次进行一次半价收费的[结算]（向上取整10），否则可选择该格子同色的无主或玩家拥有的[可购买格子]之一进行一次[结算]。
//!
//! and 「专有名词」:
//! > · [地产商]：标记为地产商的格子，每种颜色拥有一个地产商格子。
//!
//! The whole branch is one engine routine -- `H.AgentLanding`, exposed as
//! `ctx::agent_landing` -- so the body is a citation and a call. The engine
//! keeps it because it is a *prompting* routine over a colour group (the
//! 「从前到后依次」 order and the 「向上取整10」 half-charge rounding are its
//! bookkeeping); a card that reshapes the agent still talks to the same
//! primitive.
//!
//! TODO(规则书): 「同色」 is the board's `group` (see `data/board.json`); the
//! book never says whether `extraColor` / 「该格获得所有颜色」 widens it. The
//! half-charge's 「向上取整10」 is implemented as `ceil(full/2/10)*10` in
//! `pay_rent`, matching the C#; the book only says 「向上取整10」.

use card_sdk::ctx;
use card_sdk::{CardDef, On};

pub const AGENT: CardDef = CardDef::new("tile:agent", &[On::Settle(settle)]);

/// 规则书: 「[地产商]的[结算]为：若…则需向…依次进行一次半价收费的[结算]…，
/// 否则可选择…之一进行一次[结算]。」
fn settle(player_id: i32) -> card_sdk::Asked {
    let agent = ctx::self_tile().unwrap_or(-1);
    // 规则书: the whole branch (「若…则…否则…」) is `H.AgentLanding`. It raises
    // the same `pay` / `buy` / `build` triggers a plain landing would, so a
    // [反击] to those sees the agent's settle like any other. It logs its own
    // outcomes (`log.agent_none` / `log.agent_all_owned` / …) -- no extra
    // landing line, matching the built-in body.
    ctx::agent_landing(player_id, agent);
    Ok(())
}
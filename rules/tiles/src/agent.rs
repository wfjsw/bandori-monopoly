//! `tile:agent` -- [地产商] 的 [结算].
//!
//! 规则书（data/rules.txt, 「基础[结算]规则」, line 107）:
//! > · [地产商]的[结算]为：若与该格子同色的所有[可购买格子]均已属于其他玩家则需向该格子同色的所有[可购买格子]从前到后依次进行一次半价收费的[结算]（向上取整10），否则可选择该格子同色的无主或玩家拥有的[可购买格子]之一进行一次[结算]。
//!
//! and 「专有名词」:
//! > · [地产商]：标记为地产商的格子，每种颜色拥有一个地产商格子。
//!
//! The body is the 「若…则…否则…」 branch itself. The engine keeps a built-in
//! copy (`Play::agent_landing`, `land_at_built_in`'s `"agent"` arm) for
//! `StubRules` and for a tile with no rule instance bound -- the same shape
//! `tile:property` / `tile:ring` use (`docs/TILES.md`).
//!
//! 同色 is the board's `group` widened by the tile-prop colour overrides
//! (`prop::ANY_COLOR` 「该格获得所有颜色」 / `prop::colorFor:<p>`), via
//! `ctx::is_color`. An `ANY_COLOR`-only member is offered for the buy branch
//! but never for the build branch (soyo 「该格本身不可因自有以外的颜色的地产商盖房」).
//!
//! TODO(规则书): 「同色」 is the board's `group` (see `data/board.json`); the
//! book never says whether the per-player colour overrides (`anyColor` /
//! `colorFor:<p>`) / 「该格获得所有颜色」 widens it. The
//! half-charge's 「向上取整10」 is implemented as `ceil(full/2/10)*10` in
//! `pay_rent`, matching the C#; the book only says 「向上取整10」.

use alloc::vec::Vec;

use card_sdk::abi::BuyKind;
use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

pub const AGENT: CardDef = CardDef::new("tile:agent", &[On::Settle("", None, settle)]);

/// 规则书: 「[地产商]的[结算]为：若…则需向…依次进行一次半价收费的[结算]…，
/// 否则可选择…之一进行一次[结算]。」
fn settle(player_id: i32) -> card_sdk::Asked {
    let agent = ctx::self_tile().unwrap_or(-1);
    if agent < 0 {
        return Ok(());
    }
    // The 同色 set (`docs/PURCHASE.md`): the agent's own `group`, widened by
    // `prop::ANY_COLOR` / `prop::colorFor:<player>` (`ctx::is_color`). An
    // `ANY_COLOR`-only member (its own group is not the agent's) is buyable
    // here but never buildable -- soyo 「该格本身不可因自有以外的颜色的地产商盖房」.
    let group = ctx::tile_group(agent);
    let mut set: Vec<(i32, bool)> = Vec::new();
    for t in 0..ctx::tile_count() {
        if !ctx::is_buyable(t) || !ctx::is_color(player_id, t, group) {
            continue;
        }
        // Buildable only in the agent's own group.
        set.push((t, ctx::tile_group(t) == group));
    }
    let same: Vec<i32> = set.iter().map(|&(t, _)| t).collect();
    if same.is_empty() {
        ctx::log(
            player_id,
            &Msg::new("log.agent_none")
                .player_id("who", player_id)
                .tile("agent", agent),
        );
        return Ok(());
    }
    // 规则书: 「若与该格子同色的所有[可购买格子]均已属于其他玩家则需向该格子
    // 同色的所有[可购买格子]从前到后依次进行一次半价收费的[结算]（向上取整10）」
    // -- 「属于其他玩家」 excludes both unowned and the mover's own deeds.
    // 「从前到后」 is board order (`same` is tile-index order).
    if same
        .iter()
        .all(|&t| ctx::tile_owner(t) >= 0 && ctx::tile_owner(t) != player_id)
    {
        ctx::log(
            player_id,
            &Msg::new("log.agent_all_owned")
                .player_id("who", player_id)
                .tile("agent", agent)
                .i("n", same.len() as i64),
        );
        for t in same {
            // 「半价收费的[结算]（向上取整10）」 -- `H.PayRent` with the half
            // flag; the ceil-10 rounding lives in the rent pipeline.
            ctx::pay_rent(player_id, t, true)?;
        }
        return Ok(());
    }
    // 规则书: 「否则可选择该格子同色的无主或玩家拥有的[可购买格子]之一进行
    // 一次[结算]。」 -- one offer over the unowned (buy) and own (build) members.
    let quotes = ctx::buy_quotes(player_id, BuyKind::Agent as i32, &same);
    let mut options: Vec<i32> = Vec::new();
    let mut labels: Vec<Msg> = Vec::new();
    let mut prices: Vec<i32> = Vec::new();
    for (idx, &(t, buildable)) in set.iter().enumerate() {
        let owner = ctx::tile_owner(t);
        if owner < 0 {
            // `buy_quotes` is parallel to `same`.
            let (price, eligible) = quotes.get(idx).copied().unwrap_or((-1, false));
            let price = price.max(0);
            if ctx::can_pay(player_id) && eligible {
                let houses = ctx::houses_of(t);
                options.push(t);
                prices.push(price);
                let mut label = Msg::new("ask.agent.buy")
                    .tile("tile", t)
                    .n("price", price as i64);
                if houses > 0 {
                    label = label.msg(
                        "extra",
                        &Msg::new("ask.part.incl_houses").i("h", houses as i64),
                    );
                }
                labels.push(label);
            }
        } else if owner == player_id && buildable && ctx::can_build_on(player_id, t) {
            let cost = ctx::build_cost(t);
            options.push(t);
            prices.push(cost);
            labels.push(
                Msg::new("ask.agent.build")
                    .tile("tile", t)
                    .i("nth", ctx::houses_of(t) as i64 + 1)
                    .n("cost", cost as i64),
            );
        }
    }
    if options.is_empty() {
        ctx::log(
            player_id,
            &Msg::new("log.agent_nothing")
                .player_id("who", player_id)
                .tile("agent", agent),
        );
        return Ok(());
    }
    // The ask carries the per-option labels, their prices, and the AI's
    // preferred index (`H.AgentLanding`'s `ai_agent_choice`). It answers
    // with the **tile id** (`PromptKind::TileId`), so the commit-pass re-run
    // of this body -- whose `set` the just-committed host effect reshaped --
    // still maps the pick to the same tile.
    let ai = ctx::ai_agent_choice(player_id, &options);
    let t = ctx::ask_tiles(
        player_id,
        &Msg::new("ask.agent.title").tile("agent", agent),
        &Msg::new("ask.agent.text").player_id("who", player_id),
        &options,
        &labels,
        &prices,
        ai,
    )?;
    if t < 0 {
        // The prompt's 「不选」 slot (`fallback = options.len()`).
        ctx::log(
            player_id,
            &Msg::new("log.agent_skip").player_id("who", player_id),
        );
        return Ok(());
    }
    // The chosen branch settles once -- 「进行一次[结算]」. `H.AgentOffer`
    // runs the buy or the build as one host routine, so the commit pass's
    // pre-filled answer cannot double-act.
    let kind = if ctx::tile_owner(t) < 0 { 0 } else { 1 };
    ctx::agent_offer(player_id, agent, t, kind);
    Ok(())
}
//! `event:弦卷集团地产开发` -- 事件卡「弦卷集团地产开发」（中立事件 A3）.
//!
//! 事件文本（data/events.json, id `弦卷集团地产开发`）:
//! > 随机指定一个可购买格子（如果有）然后所有玩家进行竞价，出价最大的玩家支付选择数量的资金并获得该格子的地契

use alloc::vec::Vec;

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::{all_players, roll};

pub const TSURUMAKI_ESTATE: CardDef =
    CardDef::new("event:弦卷集团地产开发", &[On::Play(None, play)]);

/// 规则书: 「随机指定一个可购买格子（如果有）」 -- one unowned buyable tile,
/// chosen uniformly by a bare 1dN. 「如果有」 is the empty-pool no-op below.
fn play(player_id: i32) -> card_sdk::Asked {
    let mut open: Vec<i32> = Vec::new();
    for t in 0..ctx::tile_count() {
        if ctx::is_buyable(t) && ctx::tile_owner(t) < 0 {
            open.push(t);
        }
    }
    if open.is_empty() {
        ctx::log(player_id, &Msg::new("log.event.estate_none"));
        return Ok(());
    }
    // 1..=len from the match RNG, mapped down to a 0-based index.
    let face = roll(player_id, 1, open.len() as i32);
    let tile = open[(face - 1).clamp(0, open.len() as i32 - 1) as usize];
    ctx::log(player_id, &Msg::new("log.event.estate_tile").tile("tile", tile));

    // 规则书: 「然后所有玩家进行竞价，出价最大的玩家支付选择数量的资金并获得
    // 该格子的地契」 -- sealed high-bid: everyone states a bid in turn order, the
    // highest pays exactly that bid and takes the deed.
    // TODO(规则书)/TODO(engine): 「竞价」 is a real raise-and-bend auction in the
    // engine's internal `auction_leftovers`; that machinery is not exposed to
    // card bodies (`ctx` has no auction primitive), so this is one sealed bid
    // per player, not an open auction.
    let players = all_players();
    if players.is_empty() {
        return Ok(());
    }
    let mut best_bid = -1;
    let mut best_p = -1;
    for &p in &players {
        let max = ctx::money_of(p).max(0);
        // `ask_number` builds one option per integer in the range; a money
        // range is far too wide for that, so the bid candidates are 0 / every
        // 1000 up to `max` / `max` itself (the game's money unit is 1000 --
        // 「支付X次1000资金」 elsewhere).
        // TODO(规则书)/TODO(engine): 「竞价」 should let a player name any
        // amount. There is no `ask_money`; `ask_number` is an `ask_pick` over
        // the whole range. A free-amount ask is the missing surface.
        let mut steps: Vec<i32> = Vec::new();
        steps.push(0);
        let mut v = 1000;
        while v < max {
            steps.push(v);
            v += 1000;
        }
        if max > 0 && steps.last() != Some(&max) {
            steps.push(max);
        }
        let options: Vec<Msg> = steps
            .iter()
            .map(|&b| Msg::new("ask.intOption").i("n", b as i64))
            .collect();
        let pick = ctx::ask_pick(
            p,
            &Msg::new("log.event.estate_bid_title").tile("tile", tile),
            &Msg::new("log.event.estate_bid_ask").tile("tile", tile).n("n", max as i64),
            &options,
        )?;
        let bid = steps[pick.min(steps.len() - 1)];
        ctx::log(
            player_id,
            &Msg::new("log.event.estate_bid").player_id("who", p).n("n", bid as i64),
        );
        // TODO(规则书): 「出价最大」 does not break a tie. Earliest seat in turn
        // order keeps the deed here.
        if bid > best_bid {
            best_bid = bid;
            best_p = p;
        }
    }
    if best_p < 0 {
        return Ok(());
    }
    // 「支付选择数量的资金」 -- the winner's own bid, not the tile's price.
    if best_bid > 0 {
        ctx::pay(best_p, best_bid, &Msg::new("log.event.estate_pay"))?;
    }
    // 「获得该格子的地契」 -- `raise_bought` commits the transfer on the live
    // world *and* announces it (`docs/CARDS.md`). A separate `set_owner` on the
    // run's world copy would be re-executed on every replay of this body with a
    // possibly-different pick (the `open` list shrinks once `raise_bought` has
    // set an owner), handing out a second deed. The announcement is the transfer.
    ctx::raise_bought(best_p, tile);
    ctx::log(
        player_id,
        &Msg::new("log.event.estate_win")
            .player_id("who", best_p)
            .tile("tile", tile)
            .n("n", best_bid as i64),
    );
    Ok(())
}
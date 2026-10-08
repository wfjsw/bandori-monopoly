//! `event:卡池BUG` -- 事件卡「卡池BUG」（中立事件 A14）.
//!
//! 事件文本（data/events.json, id `卡池BUG`）:
//! > 将此卡放置于场地中央并将一张“[衍生]修复公告”背面朝上放置于事件牌堆顶部，下次触发事件时将此卡放入事件弃牌。此卡在场时不可在造价1500及以上的格子上加盖房屋。

use card_sdk::abi::{prop, HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{CardDef, Msg, On};

use crate::util::{expire, keep};

const ID: &str = "卡池BUG";

/// The derived event this one seeds onto the deck.
const FIX_NOTE: &str = "修复公告";

pub const POOL_BUG: CardDef = CardDef::new(
    "event:卡池BUG",
    &[
        On::Play(None, play, ""),
        On::Hook(&[HookKind::Event], Some(always), on_event, ""),
        On::Hook(&[HookKind::BuildBefore], Some(always), before_build, ""),
    ],
);

fn always(_player_id: i32) -> bool {
    true
}

/// 规则书: 「将此卡放置于场地中央并将一张“[衍生]修复公告”背面朝上放置于事件
/// 牌堆顶部」 -- keep the event and seed the derived card face-down on top of the
/// event deck (`docs/EVENTS.md`).
/// 规则书: 「此卡在场时不可在造价1500及以上的格子上加盖房屋」 -- arm
/// `prop::NO_BUILD_ABOVE` on this instance so `why_not_build_on` refuses the
/// build before the player is even offered it (the `BuildBefore` hook below is
/// the belt to that brace).
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    ctx::event_deck_push(FIX_NOTE, true);
    ctx::set_prop(prop::NO_BUILD_ABOVE, 1500);
    ctx::log(
        player_id,
        &Msg::new("log.event.pool_bug_on").card("event", ID),
    );
    Ok(())
}

/// 规则书: 「下次触发事件时将此卡放入事件弃牌」 -- the next event draw (which is
/// the 修复公告 just seeded) files this one away. The `event` raise for our own
/// draw happens before we are bound, so this hook sees only later draws.
fn on_event(_owner: i32) -> card_sdk::Asked {
    expire(ID);
    Ok(())
}

/// 规则书: 「此卡在场时不可在造价1500及以上的格子上加盖房屋」 -- refuse the build
/// outright when the tile's house cost is 1500 or more.
fn before_build(_owner: i32) -> card_sdk::Asked {
    let t = trigger::tile();
    if t < 0 {
        return Ok(());
    }
    // 「造价1500及以上」 -- the tile's build cost per level (`TileData.house`).
    if ctx::build_cost(t) < 1500 {
        return Ok(());
    }
    trigger::set_cancelled();
    ctx::log(
        trigger::player_id(),
        &Msg::new("log.event.pool_bug_ban_build")
            .player_id("who", trigger::player_id())
            .tile("tile", t),
    );
    Ok(())
}
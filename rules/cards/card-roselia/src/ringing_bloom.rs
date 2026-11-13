//!
//! 规则书（docs/rulebook/cards.json, id `R:（燐子）Ringing Bloom`）:
//! > （燐子）Ringing Bloom：
//! >
//! > （1）将此卡放置于自身场上
//!
//! > （2）你的所有格子上的房屋数视为与你房屋数最多的格子等同，但受此效果影响获得额外房屋数的格子收费减半
//!
//! > （3）你的任意非RiNG格子收费后，此卡置入弃牌堆，然后你获得500*X资金，X为你收费格上的房屋数。
//!
//! place this card on your field; it then fakes your house counts up to your
//! best tile and pays out when one of your non-RiNG tiles collects rent.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{prop, ActionKind, TriggerKind};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

pub const RINGING_BLOOM: CardDef = CardDef::new(
    "R:（燐子）Ringing Bloom",
    &[
        On::Play("", None, play),
        On::Action(Action::PayAfter(
            "card.placed && pay_is_rent && target == owner",
            None,
            pay_after)),
        // 规则书（2）: 「视为」 is continuous -- it tracks the max as houses move,
        // so the fall re-runs the same view as the rise. The write may come from
        // any seat; what matters is that the tile is the owner's (`actor` is the
        // raiser, so it cannot be used here).
        On::Action(Action::HouseAdded(
            "card.placed && tile.owner == owner",
            None,
            recompute_view)),
        On::Action(Action::HouseRemoved(
            "card.placed && tile.owner == owner",
            None,
            recompute_view)),
        // 规则书（2）: 「你的所有格子」 -- a tile acquired while the card is placed
        // is lifted by the owner-wide `RENT_HOUSES` view too, so its half-charge
        // is armed like a rise (`bought` carries the buyer as actor).
        On::Action(Action::Bought(
            "card.placed && actor == owner",
            None,
            recompute_view)),
        // 规则书（2）: the 「视为」 view and every half-charge end wherever the
        // card goes -- a bounce, a banish, a deck return and the card's own
        // discard alike. `zoneLeft` runs on the still-live row (G08 I3), so
        // `card.placed` answers here. Silent: it is not a question.
        On::Action(Action::ZoneLeft("card.placed", None, on_zone_left)).silent(),
        // 规则书（2）: a `set_owner` gift moves a tile out of 「你的格子」 and out
        // of 「受此效果影响获得额外房屋数的格子」, so the half must follow the
        // owner. `ownerSet` is an effect; the hook runs only after the
        // counteraction round has let the write land (a cancelled write runs
        // no hook). Silent.
        On::Action(Action::Effect(
            "card.placed",
            Some(owner_set_guard),
            on_owner_set,
        ))
        .silent(),
        // The card's own discard, kept for the counted view (an idempotent
        // backstop to the `zoneLeft` hook); the half-charges are the hook's.
        // The raise names the discarded card; `card.placed` is false here.
        On::Action(Action::Discarded("trigger_card == card.id", None, on_discarded)).silent(),
    ],
)
.props(&[(card_sdk::abi::prop::SONG, 1)])
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"Ringing Bloom"#)
        .raw_name(r#"（燐子）Ringing Bloom"#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"将此卡放置于自身场上"#),
            it(r#"你的所有格子上的房屋数视为与你房屋数最多的格子等同，但受此效果影响获得额外房屋数的格子收费减半"#),
            it(r#"你的任意非RiNG格子收费后，此卡置入弃牌堆，然后你获得500*X资金，X为你收费格上的房屋数。"#),
        ]))
        // sheet cell E12
        .fields(&[("group", r#"Roselia"#), ("owner", r#"白金燐子"#)]),
);

const ID: &str = "R:（燐子）Ringing Bloom";

/// 规则书（2）: 「你的所有格子上的房屋数视为与你房屋数最多的格子等同」 --
/// the *counted* rent-house-count is the owner's max. 「视为」 is virtual:
/// real `houses` must not change (build caps, raze, sale and the count after
/// this card leaves all still see the standing houses). The override is
/// `prop::RENT_HOUSES` on **this placed instance** (`ctx::set_prop`), so it
/// ends the moment the card leaves the field.
fn counted_max(player_id: i32) -> i32 {
    let mut max_h = 0i32;
    for &t in &ctx::owned_tiles(player_id) {
        if ctx::is_deed(t) {
            continue;
        }
        max_h = max_h.max(ctx::houses_of(t));
    }
    max_h
}

/// Which of the owner's non-RiNG tiles this effect is lifting (「受此效果影响
/// 获得额外房屋数的格子」) -- real count below the counted max.
fn raised_tiles(player_id: i32, max_h: i32) -> alloc::vec::Vec<i32> {
    let mut raised = alloc::vec::Vec::new();
    if max_h <= 0 {
        return raised;
    }
    for &t in &ctx::owned_tiles(player_id) {
        if ctx::is_deed(t) {
            continue;
        }
        if ctx::houses_of(t) < max_h {
            raised.push(t);
        }
    }
    raised
}

fn play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, ID, &Msg::new(key!("ringing_bloom_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("ringing_bloom_placed")).player_id("who", player_id),
    );
    // 规则书（2）: the 「视为」 view.
    apply_view(player_id);
    Ok(())
}

/// 规则书（2）: 「你的所有格子上的房屋数视为与你房屋数最多的格子等同，但受此
/// 效果影响获得额外房屋数的格子收费减半」 -- recompute the counted max and the
/// half-charge set. Called at play, on every `HouseAdded`, `HouseRemoved` and
/// `Bought`, so the 「视为」 tracks the max as houses move either way and
/// reaches newly acquired tiles.
///
/// Ownership leaving the owner does not go through here: `zoneLeft` disarms
/// every half when the card leaves, and an `ownerSet` re-runs this view against
/// the post-write tile set. TODO(规则书)（2）: a plain **sale** uses
/// `assign_deed`→`set_tile_owner` with no effect event, so a sold tile keeps
/// its half until the next view (play/build/raze). Not claimed covered.
fn apply_view(player_id: i32) {
    let max_h = counted_max(player_id);
    // Written unconditionally: a max that drops back to 0 (every house razed)
    // must clear the view too, not leave the last count standing.
    ctx::set_prop(prop::RENT_HOUSES, max_h);
    // Reconcile the whole set, not just the rise: a tile that has caught up with
    // the max is no longer 「受此效果影响获得额外房屋数」 and must charge in full.
    for &t in &ctx::owned_tiles(player_id) {
        if ctx::is_deed(t) {
            continue;
        }
        ctx::set_tile_prop(t, prop::RENT_FACTOR, 0);
    }
    for &t in &raised_tiles(player_id, max_h) {
        ctx::set_tile_prop(t, prop::RENT_FACTOR, 500);
    }
}

/// 规则书（2）: 「视为」 tracks the max as houses move (either direction) and
/// covers a tile acquired mid-life.
fn recompute_view(player_id: i32) -> card_sdk::Asked {
    apply_view(player_id);
    Ok(())
}

/// The card left the field through `zoneLeft` -- the one hook every removal
/// route crosses. The 「视为」 prop rides this instance and dies with it; the
/// half-charges are board-tile props and are cleared here by hand. The row is
/// still live at `zoneLeft` (G08 I3), so `owned_tiles` reads the same set the
/// view armed. Nothing is recomputed -- the card is gone.
fn on_zone_left(player_id: i32) -> card_sdk::Asked {
    // Instance kind: every placed copy hears this raise; only the leaving
    // row's own view ends here.
    if trigger::instance() != ctx::self_uid() {
        return Ok(());
    }
    ctx::set_prop(prop::RENT_HOUSES, 0);
    for &t in &ctx::owned_tiles(player_id) {
        if ctx::is_deed(t) {
            continue;
        }
        ctx::set_tile_prop(t, prop::RENT_FACTOR, 0);
    }
    // A second copy of this player's is still in play and keeps the same view
    // in force: the clearing above is whole-board for the seat, not per copy.
    let second_copy = ctx::field_instances(player_id)
        .iter()
        .any(|&(uid, card)| uid != ctx::self_uid() && card == ctx::intern(ID));
    if second_copy {
        apply_view(player_id);
    }
    Ok(())
}

/// The card's own discard: the counted view ends too (idempotent with the
/// `zoneLeft` hook). The half-charges belong to that hook. The raise names the
/// card (`-1` instance on a name-only discard), so a surviving copy of the
/// same card would otherwise clear its own view here.
fn on_discarded(_player_id: i32) -> card_sdk::Asked {
    if trigger::instance() != ctx::self_uid() {
        return Ok(());
    }
    ctx::set_prop(prop::RENT_HOUSES, 0);
    Ok(())
}

/// Effect index `i`'s owner change as `(tile, new owner)`, if it is an
/// `ownerSet` with a tile.
fn owner_set(i: i32) -> Option<(i32, i32)> {
    let t = ctx::effect::tile(i);
    (t >= 0 && ctx::effect::kind(i) == TriggerKind::OwnerSet).then(|| (t, ctx::effect::value(i)))
}

/// Does any declared `ownerSet` move a tile to or away from this owner? The
/// write is applied only after the trigger settles, so `tile_owner` is still
/// the pre-write owner: `== owner` is a loss, `value == owner` a gain.
fn owner_set_guard(owner: i32) -> bool {
    (0..ctx::effect::count()).any(|i| {
        owner_set(i).is_some_and(|(t, to)| {
            if to == owner {
                ctx::tile_owner(t) != owner
            } else {
                ctx::tile_owner(t) == owner
            }
        })
    })
}

/// 规则书（2）: an `ownerSet` changes which tiles are 「你的格子」 and which of
/// them the view lifts. Run the view against the **post-write** set (the lost
/// tiles dropped, the gained ones added); the write itself lands after the
/// hook, so neither list is in `owned_tiles` yet.
fn on_owner_set(owner: i32) -> card_sdk::Asked {
    let mut gained = alloc::vec::Vec::new();
    let mut lost = alloc::vec::Vec::new();
    for i in 0..ctx::effect::count() {
        let Some((t, to)) = owner_set(i) else {
            continue;
        };
        if to == owner {
            gained.push(t);
        } else if ctx::tile_owner(t) == owner {
            lost.push(t);
        }
    }
    for &t in &lost {
        ctx::set_tile_prop(t, prop::RENT_FACTOR, 0);
    }
    let owned: alloc::vec::Vec<i32> = ctx::owned_tiles(owner)
        .into_iter()
        .filter(|t| !lost.contains(t))
        .chain(gained)
        .filter(|&t| !ctx::is_deed(t))
        .collect();
    let max_h = owned.iter().map(|&t| ctx::houses_of(t)).max().unwrap_or(0);
    ctx::set_prop(prop::RENT_HOUSES, max_h);
    for &t in &owned {
        let factor = if ctx::houses_of(t) < max_h { 500 } else { 0 };
        ctx::set_tile_prop(t, prop::RENT_FACTOR, factor);
    }
    Ok(())
}

/// Runs through the hook dispatch, so this is a field effect, not a [反击]. Pure
/// guard for [`pay_after`] -- the activation gate. `false` means the card is not
/// activated at all.
fn pay_after(player_id: i32) -> card_sdk::Asked {
    // TODO(规则书)（你的任意非RiNG格子收费后）: 「收费后」 can also be read as a pay that collected
    // nothing (fully counteracted, or a blocked payee) -- the engine still raises
    // `payAfter` there with `value` 0, and we discard and pay out on it anyway (in a
    // reflected rent only the 0-value parent pay carries `target == owner`, so the
    // card can never fire on the real leg).
    let tile = trigger::tile();
    if tile < 0 || ctx::is_deed(tile) {
        return Ok(());
    }
    // 规则书（3）: 「X为你收费格上的房屋数」 -- the **counted** value (「视为」),
    // i.e. what the rent table was read at, not the standing houses. A read of
    // the standing houses makes X = 0 on the very tiles (2) lifted. Read before
    // `set_dest`, while the card and its counted view are still live.
    let x = ctx::rent_houses_of(tile);
    // The disarm is not repeated here: the leave crosses `zoneLeft`, which
    // clears the counted view and every half-charge for *all* removal routes.
    ctx::set_dest(ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
    ctx::log(
        player_id,
        &Msg::new(key!("ringing_bloom_done"))
            .player_id("who", player_id)
            .tile("tile", tile),
    );
    if x > 0 {
        ctx::gain(
            player_id,
            500 * x,
            &Msg::new(key!("ringing_bloom_pay"))
                .player_id("who", player_id)
                .tile("tile", tile)
                .i("n", x as i64),
        )?;
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_after_kani(player_id: i32) -> card_sdk::Asked {
    pay_after(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_house_added_kani(player_id: i32) -> card_sdk::Asked {
    recompute_view(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn recompute_view_kani(player_id: i32) -> card_sdk::Asked {
    recompute_view(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_discarded_kani(player_id: i32) -> card_sdk::Asked {
    on_discarded(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_zone_left_kani(player_id: i32) -> card_sdk::Asked {
    on_zone_left(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_owner_set_kani(player_id: i32) -> card_sdk::Asked {
    on_owner_set(player_id)
}

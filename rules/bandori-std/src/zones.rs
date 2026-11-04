//! Zone-family sugar (design G08 §5.6).
//!
//! The generic vocabulary (`Zone`, `ZoneChange`, `ZoneChangeCause`, the
//! reader `ctx::trigger::zone()`, the move verbs) lives in `card_sdk`. What is
//! a rule of *this* game -- the leave-destination prop, the 「未生效」 flag and
//! the 同一回合内打出两张 idiom -- lives here.

pub use card_sdk::abi::{Zone, ZoneChangeCause};

use card_sdk::ctx;
use card_sdk::ctx::Dest;

/// The field-leave destination a card sets from its own `zoneLeft` body
/// (`prop::LEAVE_DEST`), read back by the writer at commit.
pub fn set_leave_destination(dest: Dest) {
    ctx::set_prop(card_sdk::abi::prop::LEAVE_DEST, dest as i32);
}

/// The destination set by [`set_leave_destination`] (Graveyard when unset).
pub fn leave_destination() -> Dest {
    Dest::from_i32(ctx::prop(card_sdk::abi::prop::LEAVE_DEST))
}

/// 「视为此卡未生效」 -- the waste flag over `ctx::set_ineffective_at` on the
/// running instance. A waste discard carries it into `discarded`.
pub fn set_ineffective() {
    ctx::set_ineffective_at(ctx::self_uid(), true);
}

/// Did this instance take effect?
pub fn is_ineffective() -> bool {
    ctx::is_ineffective_at(ctx::self_uid())
}

/// 「同一回合内打出两张」-style card-id match over the raise's card.
pub fn played_card_is(id: &str) -> bool {
    ctx::trigger::card_is(id)
}

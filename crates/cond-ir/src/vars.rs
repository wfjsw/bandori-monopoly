//! Variable ids. **Checked against `rules-cond`'s `VOCAB`** by
//! `rules-cond/tests/cond_ir_vocab.rs` (`ir_vars_match_vocab`): adding a VOCAB
//! scalar without extending this table fails that test, so a name cannot
//! silently skip the Rust side.

/// How a scalar binds.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VarTy {
    Int,
    Bool,
    /// `-1` in the raw i64 means CEL `null`.
    OptInt,
    /// A CEL string scalar. The IR stores `id_of(spelling)` in the slot (the
    /// same FNV hash string literals bake to); `Eq` compares hashes. No heap,
    /// no string pool -- the established `counter_name` / `counter_is` pattern.
    Str,
}

/// Variable index. The discriminant is the `IrCtx` slot.
///
/// Five owner scalars that were content sugar died with the vocabulary split:
/// they lower to `Call(State, [Var(Owner), key])` and live in `IrCtx::state`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum VarId {
    Kind = 0,
    Actor,
    Target,
    Value,
    Step,
    By,
    PayIsRent,
    RollSource,
    Abnormal,
    TurnPlayer,
    TurnKey,
    PlanFixedRoll,
    TriggerCard,
    CounterName,
    TileId,
    TileOwner,
    TileHouses,
    TileMortgaged,
    TilePrice,
    MoveRoll,
    MoveKind,
    MoveRemaining,
    MoveMain,
    MoveBySkill,
    MoveDir,
    EffectCount,
    Owner,
    OwnerId,
    OwnerMoney,
    OwnerHand,
    OwnerPos,
    OwnerOut,
    OwnerStay,
    OwnerStun,
    OwnerExile,
    OwnerTiles,
    CardId,
    CardPlaced,
    CardTile,
}

pub const SCALAR_COUNT: usize = 39;

/// Flat identifier + type, in `VarId` order. Mirrors `vocab::VOCAB` scalars
/// (Window + Candidate, non-Func).
pub const VARS: &[(VarId, &str, VarTy)] = &[
    (VarId::Kind, "kind", VarTy::Str),
    (VarId::Actor, "actor", VarTy::Int),
    (VarId::Target, "target", VarTy::Int),
    (VarId::Value, "value", VarTy::Int),
    (VarId::Step, "step", VarTy::Int),
    (VarId::By, "by", VarTy::Int),
    (VarId::PayIsRent, "pay_is_rent", VarTy::Bool),
    (VarId::RollSource, "roll_source", VarTy::Int),
    (VarId::Abnormal, "abnormal", VarTy::Bool),
    (VarId::TurnPlayer, "turn_player", VarTy::Int),
    (VarId::TurnKey, "turn_key", VarTy::Int),
    (VarId::PlanFixedRoll, "plan_fixed_roll", VarTy::OptInt),
    (VarId::TriggerCard, "trigger_card", VarTy::Int),
    (VarId::CounterName, "counter_name", VarTy::Int),
    (VarId::TileId, "tile_id", VarTy::Int),
    (VarId::TileOwner, "tile_owner", VarTy::Int),
    (VarId::TileHouses, "tile_houses", VarTy::Int),
    (VarId::TileMortgaged, "tile_mortgaged", VarTy::Int),
    (VarId::TilePrice, "tile_price", VarTy::Int),
    (VarId::MoveRoll, "move_roll", VarTy::OptInt),
    (VarId::MoveKind, "move_kind", VarTy::OptInt),
    (VarId::MoveRemaining, "move_remaining", VarTy::Int),
    (VarId::MoveMain, "move_main", VarTy::Bool),
    (VarId::MoveBySkill, "move_by_skill", VarTy::Bool),
    (VarId::MoveDir, "move_dir", VarTy::Int),
    (VarId::EffectCount, "effect_count", VarTy::Int),
    (VarId::Owner, "owner", VarTy::Int),
    (VarId::OwnerId, "owner_id", VarTy::Int),
    (VarId::OwnerMoney, "owner_money", VarTy::Int),
    (VarId::OwnerHand, "owner_hand", VarTy::Int),
    (VarId::OwnerPos, "owner_pos", VarTy::Int),
    (VarId::OwnerOut, "owner_out", VarTy::Int),
    (VarId::OwnerStay, "owner_stay", VarTy::Int),
    (VarId::OwnerStun, "owner_stun", VarTy::Int),
    (VarId::OwnerExile, "owner_exile", VarTy::Int),
    (VarId::OwnerTiles, "owner_tiles", VarTy::Int),
    (VarId::CardId, "card_id", VarTy::Int),
    (VarId::CardPlaced, "card_placed", VarTy::Bool),
    (VarId::CardTile, "card_tile", VarTy::Int),
];

/// Look up a flat name (after `root.field` rewrite).
pub fn by_flat(name: &str) -> Option<(u16, VarTy)> {
    VARS.iter()
        .find(|(_, f, _)| *f == name)
        .map(|(id, _, ty)| (*id as u16, *ty))
}

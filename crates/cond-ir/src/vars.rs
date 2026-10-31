//! Variable ids. **Checked against `rules-cond`'s `VOCAB`** by
//! `rules-cond/tests/cond_ir_vocab.rs` (`ir_vars_match_vocab`): adding a VOCAB
//! scalar without extending this table fails that test, so a name cannot
//! silently skip the Rust side.

/// How a scalar binds.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VarType {
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
pub const VARS: &[(VarId, &str, VarType)] = &[
    (VarId::Kind, "kind", VarType::Str),
    (VarId::Actor, "actor", VarType::Int),
    (VarId::Target, "target", VarType::Int),
    (VarId::Value, "value", VarType::Int),
    (VarId::Step, "step", VarType::Int),
    (VarId::By, "by", VarType::Int),
    (VarId::PayIsRent, "pay_is_rent", VarType::Bool),
    (VarId::RollSource, "roll_source", VarType::Int),
    (VarId::Abnormal, "abnormal", VarType::Bool),
    (VarId::TurnPlayer, "turn_player", VarType::Int),
    (VarId::TurnKey, "turn_key", VarType::Int),
    (VarId::PlanFixedRoll, "plan_fixed_roll", VarType::OptInt),
    (VarId::TriggerCard, "trigger_card", VarType::Int),
    (VarId::CounterName, "counter_name", VarType::Int),
    (VarId::TileId, "tile_id", VarType::Int),
    (VarId::TileOwner, "tile_owner", VarType::Int),
    (VarId::TileHouses, "tile_houses", VarType::Int),
    (VarId::TileMortgaged, "tile_mortgaged", VarType::Int),
    (VarId::TilePrice, "tile_price", VarType::Int),
    (VarId::MoveRoll, "move_roll", VarType::OptInt),
    (VarId::MoveKind, "move_kind", VarType::OptInt),
    (VarId::MoveRemaining, "move_remaining", VarType::Int),
    (VarId::MoveMain, "move_main", VarType::Bool),
    (VarId::MoveBySkill, "move_by_skill", VarType::Bool),
    (VarId::MoveDir, "move_dir", VarType::Int),
    (VarId::EffectCount, "effect_count", VarType::Int),
    (VarId::Owner, "owner", VarType::Int),
    (VarId::OwnerId, "owner_id", VarType::Int),
    (VarId::OwnerMoney, "owner_money", VarType::Int),
    (VarId::OwnerHand, "owner_hand", VarType::Int),
    (VarId::OwnerPos, "owner_pos", VarType::Int),
    (VarId::OwnerOut, "owner_out", VarType::Int),
    (VarId::OwnerStay, "owner_stay", VarType::Int),
    (VarId::OwnerStun, "owner_stun", VarType::Int),
    (VarId::OwnerExile, "owner_exile", VarType::Int),
    (VarId::OwnerTiles, "owner_tiles", VarType::Int),
    (VarId::CardId, "card_id", VarType::Int),
    (VarId::CardPlaced, "card_placed", VarType::Bool),
    (VarId::CardTile, "card_tile", VarType::Int),
];

/// Look up a flat name (after `root.field` rewrite).
pub fn by_flat(name: &str) -> Option<(u16, VarType)> {
    VARS.iter()
        .find(|(_, f, _)| *f == name)
        .map(|(id, _, ty)| (*id as u16, *ty))
}

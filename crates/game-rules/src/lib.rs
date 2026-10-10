//! Host for the card ruleset -- one `.wasm` module per card -- executed with **wasmi** on both the
//! dedicated server (native) and the browser (inside our own wasm32 build).
//!
//! # Deterministic replay instead of suspension
//!
//! WebAssembly has no portable way to suspend a running effect at a prompt and
//! resume it later, so we don't: an effect runs from the start against a
//! **copy** of the world. If it reaches a prompt that has no answer in the
//! effect's answer log, the run is aborted and the copy -- with every side
//! effect the run made -- is thrown away. When the player answers, the effect
//! runs again from the same snapshot (RNG included), so dice re-roll identically
//! and it gets past the prompt this time. Only a run that finishes is committed.
//!
//! This covers every prompt shape the cards need, including the 14 classes whose
//! options depend on dice or loop state, and counteractions (another player's
//! counteraction is one more entry in the answer log). Cost: an effect with *k*
//! prompts runs *k + 1* times; the deepest effect in the shipped data asks 6
//! times in one run.
//!
//! See `docs/P0-FINDINGS.md` for why this replaced the CEL + Lua design.

mod host;
pub mod hostfns;
pub mod inline;
/// The `bandori_*` host-import shims (native only). Lives here, not in
/// `rules-native`, so every native binary that links `card-sdk` with `guest`
/// on -- including a server test binary in a unified `cargo test` -- resolves
/// the symbols (`docs/BOT.md` B1, the `LNK2019` fix).
#[cfg(not(target_arch = "wasm32"))]
pub mod native_shims;
mod wasm_rules;
mod world;

/// G0/G2 guard prefilter (docs/GUARDS.md): the serialized condition per
/// guarded entry and the one shared `admits()` every guard call site goes
/// through.
pub mod cond_pre;

pub use card_sdk::abi::{AbKind, CardPile, MoveKind, PromptKind, TriggerKind, ABI_VERSION};
pub use cond_pre::{
    admits, admits_gate, admits_pre, CompiledPre, PrecompiledCond, PrecompiledConds,
    PRECOMPILED_CONDS_VERSION,
};
pub use game_core::msg::Msg;
pub use host::{
    Call, CallOut, CardInfo, CardModules, GuestMem, HookRun, HostCtx, HostErr, HostRequest,
    HostState, Outcome, Prompt, PromptOption, RuleError, Ruleset, RulesetBuilder, RulesHandle,
    MAX_NESTING,
};
/// Simulation-mode inline answers (`docs/BOT.md` §3.2).
pub use inline::{with_inline_host, InlineHost};
/// Shared with `rules-native`: the engine-side `Msg` decoder and the per-run
/// fuel budget the nested-call plumbing shares.
pub use host::{engine_msg, DEFAULT_FUEL};
pub use wasm_rules::{RulesBridge, Run, WasmRules};
mod trigger_why;
pub use world::{CardWorld, Trigger, TriggerBuy, TriggerMove, TriggerPay};
/// The 「效果适用」 why-clause builder.
pub use trigger_why::{counteract_reason, trigger_reason};

/// Measurement counters for `docs/BOT.md` §5 (B0); see `host::bot_cost`.
#[cfg(feature = "bot-cost")]
pub use host::bot_cost;

//! Host for the card ruleset -- one `.wasm` module per card -- executed with **wasmi** on both the
//! dedicated server (native) and the browser (inside our own wasm32 build).
//!
//! # Deterministic replay instead of coroutines
//!
//! The C# engine suspends an effect at `yield return Ask(...)` and resumes it later.
//! WebAssembly has no portable way to suspend, so we don't: an effect runs from the
//! start against a **copy** of the world. If it reaches a prompt that has no answer
//! in the effect's answer log, the run is aborted and the copy -- with every side
//! effect the run made -- is thrown away. When the player answers, the effect runs
//! again from the same snapshot (RNG included), so dice re-roll identically and it
//! gets past the prompt this time. Only a run that finishes is committed.
//!
//! This covers every prompt shape in the original, including the 14 classes whose
//! options depend on dice or loop state, and reactions (another player's reaction is
//! one more entry in the answer log). Cost: an effect with *k* prompts runs *k + 1*
//! times; the original's maximum is 6 prompts in one effect.
//!
//! See `docs/P0-FINDINGS.md` for why this replaced the CEL + Lua design.

mod host;
mod wasm_rules;
mod world;

pub use card_sdk::abi::{PromptKind, TriggerKind, ABI_VERSION};
pub use host::{CardInfo, Call, Outcome, Prompt, PromptOption, RuleError, Ruleset, RulesetBuilder, MAX_NESTING};
pub use game_core::msg::Msg;
pub use wasm_rules::WasmRules;
pub use world::{CardWorld, Trigger};

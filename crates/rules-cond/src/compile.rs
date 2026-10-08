//! `compile(src) -> Result<Cond, CondError>`: parse, lint, rewrite, hold.
//!
//! The serialized form (feature `wire`) is postcard of `cel::wire::Compiled`
//! -- the rewritten AST plus an optional source string, with a one-byte
//! format version in front so a fork bump fails loudly instead of decoding
//! garbage. `used_vars` / `used_fns` are recomputed from the tree on load.

use cel::common::ast::IdedExpr;
use cel::Env;
use std::sync::{Arc, OnceLock};

#[cfg(feature = "compile")]
use crate::schema;
use crate::CondError;

/// Shared CEL environment: stdlib (operators + the int-safe conversions) plus
/// nothing else. Our lookup functions live on each window's root [`cel::Context`]
/// because they need to read the bound variables; see [`crate::eval`].
pub(crate) fn shared_env() -> Arc<Env> {
    static ENV: OnceLock<Arc<Env>> = OnceLock::new();
    // Minimal stdlib: operators are interpreted natively; the guard
    // schema needs only the scalar conversions and the string predicates
    // (`size`/`contains`/`startsWith`/`endsWith`) plus the functions
    // `install_functions` adds. See cel `Env::with_minimal_stdlib`.
    ENV.get_or_init(|| Arc::new(Env::default().with_minimal_stdlib())).clone()
}

/// A compiled condition. Built once per guarded entry at load time
/// (`RulesetBuilder::build` in G2), evaluated natively against
/// `(&WindowCtx, &CandidateCtx)`.
#[derive(Clone, Debug)]
pub struct Cond {
    /// Rewritten AST: dotted roots flattened, `effect.has` -> `chain_has`.
    expr: IdedExpr,
    /// Original source, for error messages and the G3 `precheck` report.
    src: String,
    /// Variables this condition actually reads (from the rewritten AST).
    /// `eval` binds only these -- the win over "bind the whole schema" is
    /// most of the per-candidate cost.
    used_vars: Vec<String>,
    /// Functions this condition calls (after rewrite).
    used_fns: Vec<String>,
}

impl Cond {
    pub fn src(&self) -> &str {
        &self.src
    }

    /// The rewritten expression (exposed for tests and the G3 `precheck`).
    pub fn expr(&self) -> &IdedExpr {
        &self.expr
    }

    /// Variables the condition reads (schema names, after flattening).
    pub fn used_vars(&self) -> &[String] {
        &self.used_vars
    }

    /// Functions the condition calls (after `effect.has` -> `chain_has`).
    pub fn used_fns(&self) -> &[String] {
        &self.used_fns
    }
}

/// Parse + type/unknown-variable check + int-only lint, once.
///
/// * [`CondError::Parse`] -- the source does not parse.
/// * [`CondError::UnknownVar`] / [`CondError::UnknownFn`] -- not in the §4.2 schema.
/// * [`CondError::FloatLiteral`] / [`CondError::FloatOp`] -- int-only mandate.
#[cfg(feature = "compile")]
pub fn compile(src: &str) -> Result<Cond, CondError> {
    let env = shared_env();
    let program = env.compile(src).map_err(|e| CondError::Parse(e.to_string()))?;
    let rewritten = schema::lint_and_rewrite(program.expression())?;
    Ok(Cond::from_expr(rewritten, src.to_string()))
}

impl Cond {
    /// Builds a condition from a (rewritten) AST. Recomputes `used_vars` /
    /// `used_fns` from the tree.
    pub(crate) fn from_expr(expr: IdedExpr, src: String) -> Cond {
        let refs = expr.references();
        let mut used_vars: Vec<String> = refs.variables().into_iter().map(str::to_string).collect();
        used_vars.sort();
        used_vars.dedup();
        let mut used_fns: Vec<String> = refs.functions().into_iter().map(str::to_string).collect();
        used_fns.sort();
        used_fns.dedup();
        Cond {
            expr,
            src,
            used_vars,
            used_fns,
        }
    }

    /// The serialized compiled form: postcard of `cel::wire::Compiled`
    /// (format version first, then the rewritten AST, then the source text
    /// when `with_source`). Pass `with_source = false` for the lean form a
    /// runtime-only consumer evaluates.
    ///
    /// This is the form `RulesetBuilder::build` (G2) caches per guarded
    /// entry; the browser loads it with [`Cond::from_bytes`] and never sees
    /// the parser.
    #[cfg(feature = "wire")]
    pub fn to_bytes(&self, with_source: bool) -> Vec<u8> {
        let compiled = cel::wire::Compiled::new(
            self.expr.clone(),
            if with_source {
                Some(self.src.clone())
            } else {
                None
            },
        );
        postcard::to_allocvec(&compiled).expect("postcard encode of a Cond cannot fail")
    }

    /// Loads a condition serialized by [`Cond::to_bytes`]. Fails loudly on a
    /// format-version mismatch ([`CondError::WireVersion`]) -- postcard is
    /// not self-describing, so a fork bump that changes the AST shape must
    /// not silently decode garbage.
    #[cfg(feature = "wire")]
    pub fn from_bytes(bytes: &[u8]) -> Result<Cond, CondError> {
        let compiled: cel::wire::Compiled =
            postcard::from_bytes(bytes).map_err(|e| CondError::WireDecode(e.to_string()))?;
        compiled
            .check()
            .map_err(|e| match e {
                cel::wire::WireError::Version { found, expected } => {
                    CondError::WireVersion { found, expected }
                }
            })?;
        let src = compiled.source().unwrap_or("").to_string();
        Ok(Cond::from_expr(compiled.expression().clone(), src))
    }
}
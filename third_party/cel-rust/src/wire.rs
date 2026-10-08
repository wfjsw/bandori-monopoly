//! The serialized compiled form: a versioned wrapper around the native AST.
//!
//! [`Compiled`] is one expression tree plus a format version — what you
//! compile once at load time (host, with the `parser` feature) and feed to
//! [`Program::from_ast`] on the way in (runtime-only, no parser). The derive
//! is plain serde, so any format works: this project uses postcard (the same
//! format as its ruleset manifest), but bincode / serde_json / a
//! self-describing format are drop-in alternatives.
//!
//! # Format version
//!
//! postcard is not self-describing: a blob is just field bytes in declaration
//! order. [`FORMAT_VERSION`] is therefore the first field, and
//! [`Compiled::check`] (called by [`Compiled::into_program`]) rejects any
//! other value with [`WireError::Version`]. A fork bump that changes the
//! shape of [`IdedExpr`] (or of this wrapper) **must** bump
//! [`FORMAT_VERSION`]; every decode of an older blob then fails loudly
//! instead of producing garbage. There is no cross-version migration.
//!
//! # What is on the wire
//!
//! Only what evaluation needs: the expression tree. Source offsets are not
//! carried (they exist to place parse errors, which happen on the host), and
//! neither are macro bookkeeping or types — the interpreter resolves names
//! against the [`Context`](crate::Context) at runtime. The source *text* is
//! optional and omitted when `None`, for hosts that want it in error
//! messages.
//!
//! # Example
//!
//! ```
//! use cel::wire::Compiled;
//! use cel::{Context, Program, Value};
//!
//! // Host: compile once, serialize.
//! let program = Program::compile("1 + 2").unwrap();
//! let compiled = program.to_compiled(true);
//! let bytes = postcard::to_allocvec(&compiled).unwrap();
//!
//! // Runtime: deserialize, evaluate. No parser involved.
//! let compiled: Compiled = postcard::from_bytes(&bytes).unwrap();
//! let program = compiled.into_program().unwrap();
//! assert_eq!(program.execute(&Context::default()), Ok(Value::Int(3)));
//! ```

use serde::{Deserialize, Serialize};

use crate::common::ast::{IdedExpr, SourceInfo};
use crate::Program;

/// The version of the serialized form. Bump on any change to [`Compiled`]'s
/// fields or to the shape of [`IdedExpr`] that would make an older blob
/// decode to garbage.
pub const FORMAT_VERSION: u8 = 1;

/// A serialized compiled expression.
///
/// Field order is wire-relevant for non-self-describing formats (postcard):
/// `version` comes first on purpose.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Compiled {
    /// [`FORMAT_VERSION`] at encode time. Checked on load.
    pub version: u8,
    /// The expression tree, as the interpreter walks it.
    pub expression: IdedExpr,
    /// The source text, for host-side error messages. Not needed to
    /// evaluate: pass `None` for the lean runtime-only form.
    ///
    /// Always encoded, even when `None` (one tag byte): postcard is not
    /// self-describing, so a `skip_serializing_if` here would make the
    /// decoder read past the end of a lean blob.
    pub source: Option<String>,
}

/// Why a serialized compiled expression was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    /// The blob carries a format version this build does not understand.
    Version { found: u8, expected: u8 },
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireError::Version { found, expected } => write!(
                f,
                "compiled-expression format version {found}, this build reads {expected}"
            ),
        }
    }
}

impl std::error::Error for WireError {}

impl Compiled {
    /// A compiled expression carrying `source` for error messages.
    pub fn new(expression: IdedExpr, source: impl Into<Option<String>>) -> Self {
        Compiled {
            version: FORMAT_VERSION,
            expression,
            source: source.into(),
        }
    }

    /// A compiled expression with no source text: the lean runtime-only form.
    pub fn bare(expression: IdedExpr) -> Self {
        Compiled::new(expression, None)
    }

    /// Rejects a blob whose format version isn't this build's.
    pub fn check(&self) -> Result<(), WireError> {
        if self.version == FORMAT_VERSION {
            Ok(())
        } else {
            Err(WireError::Version {
                found: self.version,
                expected: FORMAT_VERSION,
            })
        }
    }

    /// The source text, if the blob carried one.
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// The expression tree.
    pub fn expression(&self) -> &IdedExpr {
        &self.expression
    }

    /// Checks the version and hands back a program. The source text, when
    /// present, is kept on the program's [`SourceInfo`] for error positions.
    pub fn into_program(self) -> Result<Program, WireError> {
        self.check()?;
        let mut source_info = SourceInfo::default();
        source_info.source = self.source.unwrap_or_default();
        Ok(Program::from_ast(self.expression, source_info))
    }
}

impl Program {
    /// This program as a [`Compiled`], ready to serialize.
    ///
    /// `with_source` keeps the source text for host-side error messages;
    /// pass `false` for the lean form a runtime-only consumer evaluates.
    pub fn to_compiled(&self, with_source: bool) -> Compiled {
        let source = if with_source {
            let source = &self.source_info().source;
            if source.is_empty() {
                None
            } else {
                Some(source.clone())
            }
        } else {
            None
        };
        Compiled::new(self.expression().clone(), source)
    }

    /// A [`Compiled`] as a program: the runtime-only counterpart of
    /// [`Program::compile`]. Checks the format version.
    pub fn from_compiled(compiled: Compiled) -> Result<Program, WireError> {
        compiled.into_program()
    }
}
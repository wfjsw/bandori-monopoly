//! The expression tree and, behind the `parser` feature, the parser that
//! builds it.
//!
//! [`Expression`] and [`ExpressionReferences`] are part of the interpreter's
//! input and available without the `parser` feature: a runtime-only build
//! deserializes or hand-builds an [`Expression`] and evaluates it with
//! [`Program::from_ast`](crate::Program::from_ast).

#![allow(clippy::module_inception)]
#[cfg(feature = "parser")]
#[allow(clippy::all)]
mod gen;

pub mod references;

pub use crate::common::ast::IdedExpr as Expression;

#[cfg(feature = "parser")]
mod macros;
#[cfg(feature = "parser")]
mod parse;
#[cfg(feature = "parser")]
#[allow(non_snake_case)]
mod parser;
#[cfg(feature = "parser_pratt")]
#[doc(hidden)]
pub mod pratt_parser;

#[cfg(feature = "parser")]
pub use macros::Macro;
#[cfg(feature = "parser")]
pub(crate) use macros::{map_macro_expander, Macros};
#[cfg(feature = "parser")]
pub use parser::*;
#[cfg(feature = "parser_pratt")]
#[doc(hidden)]
pub use pratt_parser::PrattParser;
pub use references::ExpressionReferences;
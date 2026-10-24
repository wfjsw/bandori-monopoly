//! Content extension: game CEL sugar names, kind constants, guard-const
//! expansions. The kernel keeps a closed generic algebra; a ruleset registers
//! one [`CondExt`] (once) or passes a [`Vocab`] explicitly to the hermetic
//! entry points.

use std::sync::OnceLock;

use rules_cond_ir::ctx::FnId;

use crate::vocab::{self, Name};

/// How a [`Name`] lowers to the IR. Kernel rows use [`LowerAs::Direct`].
#[derive(Clone, Copy, Debug)]
pub enum LowerAs {
    /// Kernel name: scalar → `VarId`, func → `FnId` from its flat id.
    Direct,
    /// Func sugar. Emit `Call(fid, [user_args…, Const(id_of(bake))…])` — the
    /// bake constants occupy the trailing key slots of the target function
    /// (`f` → `Call{ fid: FnId::State, bake: &["k"] }` lowers
    /// `f(p)` to `Call(State, [p, id_of("k")])`).
    Call {
        fid: FnId,
        bake: &'static [&'static str],
    },
    /// Func sugar. Emit
    /// `Eq(Call(fid, [user_args[..n-1]…, Const(id_of(bake))…]), user_args[n-1])`
    /// (`f_is` → `CallEq{ fid: FnId::State, bake: &["k"] }`).
    CallEq {
        fid: FnId,
        bake: &'static [&'static str],
    },
    /// Candidate scalar sugar. Emit `Call(fid, [Var(Owner), Const(id_of(key))])`
    /// (`owner.f` → `OwnerState{ fid: FnId::State, key: "k" }`).
    OwnerState { fid: FnId, key: &'static str },
}

/// The static data package a content crate installs.
pub struct CondExt {
    /// Extra CEL names. `Name::lower_as` must reference kernel `FnId`s only.
    pub names: &'static [Name],
    /// Bare-ident CEL string constants: (`"SomeKind"`, `"wire:ns:id"`).
    /// Merged into `kinds::str_constants()` at lookup time.
    pub kind_consts: &'static [(&'static str, &'static str)],
    /// Source-scanner expansions for Rust const refs in `pre` position:
    /// (`"content::guards::OWN_START"`,
    ///  `"actor == owner && tile_is(tile.id, \"kind\")")`).
    pub guard_consts: &'static [(&'static str, &'static str)],
}

pub static EMPTY_EXT: CondExt = CondExt {
    names: &[],
    kind_consts: &[],
    guard_consts: &[],
};

/// Why a second [`install_ext`] was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlreadyInstalled;

impl std::fmt::Display for AlreadyInstalled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CondExt already installed")
    }
}

impl std::error::Error for AlreadyInstalled {}

static EXT: OnceLock<&'static CondExt> = OnceLock::new();

/// Install the content package. Panics via [`OnceLock`] on a 2nd set — call
/// before the first compile/lower.
pub fn install_ext(ext: &'static CondExt) -> Result<(), AlreadyInstalled> {
    EXT.set(ext).map_err(|_| AlreadyInstalled)
}

/// The installed package, or [`EMPTY_EXT`] before install.
pub fn ext() -> &'static CondExt {
    EXT.get().copied().unwrap_or(&EMPTY_EXT)
}

/// Merged view: kernel `VOCAB` ++ `ext().names`, kernel kinds ++ `ext().kind_consts`.
pub struct Vocab<'a> {
    ext: Option<&'a CondExt>,
}

impl Vocab<'static> {
    pub fn kernel_only() -> Self {
        Vocab { ext: None }
    }

    pub fn with_ext(ext: &'static CondExt) -> Self {
        Vocab { ext: Some(ext) }
    }
}

impl<'a> Vocab<'a> {
    /// The merged name set (kernel first, then extension).
    pub fn names(&self) -> impl Iterator<Item = &'static Name> + 'a {
        vocab::VOCAB.iter().chain(self.ext.map(|e| e.names.iter()).into_iter().flatten())
    }

    pub fn by_cel(&self, s: &str) -> Option<&'static Name> {
        vocab::VOCAB
            .iter()
            .chain(self.ext.map(|e| e.names.iter()).into_iter().flatten())
            .find(|n| n.cel == s || n.aliases.contains(&s))
    }

    pub fn by_flat(&self, s: &str) -> Option<&'static Name> {
        vocab::VOCAB
            .iter()
            .chain(self.ext.map(|e| e.names.iter()).into_iter().flatten())
            .find(|n| n.flat == s)
    }

    /// Kernel kind spellings ++ extension kind constants.
    pub fn str_constants(&self) -> impl Iterator<Item = (&'static str, &'static str)> + 'a {
        crate::kinds::engine_str_constants()
            .iter()
            .copied()
            .chain(self.ext.map(|e| e.kind_consts.iter().copied()).into_iter().flatten())
    }

    /// Expansion for a Rust const path in `pre` position.
    pub fn guard_const(&self, path: &str) -> Option<&'static str> {
        self.ext?
            .guard_consts
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, lit)| *lit)
    }

    /// The extension this view was built with, if any.
    pub fn ext(&self) -> Option<&'a CondExt> {
        self.ext
    }
}

/// The process-global merged vocabulary (`Vocab::with_ext(ext())`).
pub fn vocab() -> Vocab<'static> {
    Vocab::with_ext(ext())
}

/// Hermetic compile (no global state) for build scripts.
#[cfg(feature = "compile")]
pub fn compile_with(src: &str, v: &Vocab) -> Result<crate::Cond, crate::CondError> {
    crate::compile::compile_with(src, v)
}

/// Hermetic lower (no global state) for build scripts.
#[cfg(feature = "lower")]
pub fn lower_with(
    cond: &crate::Cond,
    v: &Vocab,
) -> Result<crate::lower::OwnedProgram, crate::CondError> {
    crate::lower::lower_with(cond, v)
}

/// Hermetic generator entry (no global state) for build scripts.
#[cfg(feature = "lower")]
pub fn generate_with(
    dir: &std::path::Path,
    v: &Vocab,
) -> Result<crate::gen_ir::Generated, crate::CondError> {
    crate::gen_ir::generate_with(dir, v)
}
//! wasmtime backend: the optimizing compiler.
//!
//! The server runs card modules here -- a card effect is replayed once per
//! prompt, so the engine speed matters. Same modules as the browser (which
//! runs the wasmi interpreter), same behaviour: fuel bounds runaway effects,
//! and a host call that published a prompt stops the run.

use std::sync::OnceLock;

pub use wasmtime::{Caller, Engine, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime::{Config, InstancePre};

/// Ceiling on one instance's linear memory. A card run allocates a handful of
/// small buffers and the guest bump allocator (`card-sdk` `rt`, `MAX_MEMORY`)
/// refuses past this same figure and traps, so this is the host's matching
/// backstop: a runaway `memory.grow` stops here instead of eating the process.
/// 16 MiB is far above any real effect (the modules boot at ~1 MiB).
pub const MAX_MEMORY_BYTES: usize = 16 * 1024 * 1024;

/// Host errors and traps. The "a prompt is waiting" signal is a marker error the
/// host raises instead of a plain message, so `is_need_input` can recognize it.
pub type Error = wasmtime::Error;

#[derive(Debug)]
struct NeedInputTrap;

impl std::fmt::Display for NeedInputTrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a prompt is waiting for an answer")
    }
}

impl std::error::Error for NeedInputTrap {}

/// The host-side "a prompt is waiting" marker.
///
/// This is **not** raised into the guest for the imports a card reads as
/// `Result` -- those return `card_sdk::abi::EXIT_NEED_INPUT` as their value, and
/// `host::fold_exit` turns that sentinel back into this marker at the
/// `bandori_on` boundary. It is still raised by the imports whose guest wrappers
/// read a plain `bool`/`Option` (`gate`, `target`, `card_move`, the buy/build
/// routines): those have nowhere to carry a sentinel without it reading as
/// `true`/`Some(..)`, so the trap -- which tears the stack and therefore cannot
/// be swallowed -- is the correct shape for them until their wrappers are
/// `Result`-shaped too.
pub fn need_input() -> Error {
    Error::new(NeedInputTrap)
}

pub fn is_need_input(e: &Error) -> bool {
    e.downcast_ref::<NeedInputTrap>().is_some()
}

pub fn err(msg: impl Into<String>) -> Error {
    Error::msg(msg.into())
}

/// The whole message including the cause chain: a trap caused by running out of
/// fuel reports "error while executing at wasm backtrace ..." on top and the
/// real reason underneath.
pub fn error_text(e: &Error) -> String {
    format!("{e:#}")
}

/// One engine per process: compiling the same module set repeatedly is the
/// card replay's steady state, and wasmtime caches compilations per engine.
pub fn new_engine() -> Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            let mut cfg = Config::new();
            cfg.consume_fuel(true);
            cfg.wasm_multi_memory(false);
            Engine::new(&cfg).expect("wasmtime engine")
        })
        .clone()
}

pub fn compile(engine: &Engine, wasm: &[u8]) -> Result<Module, String> {
    Module::new(engine, wasm).map_err(|e| e.to_string())
}

pub fn set_fuel<T>(store: &mut Store<T>, fuel: u64) -> Result<(), String> {
    store.set_fuel(fuel).map_err(|e| e.to_string())
}

pub fn instantiate<T>(
    linker: &Linker<T>,
    store: &mut Store<T>,
    module: &Module,
) -> Result<Instance, Error> {
    linker.instantiate(&mut *store, module)
}

/// A module whose imports are resolved against a [`Linker`] **once**.
/// Instantiating from it skips the per-call import walk, which is the bulk of
/// what is left of a fire-up now that the linker itself is cached. The host
/// caches these per (engine, world type, module).
pub struct Prepared<T>(InstancePre<T>);

pub fn prepare<T: 'static>(linker: &Linker<T>, module: &Module) -> Result<Prepared<T>, Error> {
    Ok(Prepared(linker.instantiate_pre(module)?))
}

pub fn instantiate_prepared<T>(p: &Prepared<T>, store: &mut Store<T>) -> Result<Instance, Error> {
    p.0.instantiate(&mut *store)
}

pub fn has_func(inst: &Instance, store: &mut Store<impl Sized>, name: &str) -> bool {
    inst.get_func(store, name).is_some()
}

pub fn read_mem(
    inst: &Instance,
    store: &mut Store<impl Sized>,
    ptr: i32,
    len: i32,
) -> Result<Vec<u8>, Error> {
    let mem = inst
        .get_memory(&mut *store, card_sdk::abi::export::MEMORY)
        .ok_or_else(|| err("no exported memory"))?;
    let (ptr, len) = (ptr as u32 as usize, len as u32 as usize);
    let bytes = mem
        .data(&mut *store)
        .get(ptr..ptr + len)
        .ok_or_else(|| err("memory read out of bounds"))?;
    Ok(bytes.to_vec())
}

pub fn read_guest<W>(caller: &mut Caller<'_, W>, ptr: i32, len: i32) -> Result<Vec<u8>, Error> {
    let mem = caller
        .get_export(card_sdk::abi::export::MEMORY)
        .and_then(|e| e.into_memory())
        .ok_or_else(|| err("guest has no memory export"))?;
    let (ptr, len) = (ptr as u32 as usize, len as u32 as usize);
    let bytes = mem
        .data(&*caller)
        .get(ptr..ptr + len)
        .ok_or_else(|| err("guest read out of bounds"))?;
    Ok(bytes.to_vec())
}

/// Write `bytes` at `ptr` into a guest-owned buffer (the guest allocates; the
/// host never allocates inside guest memory).
pub fn write_guest<W>(caller: &mut Caller<'_, W>, ptr: i32, bytes: &[u8]) -> Result<(), Error> {
    let mem = caller
        .get_export(card_sdk::abi::export::MEMORY)
        .and_then(|e| e.into_memory())
        .ok_or_else(|| err("guest has no memory export"))?;
    let ptr = ptr as u32 as usize;
    let dst = mem
        .data_mut(&mut *caller)
        .get_mut(ptr..ptr + bytes.len())
        .ok_or_else(|| err("guest write out of bounds"))?;
    dst.copy_from_slice(bytes);
    Ok(())
}

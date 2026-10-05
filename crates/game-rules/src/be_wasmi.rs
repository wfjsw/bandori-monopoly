//! wasmi backend: the interpreter.
//!
//! This is what runs in the browser (the whole game compiles to wasm32, where an
//! embedded JIT is not available) and, with the `wasmi-native` feature, on the
//! server too -- mainly to keep both paths covered by the same tests.

pub use wasmi::{Caller, Engine, Linker, Module, Store};
use wasmi::{Config, Extern, Instance, Memory};

/// Host errors and traps. wasmi carries an i32 exit status on traps.
pub type Error = wasmi::Error;

/// The exit status a host function raises when it published a prompt and the
/// run must be replayed with the answer (see `EXIT_NEED_INPUT`).
pub fn need_input() -> Error {
    wasmi::Error::i32_exit(card_sdk::abi::EXIT_NEED_INPUT)
}

pub fn is_need_input(e: &Error) -> bool {
    e.i32_exit_status() == Some(card_sdk::abi::EXIT_NEED_INPUT)
}

pub fn err(msg: impl Into<String>) -> Error {
    Error::new(msg.into())
}

/// The whole message, including causes (wasmi errors are flat strings).
pub fn error_text(e: &Error) -> String {
    e.to_string()
}

pub fn new_engine() -> Engine {
    let mut cfg = Config::default();
    cfg.consume_fuel(true); // runaway effects stop at the fuel limit, like wasmtime
    Engine::new(&cfg)
}

pub fn compile(engine: &Engine, wasm: &[u8]) -> Result<Module, String> {
    Module::new(engine, wasm).map_err(|e| e.to_string())
}

pub fn set_fuel<T>(store: &mut Store<T>, fuel: u64) -> Result<(), String> {
    store.set_fuel(fuel).map_err(|e| e.to_string())
}

pub fn instantiate<T>(linker: &Linker<T>, store: &mut Store<T>, module: &Module) -> Result<Instance, Error> {
    linker.instantiate_and_start(&mut *store, module)
}

pub fn has_func(inst: &Instance, store: &Store<impl Sized>, name: &str) -> bool {
    inst.get_func(store, name).is_some()
}

/// Read `len` bytes at `ptr` from the module's exported memory.
pub fn read_mem(inst: &Instance, store: &Store<impl Sized>, ptr: i32, len: i32) -> Result<Vec<u8>, Error> {
    let mem: Memory = inst.get_memory(store, card_sdk::abi::export::MEMORY).ok_or_else(|| err("no exported memory"))?;
    let (ptr, len) = (ptr as u32 as usize, len as u32 as usize);
    let bytes = mem.data(store).get(ptr..ptr + len).ok_or_else(|| err("memory read out of bounds"))?;
    Ok(bytes.to_vec())
}

/// Read `len` bytes at `ptr` from a host call's view of the guest memory.
pub fn read_guest<W>(caller: &mut Caller<'_, W>, ptr: i32, len: i32) -> Result<Vec<u8>, Error> {
    let mem = caller.get_export(card_sdk::abi::export::MEMORY).and_then(Extern::into_memory).ok_or_else(|| err("guest has no memory export"))?;
    let (ptr, len) = (ptr as u32 as usize, len as u32 as usize);
    let bytes = mem.data(&*caller).get(ptr..ptr + len).ok_or_else(|| err("guest read out of bounds"))?;
    Ok(bytes.to_vec())
}

/// Write `bytes` at `ptr` into a guest-owned buffer (the guest allocates; the
/// host never allocates inside guest memory).
pub fn write_guest<W>(caller: &mut Caller<'_, W>, ptr: i32, bytes: &[u8]) -> Result<(), Error> {
    let mem = caller.get_export(card_sdk::abi::export::MEMORY).and_then(Extern::into_memory).ok_or_else(|| err("guest has no memory export"))?;
    let ptr = ptr as u32 as usize;
    let dst = mem.data_mut(&mut *caller).get_mut(ptr..ptr + bytes.len()).ok_or_else(|| err("guest write out of bounds"))?;
    dst.copy_from_slice(bytes);
    Ok(())
}
//! The sandboxed WASM plugin runtime.
//!
//! A `wasm` plugin is a WebAssembly module that speaks the same method/params →
//! JSON contract as the process runtime, but runs in-process under `wasmi` with
//! **no ambient authority**: the host links only the capability functions the
//! plugin declared and was granted, so a guest that never asked for `network`
//! cannot import `host_fetch`, and one that did not get it fails to instantiate.
//!
//! ## ABI
//!
//! The guest module must export:
//! - `memory` - its linear memory.
//! - `alloc(len: i32) -> i32` - bump-allocate `len` bytes, return the pointer.
//! - `invoke(method_ptr, method_len, params_ptr, params_len: i32) -> i64` -
//!   handle a call and return a packed `(ptr << 32) | len` pointing at a UTF-8
//!   JSON result in guest memory.
//!
//! The host writes `method` and `params` (a JSON string) into guest memory via
//! `alloc`, calls `invoke`, and reads the result back. Capability host functions
//! live in the `env` module (e.g. `env.host_log`).

use std::collections::HashSet;

use serde_json::Value;
use wasmi::{
    Caller, Config, Engine, Extern, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
};

use crate::Error;

/// Ceiling on a guest's linear memory. A plugin cannot grow past this, so a
/// runaway allocation traps instead of exhausting host memory.
const MEMORY_MAX_BYTES: usize = 64 * 1024 * 1024;
/// Ceiling on a guest's table size (function-pointer table growth).
const TABLE_MAX_ELEMENTS: u32 = 100_000;
/// Fuel granted for instantiation (the module's `start`, if any).
const FUEL_INSTANTIATE: u64 = 50_000_000;
/// Fuel granted per `invoke` call. Fuel is roughly one unit per executed
/// instruction, so an infinite loop exhausts it and traps deterministically.
const FUEL_PER_CALL: u64 = 200_000_000;
/// Ceiling on the JSON result a guest may return from `invoke`, so a bogus
/// `(ptr, len)` cannot make the host allocate an enormous buffer.
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
/// Ceiling on total bytes retained from `host_log` across a runtime's life, so a
/// log flood cannot exhaust host memory.
const MAX_LOG_BYTES: usize = 64 * 1024;
/// Ceiling on a single `host_log` line the host will read from guest memory.
const MAX_LOG_LINE_BYTES: usize = 8 * 1024;

/// The host-side state a guest's imports operate on.
pub struct HostState {
    /// Capabilities granted to this plugin - gates which imports are linked.
    caps: HashSet<String>,
    /// Log lines the guest emitted via `host_log` (surfaced to the app).
    pub logs: Vec<String>,
    /// Running total of retained log bytes, bounded by [`MAX_LOG_BYTES`].
    log_bytes: usize,
    /// Memory/table growth limits enforced by wasmi on this store.
    limits: StoreLimits,
}

impl HostState {
    /// Append a guest log line, dropping it (or the overflow) once the total log
    /// budget is spent, so a flood of `host_log` calls cannot grow unbounded.
    fn push_log(&mut self, mut text: String) {
        if self.log_bytes >= MAX_LOG_BYTES {
            return;
        }
        let remaining = MAX_LOG_BYTES - self.log_bytes;
        if text.len() > remaining {
            text.truncate(floor_char_boundary(&text, remaining));
        }
        self.log_bytes += text.len();
        self.logs.push(text);
    }
}

/// Largest char-boundary index at or below `max` (stable-Rust replacement for
/// the nightly `str::floor_char_boundary`).
fn floor_char_boundary(s: &str, max: usize) -> usize {
    if max >= s.len() {
        return s.len();
    }
    let mut i = max;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// A loaded, instantiated WASM plugin ready to answer `invoke` calls.
pub struct WasmRuntime {
    store: Store<HostState>,
    memory: Memory,
    alloc: wasmi::TypedFunc<i32, i32>,
    invoke: wasmi::TypedFunc<(i32, i32, i32, i32), i64>,
}

impl WasmRuntime {
    /// Load and instantiate `wasm_bytes`, granting `capabilities`. The store is
    /// fuel-metered and memory/table-limited, so a defective or hostile guest
    /// fails safely within deterministic bounds instead of monopolizing the host.
    pub fn new(wasm_bytes: &[u8], capabilities: &[String]) -> Result<Self, Error> {
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module =
            Module::new(&engine, wasm_bytes).map_err(|e| Error::Protocol(e.to_string()))?;

        let limits = StoreLimitsBuilder::new()
            .memory_size(MEMORY_MAX_BYTES)
            .table_elements(TABLE_MAX_ELEMENTS)
            .memories(1)
            .tables(4)
            .instances(1)
            // Trap on an over-limit grow instead of returning -1, so a runaway
            // allocation surfaces as an error rather than odd guest behavior.
            .trap_on_grow_failure(true)
            .build();
        let state = HostState {
            caps: capabilities.iter().cloned().collect(),
            logs: Vec::new(),
            log_bytes: 0,
            limits,
        };
        let mut store = Store::new(&engine, state);
        store.limiter(|s| &mut s.limits);
        // Budget for instantiation (a module `start`, if present).
        store
            .add_fuel(FUEL_INSTANTIATE)
            .map_err(|e| Error::Protocol(e.to_string()))?;
        let mut linker: Linker<HostState> = Linker::new(&engine);

        // Always-linked: logging. Ungated, harmless. Bounded by MAX_LOG_BYTES.
        linker
            .func_wrap(
                "env",
                "host_log",
                |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
                    if let Some(text) = read_string(&caller, ptr, len) {
                        caller.data_mut().push_log(text);
                    }
                },
            )
            .map_err(|e| Error::Protocol(e.to_string()))?;

        // cap: notify - a desktop notification bridge (recorded here).
        if store.data().caps.contains("notify") {
            linker
                .func_wrap(
                    "env",
                    "host_notify",
                    |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
                        if let Some(text) = read_string(&caller, ptr, len) {
                            caller.data_mut().push_log(format!("notify: {text}"));
                        }
                    },
                )
                .map_err(|e| Error::Protocol(e.to_string()))?;
        }

        let instance = linker
            .instantiate(&mut store, &module)
            .and_then(|pre| pre.start(&mut store))
            .map_err(|e| Error::Protocol(e.to_string()))?;

        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| Error::Protocol("guest has no exported `memory`".into()))?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&store, "alloc")
            .map_err(|_| Error::Protocol("guest has no `alloc(i32)->i32`".into()))?;
        let invoke = instance
            .get_typed_func::<(i32, i32, i32, i32), i64>(&store, "invoke")
            .map_err(|_| Error::Protocol("guest has no `invoke(i32,i32,i32,i32)->i64`".into()))?;

        Ok(WasmRuntime {
            store,
            memory,
            alloc,
            invoke,
        })
    }

    /// Call the plugin's `invoke` with a method name and JSON params, returning
    /// the parsed JSON result.
    pub fn call(&mut self, method: &str, params: &Value) -> Result<Value, Error> {
        // Grant this call a bounded fuel budget; an infinite loop traps when it
        // runs out rather than hanging the host.
        self.store
            .add_fuel(FUEL_PER_CALL)
            .map_err(|e| Error::Protocol(e.to_string()))?;

        let params_str = params.to_string();
        let (mp, ml) = self.write_bytes(method.as_bytes())?;
        let (pp, pl) = self.write_bytes(params_str.as_bytes())?;

        let packed = self
            .invoke
            .call(&mut self.store, (mp, ml, pp, pl))
            .map_err(|e| Error::Runtime(e.to_string()))?;
        let ptr = (packed >> 32) as u32 as usize;
        let len = (packed & 0xffff_ffff) as u32 as usize;

        // Refuse an oversized result before allocating for it.
        if len > MAX_RESPONSE_BYTES {
            return Err(Error::Protocol(format!(
                "plugin result too large: {len} bytes (max {MAX_RESPONSE_BYTES})"
            )));
        }
        let mut buf = vec![0u8; len];
        self.memory
            .read(&self.store, ptr, &mut buf)
            .map_err(|e| Error::Protocol(e.to_string()))?;
        let text = String::from_utf8_lossy(&buf);
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|e| Error::Protocol(e.to_string()))
    }

    /// The log lines the guest emitted so far.
    pub fn logs(&self) -> &[String] {
        &self.store.data().logs
    }

    /// Allocate `bytes.len()` in guest memory and copy `bytes` in.
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(i32, i32), Error> {
        let len = bytes.len() as i32;
        let ptr = self
            .alloc
            .call(&mut self.store, len)
            .map_err(|e| Error::Runtime(e.to_string()))?;
        self.memory
            .write(&mut self.store, ptr as usize, bytes)
            .map_err(|e| Error::Protocol(e.to_string()))?;
        Ok((ptr, len))
    }
}

/// Read a UTF-8 string from a caller's guest memory, capped at
/// [`MAX_LOG_LINE_BYTES`] so a huge `len` cannot make the host allocate an
/// enormous buffer.
fn read_string(caller: &Caller<'_, HostState>, ptr: i32, len: i32) -> Option<String> {
    let memory = caller.get_export("memory").and_then(Extern::into_memory)?;
    let want = (len.max(0) as usize).min(MAX_LOG_LINE_BYTES);
    let mut buf = vec![0u8; want];
    memory.read(caller, ptr as usize, &mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
#[path = "../tests/wasm.rs"]
mod tests;

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
//! - `memory` — its linear memory.
//! - `alloc(len: i32) -> i32` — bump-allocate `len` bytes, return the pointer.
//! - `invoke(method_ptr, method_len, params_ptr, params_len: i32) -> i64` —
//!   handle a call and return a packed `(ptr << 32) | len` pointing at a UTF-8
//!   JSON result in guest memory.
//!
//! The host writes `method` and `params` (a JSON string) into guest memory via
//! `alloc`, calls `invoke`, and reads the result back. Capability host functions
//! live in the `env` module (e.g. `env.host_log`).

use std::collections::HashSet;

use serde_json::Value;
use wasmi::{Caller, Engine, Extern, Linker, Memory, Module, Store};

use crate::Error;

/// The host-side state a guest's imports operate on.
pub struct HostState {
    /// Capabilities granted to this plugin — gates which imports are linked.
    caps: HashSet<String>,
    /// Log lines the guest emitted via `host_log` (surfaced to the app).
    pub logs: Vec<String>,
}

/// A loaded, instantiated WASM plugin ready to answer `invoke` calls.
pub struct WasmRuntime {
    store: Store<HostState>,
    memory: Memory,
    alloc: wasmi::TypedFunc<i32, i32>,
    invoke: wasmi::TypedFunc<(i32, i32, i32, i32), i64>,
}

impl WasmRuntime {
    /// Load and instantiate `wasm_bytes`, granting `capabilities`.
    pub fn new(wasm_bytes: &[u8], capabilities: &[String]) -> Result<Self, Error> {
        let engine = Engine::default();
        let module =
            Module::new(&engine, wasm_bytes).map_err(|e| Error::Protocol(e.to_string()))?;

        let state = HostState {
            caps: capabilities.iter().cloned().collect(),
            logs: Vec::new(),
        };
        let mut store = Store::new(&engine, state);
        let mut linker: Linker<HostState> = Linker::new(&engine);

        // Always-linked: logging. Ungated, harmless.
        linker
            .func_wrap(
                "env",
                "host_log",
                |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
                    if let Some(text) = read_string(&caller, ptr, len) {
                        caller.data_mut().logs.push(text);
                    }
                },
            )
            .map_err(|e| Error::Protocol(e.to_string()))?;

        // cap: notify — a desktop notification bridge (recorded here).
        if store.data().caps.contains("notify") {
            linker
                .func_wrap(
                    "env",
                    "host_notify",
                    |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
                        if let Some(text) = read_string(&caller, ptr, len) {
                            caller.data_mut().logs.push(format!("notify: {text}"));
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
        let params_str = params.to_string();
        let (mp, ml) = self.write_bytes(method.as_bytes())?;
        let (pp, pl) = self.write_bytes(params_str.as_bytes())?;

        let packed = self
            .invoke
            .call(&mut self.store, (mp, ml, pp, pl))
            .map_err(|e| Error::Runtime(e.to_string()))?;
        let ptr = (packed >> 32) as u32 as usize;
        let len = (packed & 0xffff_ffff) as u32 as usize;

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

/// Read a UTF-8 string from a caller's guest memory.
fn read_string(caller: &Caller<'_, HostState>, ptr: i32, len: i32) -> Option<String> {
    let memory = caller
        .get_export("memory")
        .and_then(Extern::into_memory)?;
    let mut buf = vec![0u8; len.max(0) as usize];
    memory.read(caller, ptr as usize, &mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
#[path = "../tests/wasm.rs"]
mod tests;

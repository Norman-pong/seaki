use std::sync::Arc;
use wasmtime::{Caller, Engine, Instance, Linker, Module, Store};

use crate::broker::secret::{BrokerError, SecretBroker};

pub struct WasmPluginRuntime {
    engine: Engine,
}

pub struct WasmPluginInstance {
    store: Store<PluginHostState>,
    instance: Instance,
}

#[derive(Debug, Clone)]
pub struct PluginHostState {
    pub plugin_id: String,
    pub allowed_network_hosts: Vec<String>,
    pub secret_broker: Arc<SecretBroker>,
    pub allowed_secret_scopes: Vec<String>,
    pub logs: Vec<(i32, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    WasmLoadFailed(String),
    ExportNotFound(String),
    ExecutionFailed(String),
    InvalidArgument(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::WasmLoadFailed(s) => write!(f, "wasm load failed: {s}"),
            RuntimeError::ExportNotFound(s) => write!(f, "export not found: {s}"),
            RuntimeError::ExecutionFailed(s) => write!(f, "execution failed: {s}"),
            RuntimeError::InvalidArgument(s) => write!(f, "invalid argument: {s}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl WasmPluginRuntime {
    pub fn new() -> Result<Self, RuntimeError> {
        let engine = Engine::default();
        Ok(Self { engine })
    }

    pub fn load_module(&self, wasm_bytes: &[u8]) -> Result<Module, RuntimeError> {
        Module::new(&self.engine, wasm_bytes)
            .map_err(|e| RuntimeError::WasmLoadFailed(e.to_string()))
    }

    pub fn instantiate(
        &self,
        module: &Module,
        host_state: PluginHostState,
    ) -> Result<WasmPluginInstance, RuntimeError> {
        let mut store = Store::new(&self.engine, host_state);
        let mut linker = Linker::new(&self.engine);

        // Register seaki_host_log
        linker
            .func_wrap(
                "env",
                "seaki_host_log",
                |mut caller: Caller<PluginHostState>, level: i32, ptr: i32, len: i32| {
                    let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                        Some(m) => m,
                        None => return,
                    };
                    let mut buf = vec![0u8; len as usize];
                    if memory.read(&caller, ptr as usize, &mut buf).is_err() {
                        return;
                    }
                    let msg = String::from_utf8_lossy(&buf).to_string();
                    caller.data_mut().logs.push((level, msg));
                },
            )
            .map_err(|e| RuntimeError::ExecutionFailed(e.to_string()))?;

        // Register seaki_host_get_secret
        linker
            .func_wrap(
                "env",
                "seaki_host_get_secret",
                |mut caller: Caller<PluginHostState>,
                 scope_ptr: i32,
                 scope_len: i32,
                 out_ptr: i32,
                 out_len: i32|
                 -> i32 {
                    let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                        Some(m) => m,
                        None => return -1,
                    };

                    let mut scope_buf = vec![0u8; scope_len as usize];
                    if memory
                        .read(&caller, scope_ptr as usize, &mut scope_buf)
                        .is_err()
                    {
                        return -2;
                    }
                    let scope = String::from_utf8_lossy(&scope_buf).to_string();

                    // Clone what we need from caller.data() to avoid borrow issues.
                    let plugin_id = caller.data().plugin_id.clone();
                    let allowed_scopes = caller.data().allowed_secret_scopes.clone();
                    let broker = Arc::clone(&caller.data().secret_broker);

                    match broker.request_token(&plugin_id, &scope, &allowed_scopes, 3600) {
                        Ok(token) => {
                            let token_bytes = token.token_id.as_bytes();
                            let write_len = token_bytes.len().min(out_len as usize);
                            if memory
                                .write(&mut caller, out_ptr as usize, &token_bytes[..write_len])
                                .is_err()
                            {
                                return -3;
                            }
                            write_len as i32
                        }
                        Err(BrokerError::ScopeNotAllowed { .. }) => -4,
                        Err(BrokerError::SecretNotFound { .. }) => -5,
                        Err(_) => -6,
                    }
                },
            )
            .map_err(|e| RuntimeError::ExecutionFailed(e.to_string()))?;

        let instance = linker
            .instantiate(&mut store, module)
            .map_err(|e| RuntimeError::ExecutionFailed(e.to_string()))?;

        Ok(WasmPluginInstance { store, instance })
    }
}

impl WasmPluginInstance {
    /// Call the guest `init()` export function if it exists.
    pub fn call_init(&mut self) -> Result<(), RuntimeError> {
        let init = self
            .instance
            .get_func(&mut self.store, "init")
            .ok_or_else(|| RuntimeError::ExportNotFound("init".to_string()))?;

        init.call(&mut self.store, &[], &mut [])
            .map_err(|e| RuntimeError::ExecutionFailed(e.to_string()))?;
        Ok(())
    }

    /// Call the guest `handle_event(ptr: i32, len: i32) -> i32` export function.
    /// The guest receives a JSON string pointer and returns a response pointer.
    pub fn call_handle_event(&mut self, event_json: &str) -> Result<String, RuntimeError> {
        let handle_event = self
            .instance
            .get_func(&mut self.store, "handle_event")
            .ok_or_else(|| RuntimeError::ExportNotFound("handle_event".to_string()))?;

        let memory = self
            .instance
            .get_export(&mut self.store, "memory")
            .and_then(|e| e.into_memory())
            .ok_or_else(|| RuntimeError::ExportNotFound("memory".to_string()))?;

        let input_offset = 0x100;
        let input_bytes = event_json.as_bytes();
        memory
            .write(&mut self.store, input_offset, input_bytes)
            .map_err(|e| RuntimeError::ExecutionFailed(e.to_string()))?;

        let mut results = [wasmtime::Val::I32(0)];
        handle_event
            .call(
                &mut self.store,
                &[
                    wasmtime::Val::I32(input_offset as i32),
                    wasmtime::Val::I32(input_bytes.len() as i32),
                ],
                &mut results,
            )
            .map_err(|e| RuntimeError::ExecutionFailed(e.to_string()))?;

        let output_ptr = match results[0] {
            wasmtime::Val::I32(v) => v,
            _ => {
                return Err(RuntimeError::InvalidArgument(
                    "invalid return type".to_string(),
                ))
            }
        };

        if output_ptr == 0 {
            return Ok(String::new());
        }

        // Read output: first 4 bytes as length (little-endian), then data.
        let mut len_buf = [0u8; 4];
        memory
            .read(&self.store, output_ptr as usize, &mut len_buf)
            .map_err(|e| RuntimeError::ExecutionFailed(e.to_string()))?;
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        memory
            .read(&self.store, output_ptr as usize + 4, &mut buf)
            .map_err(|e| RuntimeError::ExecutionFailed(e.to_string()))?;

        String::from_utf8(buf)
            .map_err(|e| RuntimeError::ExecutionFailed(format!("invalid utf-8: {e}")))
    }

    /// Get the logs collected from host function calls.
    pub fn logs(&self) -> &Vec<(i32, String)> {
        &self.store.data().logs
    }
}

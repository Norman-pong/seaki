use seaki_channel::broker::SecretBroker;
use seaki_channel::plugin::runtime::{PluginHostState, RuntimeError, WasmPluginRuntime};
use std::sync::Arc;

#[test]
fn runtime_create_success() {
    let runtime = WasmPluginRuntime::new();
    assert!(runtime.is_ok());
}

#[test]
fn load_valid_wasm_module() {
    let runtime = WasmPluginRuntime::new().unwrap();
    let wat = r#"
        (module
            (func (export "init"))
            (memory (export "memory") 1)
        )
    "#;
    let module = runtime.load_module(wat.as_bytes());
    assert!(module.is_ok());
}

#[test]
fn host_log_function_registered() {
    let runtime = WasmPluginRuntime::new().unwrap();
    let wat = r#"
        (module
            (import "env" "seaki_host_log" (func $log (param i32 i32 i32)))
            (func (export "init"))
            (func (export "handle_event") (param i32 i32) (result i32)
                i32.const 1
                i32.const 0
                i32.const 5
                call $log
                i32.const 0
            )
            (memory (export "memory") 1)
            (data (i32.const 0) "hello")
        )
    "#;
    let module = runtime.load_module(wat.as_bytes()).unwrap();
    let broker = Arc::new(SecretBroker::new());
    let host_state = PluginHostState {
        plugin_id: "test".to_string(),
        allowed_network_hosts: vec![],
        secret_broker: broker,
        allowed_secret_scopes: vec![],
        logs: vec![],
    };
    let mut instance = runtime.instantiate(&module, host_state).unwrap();
    instance.call_init().unwrap();
    instance.call_handle_event(r#"{"test":true}"#).unwrap();

    let logs = instance.logs();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0], (1, "hello".to_string()));
}

#[test]
fn guest_export_not_found() {
    let runtime = WasmPluginRuntime::new().unwrap();
    let wat = r#"
        (module
            (memory (export "memory") 1)
        )
    "#;
    let module = runtime.load_module(wat.as_bytes()).unwrap();
    let broker = Arc::new(SecretBroker::new());
    let host_state = PluginHostState {
        plugin_id: "test".to_string(),
        allowed_network_hosts: vec![],
        secret_broker: broker,
        allowed_secret_scopes: vec![],
        logs: vec![],
    };
    let mut instance = runtime.instantiate(&module, host_state).unwrap();
    let err = instance.call_init().unwrap_err();
    assert!(matches!(err, RuntimeError::ExportNotFound(ref s) if s == "init"));
}

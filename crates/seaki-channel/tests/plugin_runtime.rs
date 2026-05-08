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
    let host_state = PluginHostState::new("test", vec![], broker, vec![]);
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
    let host_state = PluginHostState::new("test", vec![], broker, vec![]);
    let mut instance = runtime.instantiate(&module, host_state).unwrap();
    let err = instance.call_init().unwrap_err();
    assert!(matches!(err, RuntimeError::ExportNotFound(ref s) if s == "init"));
}

#[test]
fn fuel_is_consumed_during_execution() {
    let runtime = WasmPluginRuntime::new().unwrap();
    // A module that does some work in init (calls host log 10 times)
    let wat = r#"
        (module
            (import "env" "seaki_host_log" (func $log (param i32 i32 i32)))
            (func (export "init")
                (local $i i32)
                i32.const 0
                local.set $i
                block
                    loop
                        local.get $i
                        i32.const 10
                        i32.ge_u
                        br_if 1
                        i32.const 1
                        i32.const 0
                        i32.const 5
                        call $log
                        local.get $i
                        i32.const 1
                        i32.add
                        local.set $i
                        br 0
                    end
                end
            )
            (memory (export "memory") 1)
            (data (i32.const 0) "hello")
        )
    "#;
    let module = runtime.load_module(wat.as_bytes()).unwrap();
    let broker = Arc::new(SecretBroker::new());
    let host_state = PluginHostState::new("test", vec![], broker, vec![]);
    let mut instance = runtime.instantiate(&module, host_state).unwrap();
    instance.call_init().unwrap();
    let fuel_after = instance.remaining_fuel().unwrap();
    // Fuel limit is 10_000_000; some should have been consumed.
    assert!(fuel_after < 10_000_000, "fuel should be consumed");
}

#[test]
fn fuel_limit_traps_infinite_loop() {
    let runtime = WasmPluginRuntime::new().unwrap();
    // A module with an infinite loop that will exhaust fuel.
    let wat = r#"
        (module
            (func (export "init")
                loop
                    br 0
                end
            )
            (memory (export "memory") 1)
        )
    "#;
    let module = runtime.load_module(wat.as_bytes()).unwrap();
    let broker = Arc::new(SecretBroker::new());
    let host_state = PluginHostState::new("test", vec![], broker, vec![]);
    let mut instance = runtime.instantiate(&module, host_state).unwrap();
    let err = instance.call_init().unwrap_err();
    // Should trap due to fuel exhaustion.
    assert!(
        matches!(err, RuntimeError::ExecutionFailed(_)),
        "expected execution failed trap, got: {err}"
    );
}

#[test]
fn host_log_ignores_negative_length() {
    let runtime = WasmPluginRuntime::new().unwrap();
    let wat = r#"
        (module
            (import "env" "seaki_host_log" (func $log (param i32 i32 i32)))
            (func (export "init"))
            (func (export "handle_event") (param i32 i32) (result i32)
                i32.const 1
                i32.const 0
                i32.const -1
                call $log
                i32.const 0
            )
            (memory (export "memory") 1)
            (data (i32.const 0) "hello")
        )
    "#;
    let module = runtime.load_module(wat.as_bytes()).unwrap();
    let broker = Arc::new(SecretBroker::new());
    let host_state = PluginHostState::new("test", vec![], broker, vec![]);
    let mut instance = runtime.instantiate(&module, host_state).unwrap();
    instance.call_handle_event(r#"{}"#).unwrap();
    // Negative length should be ignored, no panic or crash.
    assert!(instance.logs().is_empty());
}

#[test]
fn host_get_secret_rejects_negative_length() {
    let runtime = WasmPluginRuntime::new().unwrap();
    let wat = r#"
        (module
            (import "env" "seaki_host_get_secret" (func $get_secret (param i32 i32 i32 i32) (result i32)))
            (func (export "init"))
            (func (export "handle_event") (param i32 i32) (result i32)
                i32.const 0
                i32.const -1
                i32.const 100
                i32.const 256
                call $get_secret
                i32.const 0
                i32.lt_s
                if (result i32)
                    i32.const 0
                else
                    i32.const 0
                end
            )
            (memory (export "memory") 1)
            (data (i32.const 0) "slack")
        )
    "#;
    let module = runtime.load_module(wat.as_bytes()).unwrap();
    let broker = Arc::new(SecretBroker::new());
    let host_state = PluginHostState::new("test", vec![], broker, vec!["slack".to_string()]);
    let mut instance = runtime.instantiate(&module, host_state).unwrap();
    // Should not panic on negative scope_len; host function returns -7 which we
    // convert to 0 (empty string) inside the guest.
    let result = instance.call_handle_event(r#"{}"#).unwrap();
    assert_eq!(result, "");
}

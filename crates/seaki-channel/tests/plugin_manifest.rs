use seaki_channel::plugin::manifest::{parse_manifest, ManifestError, PluginRuntime};

#[test]
fn parse_valid_manifest() {
    let toml = r#"
id = "test-plugin"
name = "Test Plugin"
version = "1.0.0"
runtime = "wasm"
entry = "plugin.wasm"

[capabilities]
receive_message = true
send_message = true
send_file = true
thread_reply = true
drive_comment = true
interactive_card = true

[permissions]
network = ["api.example.com"]
local_files = ["/tmp"]
brokered_secret_scopes = ["slack", "discord"]
"#;
    let manifest = parse_manifest(toml).unwrap();
    assert_eq!(manifest.id, "test-plugin");
    assert_eq!(manifest.name, "Test Plugin");
    assert_eq!(manifest.version, "1.0.0");
    assert_eq!(manifest.runtime, PluginRuntime::Wasm);
    assert_eq!(manifest.entry, "plugin.wasm");
    assert!(manifest.capabilities.receive_message);
    assert!(manifest.capabilities.send_message);
    assert!(manifest.capabilities.send_file);
    assert!(manifest.capabilities.thread_reply);
    assert!(manifest.capabilities.drive_comment);
    assert!(manifest.capabilities.interactive_card);
    assert_eq!(manifest.permissions.network, vec!["api.example.com"]);
    assert_eq!(manifest.permissions.local_files, vec!["/tmp"]);
    assert_eq!(
        manifest.permissions.brokered_secret_scopes,
        vec!["slack", "discord"]
    );
}

#[test]
fn parse_minimal_manifest() {
    let toml = r#"
id = "minimal"
name = "Minimal"
version = "0.1.0"
runtime = "native"
entry = "main"
"#;
    let manifest = parse_manifest(toml).unwrap();
    assert_eq!(manifest.id, "minimal");
    assert_eq!(manifest.runtime, PluginRuntime::Native);
    assert!(!manifest.capabilities.receive_message);
    assert!(!manifest.capabilities.send_message);
    assert!(manifest.permissions.network.is_empty());
}

#[test]
fn parse_missing_id_fails() {
    let toml = r#"
name = "No ID"
version = "0.1.0"
runtime = "native"
entry = "main"
"#;
    let err = parse_manifest(toml).unwrap_err();
    assert!(matches!(err, ManifestError::MissingField(ref s) if s == "id"));
}

#[test]
fn parse_invalid_runtime_fails() {
    let toml = r#"
id = "bad"
name = "Bad"
version = "0.1.0"
runtime = "python"
entry = "main.py"
"#;
    let err = parse_manifest(toml).unwrap_err();
    assert!(matches!(err, ManifestError::ParseFailed(_)));
}

#[test]
fn capabilities_default_false() {
    let toml = r#"
id = "defaults"
name = "Defaults"
version = "0.1.0"
runtime = "wasm"
entry = "plugin.wasm"
"#;
    let manifest = parse_manifest(toml).unwrap();
    assert!(!manifest.capabilities.receive_message);
    assert!(!manifest.capabilities.send_message);
    assert!(!manifest.capabilities.send_file);
    assert!(!manifest.capabilities.thread_reply);
    assert!(!manifest.capabilities.drive_comment);
    assert!(!manifest.capabilities.interactive_card);
}

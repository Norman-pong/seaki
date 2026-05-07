use seaki_channel::plugin::registry::{PluginRegistry, PluginState, RegistryError};
use std::fs;
use std::path::Path;

fn create_plugin_dir(base: &Path, id: &str, runtime: &str, entry: &str, with_wasm: bool) {
    let dir = base.join(id);
    fs::create_dir_all(&dir).unwrap();
    let manifest = format!(
        r#"
id = "{}"
name = "{} Plugin"
version = "1.0.0"
runtime = "{}"
entry = "{}"
"#,
        id, id, runtime, entry
    );
    fs::write(dir.join("plugin.toml"), manifest).unwrap();
    if with_wasm {
        fs::write(dir.join(entry), b"\0asm").unwrap();
    }
}

#[test]
fn scan_empty_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let mut registry = PluginRegistry::new();
    let ids = registry.scan_dir(tmp.path()).unwrap();
    assert!(ids.is_empty());
}

#[test]
fn scan_dir_with_plugins() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(tmp.path(), "plugin-a", "native", "main", false);
    create_plugin_dir(tmp.path(), "plugin-b", "wasm", "plugin.wasm", true);

    let mut registry = PluginRegistry::new();
    let ids = registry.scan_dir(tmp.path()).unwrap();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"plugin-a".to_string()));
    assert!(ids.contains(&"plugin-b".to_string()));
}

#[test]
fn load_single_plugin() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(tmp.path(), "single", "native", "main", false);

    let mut registry = PluginRegistry::new();
    let plugin = registry.load(tmp.path().join("single").as_path()).unwrap();
    assert_eq!(plugin.manifest.id, "single");
    assert_eq!(plugin.state, PluginState::Discovered);
    assert!(plugin.wasm_path.is_none());
}

#[test]
fn duplicate_id_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(tmp.path(), "dup", "native", "main", false);

    let mut registry = PluginRegistry::new();
    registry.load(tmp.path().join("dup").as_path()).unwrap();
    let err = registry.load(tmp.path().join("dup").as_path()).unwrap_err();
    assert!(matches!(err, RegistryError::DuplicateId(ref s) if s == "dup"));
}

#[test]
fn wasm_file_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(tmp.path(), "wasm-missing", "wasm", "plugin.wasm", false);

    let mut registry = PluginRegistry::new();
    let err = registry
        .load(tmp.path().join("wasm-missing").as_path())
        .unwrap_err();
    assert!(matches!(err, RegistryError::WasmNotFound { .. }));
}

#[test]
fn list_by_capability_filter() {
    let tmp = tempfile::tempdir().unwrap();
    let dir_a = tmp.path().join("plugin-a");
    fs::create_dir_all(&dir_a).unwrap();
    fs::write(
        dir_a.join("plugin.toml"),
        r#"
id = "plugin-a"
name = "A"
version = "1.0.0"
runtime = "native"
entry = "main"

[capabilities]
receive_message = true
"#,
    )
    .unwrap();

    let dir_b = tmp.path().join("plugin-b");
    fs::create_dir_all(&dir_b).unwrap();
    fs::write(
        dir_b.join("plugin.toml"),
        r#"
id = "plugin-b"
name = "B"
version = "1.0.0"
runtime = "native"
entry = "main"

[capabilities]
send_message = true
"#,
    )
    .unwrap();

    let mut registry = PluginRegistry::new();
    registry.scan_dir(tmp.path()).unwrap();

    let receive_plugins = registry.list_by_capability("receive_message");
    assert_eq!(receive_plugins.len(), 1);
    assert_eq!(receive_plugins[0].manifest.id, "plugin-a");

    let send_plugins = registry.list_by_capability("send_message");
    assert_eq!(send_plugins.len(), 1);
    assert_eq!(send_plugins[0].manifest.id, "plugin-b");

    let file_plugins = registry.list_by_capability("send_file");
    assert!(file_plugins.is_empty());
}

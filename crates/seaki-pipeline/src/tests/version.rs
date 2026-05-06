use crate::version::{check_compatibility, ManifestVersion, VersionCompatibility, VersionedManifest, VersionedRegistry};

#[test]
fn versioned_registry_ignores_deprecated() {
    let mut registry = VersionedRegistry::new();
    registry.register(VersionedManifest {
        command_id: "wiki.search".to_string(),
        version: ManifestVersion::new(1, 0, 0),
        schema_hash: "old".to_string(),
        deprecated: true,
    });
    registry.register(VersionedManifest {
        command_id: "wiki.search".to_string(),
        version: ManifestVersion::new(1, 1, 0),
        schema_hash: "new".to_string(),
        deprecated: false,
    });

    let resolved = registry.resolve("wiki.search", &ManifestVersion::new(1, 0, 0));
    assert!(resolved.is_some());
    assert_eq!(resolved.unwrap().schema_hash, "new");
}

#[test]
fn versioned_registry_returns_none_for_incompatible() {
    let mut registry = VersionedRegistry::new();
    registry.register(VersionedManifest {
        command_id: "wiki.search".to_string(),
        version: ManifestVersion::new(1, 0, 0),
        schema_hash: "v1".to_string(),
        deprecated: false,
    });

    let incompatible = registry.resolve("wiki.search", &ManifestVersion::new(2, 0, 0));
    assert!(incompatible.is_none());
}

#[test]
fn versioned_registry_selects_best_match() {
    let mut registry = VersionedRegistry::new();
    registry.register(VersionedManifest {
        command_id: "wiki.search".to_string(),
        version: ManifestVersion::new(1, 0, 0),
        schema_hash: "v1".to_string(),
        deprecated: false,
    });
    registry.register(VersionedManifest {
        command_id: "wiki.search".to_string(),
        version: ManifestVersion::new(1, 2, 0),
        schema_hash: "v12".to_string(),
        deprecated: false,
    });
    registry.register(VersionedManifest {
        command_id: "wiki.search".to_string(),
        version: ManifestVersion::new(1, 5, 0),
        schema_hash: "v15".to_string(),
        deprecated: false,
    });

    let resolved = registry.resolve("wiki.search", &ManifestVersion::new(1, 3, 0));
    assert!(resolved.is_some());
    assert_eq!(resolved.unwrap().version, ManifestVersion::new(1, 5, 0));
}

#[test]
fn compatibility_backward_compatible() {
    let v1_0 = ManifestVersion::new(1, 0, 0);
    let v1_2 = ManifestVersion::new(1, 2, 0);

    assert_eq!(
        check_compatibility(&v1_0, &v1_2),
        VersionCompatibility::BackwardCompatible
    );
}

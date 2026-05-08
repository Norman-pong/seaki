//! Command manifest versioning and compatibility checking.

use serde::{Deserialize, Serialize};

/// Semantic version of a command manifest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ManifestVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl ManifestVersion {
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = parts[2].parse().ok()?;
        Some(Self::new(major, minor, patch))
    }
}

impl std::fmt::Display for ManifestVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Compatibility verdict between two manifest versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionCompatibility {
    /// Fully compatible: same major, minor >= required.
    Compatible,
    /// Backward compatible: same major, minor < required (some features missing).
    BackwardCompatible,
    /// Incompatible: major version differs.
    Incompatible,
}

/// Check compatibility of a runtime version against a required version.
///
/// Rules:
/// - Same major, runtime minor >= required minor → Compatible
/// - Same major, runtime minor < required minor → BackwardCompatible
/// - Different major → Incompatible
#[must_use]
pub fn check_compatibility(
    runtime: &ManifestVersion,
    required: &ManifestVersion,
) -> VersionCompatibility {
    if runtime.major != required.major {
        return VersionCompatibility::Incompatible;
    }
    if runtime.minor > required.minor {
        VersionCompatibility::Compatible
    } else if runtime.minor < required.minor {
        VersionCompatibility::BackwardCompatible
    } else {
        // Same minor: patch level determines compatibility.
        if runtime.patch >= required.patch {
            VersionCompatibility::Compatible
        } else {
            VersionCompatibility::BackwardCompatible
        }
    }
}

/// A versioned command manifest entry for the registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionedManifest {
    pub command_id: String,
    pub version: ManifestVersion,
    pub schema_hash: String,
    pub deprecated: bool,
}

/// Versioned manifest registry.
#[derive(Debug, Clone, Default)]
pub struct VersionedRegistry {
    manifests: std::collections::HashMap<String, Vec<VersionedManifest>>,
}

impl VersionedRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            manifests: std::collections::HashMap::new(),
        }
    }

    /// Register a versioned manifest.
    pub fn register(&mut self, manifest: VersionedManifest) {
        self.manifests
            .entry(manifest.command_id.clone())
            .or_default()
            .push(manifest);
    }

    /// Find the best matching manifest for a command and version requirement.
    #[must_use]
    pub fn resolve(
        &self,
        command_id: &str,
        required_version: &ManifestVersion,
    ) -> Option<&VersionedManifest> {
        let versions = self.manifests.get(command_id)?;
        versions
            .iter()
            .filter(|m| !m.deprecated)
            .filter(|m| {
                check_compatibility(&m.version, required_version)
                    != VersionCompatibility::Incompatible
            })
            .max_by_key(|m| &m.version)
    }

    /// List all versions of a command.
    #[must_use]
    pub fn list_versions(&self, command_id: &str) -> Vec<&VersionedManifest> {
        self.manifests
            .get(command_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing() {
        let v = ManifestVersion::parse("1.2.3").unwrap();
        assert_eq!(v, ManifestVersion::new(1, 2, 3));
        assert!(ManifestVersion::parse("1.2").is_none());
        assert!(ManifestVersion::parse("a.b.c").is_none());
    }

    #[test]
    fn compatibility_checks() {
        let v1_0 = ManifestVersion::new(1, 0, 0);
        let v1_2 = ManifestVersion::new(1, 2, 0);
        let v2_0 = ManifestVersion::new(2, 0, 0);

        assert_eq!(
            check_compatibility(&v1_2, &v1_0),
            VersionCompatibility::Compatible
        );
        assert_eq!(
            check_compatibility(&v1_0, &v1_2),
            VersionCompatibility::BackwardCompatible
        );
        assert_eq!(
            check_compatibility(&v2_0, &v1_0),
            VersionCompatibility::Incompatible
        );
    }

    #[test]
    fn versioned_registry_resolution() {
        let mut registry = VersionedRegistry::new();
        registry.register(VersionedManifest {
            command_id: "wiki.search".to_string(),
            version: ManifestVersion::new(1, 0, 0),
            schema_hash: "abc".to_string(),
            deprecated: false,
        });
        registry.register(VersionedManifest {
            command_id: "wiki.search".to_string(),
            version: ManifestVersion::new(1, 2, 0),
            schema_hash: "def".to_string(),
            deprecated: false,
        });
        registry.register(VersionedManifest {
            command_id: "wiki.search".to_string(),
            version: ManifestVersion::new(2, 0, 0),
            schema_hash: "ghi".to_string(),
            deprecated: false,
        });

        let resolved = registry.resolve("wiki.search", &ManifestVersion::new(1, 1, 0));
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().version, ManifestVersion::new(1, 2, 0));

        let incompatible = registry.resolve("wiki.search", &ManifestVersion::new(3, 0, 0));
        assert!(incompatible.is_none());
    }
}

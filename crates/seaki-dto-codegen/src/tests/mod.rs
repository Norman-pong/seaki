use super::{check_generated_file, generate_typescript, schema_hash};
use std::path::Path;

#[test]
fn schema_hash_is_sha256_hex() {
    let hash = schema_hash();

    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|character| character.is_ascii_hexdigit()));
}

#[test]
fn generated_typescript_is_current() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate must live under crates/");
    let generated_path = workspace_root.join("packages/dto/src/generated.ts");

    check_generated_file(generated_path)
        .expect("generated DTO file is stale or schema hash mismatch");
}

#[test]
fn generated_typescript_contains_frontend_minimum_dtos() {
    let generated = generate_typescript();

    for required in [
        "CapabilityGrantRequestDTO",
        "SourceManifestDTO",
        "SourceCardDTO",
        "AnnotationDTO",
        "WikiPatchProposalDTO",
        "ApprovalRequestDTO",
        "SearchResultDTO",
        "CitationRefDTO",
        "ChannelAnswerDTO",
        "OutboxItemDTO",
    ] {
        assert!(
            generated.contains(required),
            "missing generated DTO: {required}"
        );
    }
}

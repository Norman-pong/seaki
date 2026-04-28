pub const CORE_AUTHORITY: &str = "policy-approved-core";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreRecordKind {
    Task,
    Transaction,
    AuditEvent,
}

pub fn owns_record_kind(kind: CoreRecordKind) -> bool {
    matches!(
        kind,
        CoreRecordKind::Task | CoreRecordKind::Transaction | CoreRecordKind::AuditEvent
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_names_its_authority_boundary() {
        assert_eq!(CORE_AUTHORITY, "policy-approved-core");
        assert!(owns_record_kind(CoreRecordKind::Transaction));
    }
}

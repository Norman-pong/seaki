use super::*;

#[test]
fn core_names_its_authority_boundary() {
    assert_eq!(CORE_AUTHORITY, "policy-approved-core");
    assert!(owns_record_kind(CoreRecordKind::Transaction));
}

#[test]
fn file_backed_ledger_enables_wal_journal_mode() {
    let file = NamedTempFile::new().expect("temp sqlite file");
    let ledger = CoreLedger::open(file.path()).expect("ledger opens");

    assert_eq!(
        ledger
            .journal_mode()
            .expect("journal mode loads")
            .to_lowercase(),
        "wal"
    );
}

use crate::retention::RetentionScheduler;

#[test]
fn retention_formula_basic() {
    // elapsed = 0 时 retention 应为 1.0
    assert!((RetentionScheduler::retention(0.0, 1.0) - 1.0).abs() < f64::EPSILON);

    // elapsed = 1, stability = 1 时 retention 约为 e^-1 ≈ 0.3679
    let r = RetentionScheduler::retention(1.0, 1.0);
    assert!((r - std::f64::consts::E.powf(-1.0)).abs() < 1e-10);
    assert!((r - 0.367_879_441_17).abs() < 1e-10);
}

#[test]
fn retention_is_due_correctly() {
    let now = 1_000_000u64;
    let last = now - 86400; // 1 天前复习

    // stability=1, threshold=0.5: retention(1,1)=0.368 <= 0.5，到期
    assert!(RetentionScheduler::is_due(last, 1.0, 0.5, now));

    // stability=10, threshold=0.5: retention(0.1,10)=0.990 > 0.5，未到期
    assert!(!RetentionScheduler::is_due(last, 10.0, 0.5, now));
}

#[test]
fn retention_days_to_threshold() {
    // stability=1, threshold=e^-1 -> days = 1
    let days = RetentionScheduler::days_to_threshold(1.0, std::f64::consts::E.powf(-1.0));
    assert!((days - 1.0).abs() < 1e-10);

    // stability=2, threshold=0.5 -> days = -2 * ln(0.5) ≈ 1.386
    let days = RetentionScheduler::days_to_threshold(2.0, 0.5);
    assert!((days - 1.386_294_361_12).abs() < 1e-10);
}

#[test]
fn retention_days_to_threshold_invalid() {
    // stability <= 0
    assert_eq!(RetentionScheduler::days_to_threshold(0.0, 0.5), 0.0);
    assert_eq!(RetentionScheduler::days_to_threshold(-1.0, 0.5), 0.0);

    // threshold <= 0 -> saturate to MAX
    assert_eq!(RetentionScheduler::days_to_threshold(1.0, 0.0), f64::MAX);
    assert_eq!(RetentionScheduler::days_to_threshold(1.0, -0.1), f64::MAX);

    // threshold >= 1 -> 0
    assert_eq!(RetentionScheduler::days_to_threshold(1.0, 1.0), 0.0);
    assert_eq!(RetentionScheduler::days_to_threshold(1.0, 1.5), 0.0);
}

#[test]
fn retention_is_due_boundary_threshold() {
    let now = 1_000_000u64;
    let last = now - 86400;

    // threshold <= 0 -> always false
    assert!(!RetentionScheduler::is_due(last, 1.0, 0.0, now));
    assert!(!RetentionScheduler::is_due(last, 1.0, -0.5, now));

    // threshold >= 1 -> always true
    assert!(RetentionScheduler::is_due(last, 10.0, 1.0, now));
    assert!(RetentionScheduler::is_due(last, 10.0, 1.5, now));
}

#[test]
fn retention_negative_stability() {
    // negative stability should not panic and return clamped values
    let r = RetentionScheduler::retention(1.0, -1.0);
    assert_eq!(r, 0.0);
}

#[test]
fn retention_next_review_at_negative_stability() {
    let last = 1_000_000u64;
    // negative stability should act like 0
    assert_eq!(RetentionScheduler::next_review_at(last, -5.0), last);
}

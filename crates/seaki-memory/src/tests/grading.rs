use crate::grading::{Grade, GradingAction, GradingEngine};
use crate::review_card::{CardDifficulty, ReviewCard};
use seaki_index::IndexScope;

fn dummy_card(difficulty: CardDifficulty, stability: f64, review_count: u32) -> ReviewCard {
    ReviewCard {
        card_id: "c1".to_string(),
        memory_id: None,
        scope: IndexScope::new("ws", "ac"),
        question: "q".to_string(),
        answer: "a".to_string(),
        source: None,
        created_at: 0,
        last_reviewed_at: None,
        stability_days: stability,
        retention_threshold: difficulty.default_threshold(),
        next_review_at: 0,
        review_count,
        difficulty,
    }
}

#[test]
fn grading_again_reduces_stability() {
    let engine = GradingEngine::new();
    let card = dummy_card(CardDifficulty::Medium, 1.0, 0);
    let result = engine.grade(&card, Grade::Again, 1000);

    assert_eq!(result.new_stability_days, 0.3); // 1.0 * 0.3
    assert_eq!(result.new_next_review_at, 1600); // now + 600
    assert_eq!(result.review_count, 1);
    assert_eq!(result.recommended_action, GradingAction::Continue);
}

#[test]
fn grading_easy_increases_stability() {
    let engine = GradingEngine::new();
    let card = dummy_card(CardDifficulty::Medium, 1.0, 2);
    let result = engine.grade(&card, Grade::Easy, 1000);

    // multiplier = 2.5 + 0.2 * 2 = 2.9
    assert!((result.new_stability_days - 2.9).abs() < 1e-10);
    assert_eq!(result.review_count, 3);
    assert_eq!(result.recommended_action, GradingAction::Continue);
}

#[test]
fn grading_good_moderate_increase() {
    let engine = GradingEngine::new();
    let card = dummy_card(CardDifficulty::Medium, 1.0, 1);
    let result = engine.grade(&card, Grade::Good, 1000);

    // multiplier = 1.5 + 0.1 * 1 = 1.6
    assert!((result.new_stability_days - 1.6).abs() < 1e-10);
    assert_eq!(result.review_count, 2);
    assert_eq!(result.recommended_action, GradingAction::Continue);
}

#[test]
fn grading_hard_slight_reduction() {
    let engine = GradingEngine::new();
    let card = dummy_card(CardDifficulty::Medium, 1.0, 5);
    let result = engine.grade(&card, Grade::Hard, 1000);

    assert_eq!(result.new_stability_days, 0.8); // 1.0 * 0.8
    assert_eq!(result.review_count, 6);
    assert_eq!(result.recommended_action, GradingAction::Continue);
}

#[test]
fn grading_critical_again_heavy_penalty() {
    let engine = GradingEngine::new();
    let card = dummy_card(CardDifficulty::Critical, 1.0, 0);
    let result = engine.grade(&card, Grade::Again, 1000);

    // Critical + Again = 0.15 multiplier
    assert_eq!(result.new_stability_days, 0.15);
    assert_eq!(result.new_next_review_at, 1600);
    assert_eq!(result.review_count, 1);
    assert_eq!(result.recommended_action, GradingAction::Continue);
}

#[test]
fn grading_relink_after_repeated_failure() {
    let engine = GradingEngine::new();
    let card = dummy_card(CardDifficulty::Medium, 1.0, 3);
    let result = engine.grade(&card, Grade::Again, 1000);

    assert_eq!(result.review_count, 4);
    assert_eq!(result.recommended_action, GradingAction::RelinkToSource);
}

#[test]
fn grading_again_schedules_soon() {
    let engine = GradingEngine::new();
    let card = dummy_card(CardDifficulty::Easy, 2.0, 0);
    let result = engine.grade(&card, Grade::Again, 86400);

    assert_eq!(result.new_next_review_at, 87000); // 86400 + 600
}

#[test]
fn grading_easy_difficulty_bonus() {
    let engine = GradingEngine::new();
    let card = dummy_card(CardDifficulty::Easy, 2.0, 1);
    let result = engine.grade(&card, Grade::Easy, 1000);

    // Easy + Easy multiplier = 3.0 + 0.2 * 1 = 3.2
    assert!((result.new_stability_days - 6.4).abs() < 1e-10); // 2.0 * 3.2
}

#[test]
fn grading_review_soon_when_stability_very_low() {
    let engine = GradingEngine::new();
    let card = dummy_card(CardDifficulty::Hard, 0.05, 0);
    let result = engine.grade(&card, Grade::Hard, 1000);

    // 0.05 * 0.8 = 0.04, which is < 0.1
    assert!((result.new_stability_days - 0.04).abs() < 1e-10);
    assert_eq!(result.recommended_action, GradingAction::ReviewSoon);
}

#[test]
fn grading_stability_minimum_floor() {
    let engine = GradingEngine::new();
    let card = dummy_card(CardDifficulty::Medium, 0.01, 0);
    let result = engine.grade(&card, Grade::Again, 1000);

    // 0.01 * 0.3 = 0.003, but floor is 0.01
    assert_eq!(result.new_stability_days, 0.01);
}

//! Tests for advanced tab completion customization

use super::*;
use crate::completer::ShellHelper;

#[test]
fn fuzzy_score___exact_prefix___returns_highest_score() {
    let score = fuzzy_score("demo", "demo_actor_1");
    assert!(score > 1000, "Exact prefix should score > 1000");
}

#[test]
fn fuzzy_score___contains_substring___returns_medium_score() {
    let score = fuzzy_score("actor", "demo_actor_1");
    assert!(
        (500..1000).contains(&score),
        "Contains should score 500-999"
    );
}

#[test]
fn fuzzy_score___fuzzy_match___returns_positive_score() {
    let score = fuzzy_score("da1", "demo_actor_1");
    assert!(score > 0, "Fuzzy match should have positive score");
}

#[test]
fn fuzzy_score___no_match___returns_zero() {
    let score = fuzzy_score("xyz", "demo_actor_1");
    assert_eq!(score, 0, "No match should return 0");
}

#[test]
fn fuzzy_score___prefix_beats_contains___returns_higher_score() {
    let prefix_score = fuzzy_score("demo", "demo_actor_1");
    let contains_score = fuzzy_score("demo", "actor_demo");
    assert!(
        prefix_score > contains_score,
        "Prefix match should score higher than contains"
    );
}

#[test]
fn CaseInsensitiveCompletion___lowercase_input___matches_mixed_case() {
    let helper = ShellHelper::new();
    let candidates = vec!["DemoActor".to_string(), "TestActor".to_string()];
    let results = helper.complete_case_insensitive("demo", &candidates);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].display, "DemoActor");
}

#[test]
fn CaseInsensitiveCompletion___uppercase_input___matches_mixed_case() {
    let helper = ShellHelper::new();
    let candidates = vec!["DemoActor".to_string(), "TestActor".to_string()];
    let results = helper.complete_case_insensitive("DEMO", &candidates);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].display, "DemoActor");
}

#[test]
fn CaseInsensitiveCompletion___no_match___returns_empty() {
    let helper = ShellHelper::new();
    let candidates = vec!["DemoActor".to_string(), "TestActor".to_string()];
    let results = helper.complete_case_insensitive("xyz", &candidates);
    assert!(results.is_empty());
}

#[test]
fn MultiColumnDisplay___empty_items___returns_empty_string() {
    let helper = ShellHelper::new();
    let result = helper.format_columns(vec![], 80);
    assert!(result.is_empty());
}

#[test]
fn MultiColumnDisplay___single_item___formats_correctly() {
    let helper = ShellHelper::new();
    let result = helper.format_columns(vec!["test".to_string()], 80);
    assert!(result.contains("test"));
}

#[test]
fn DescriptiveCompletion___matching_prefix___returns_descriptions() {
    let helper = ShellHelper::new();
    let results = helper.complete_with_descriptions("he");
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.replacement == "help"));
}

#[test]
fn DescriptiveCompletion___no_match___returns_empty() {
    let helper = ShellHelper::new();
    let results = helper.complete_with_descriptions("xyz");
    assert!(results.is_empty());
}

#[test]
fn ContextualHints___send_command___returns_hint() {
    let helper = ShellHelper::new();
    let hint = helper.get_hint("send", 4);
    assert!(hint.is_some());
    assert!(hint.unwrap().contains("actor"));
}

#[test]
fn ContextualHints___unknown_command___returns_none() {
    let helper = ShellHelper::new();
    let hint = helper.get_hint("unknown", 7);
    assert!(hint.is_none());
}

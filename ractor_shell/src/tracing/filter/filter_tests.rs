//! Tests for TraceFilter
#![allow(non_snake_case)]

use super::*;

#[test]
fn filter_new___empty___matches_nothing() {
    let filter = TraceFilter::new();

    assert!(!filter.is_active());
    assert!(!filter.matches(Some("worker_1")));
    assert!(!filter.matches(None));
}

#[test]
fn filter_all___matches_everything() {
    let filter = TraceFilter::all();

    assert!(filter.is_active());
    assert!(filter.matches(Some("worker_1")));
    assert!(filter.matches(Some("supervisor")));
    assert!(filter.matches(None)); // Even unnamed actors
}

#[test]
fn filter_add_pattern___exact_match___matches() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("worker_1");

    assert!(filter.is_active());
    assert!(filter.matches(Some("worker_1")));
    assert!(!filter.matches(Some("worker_2")));
    assert!(!filter.matches(Some("supervisor")));
}

#[test]
fn filter_add_pattern___glob_star___matches_prefix() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("worker_*");

    assert!(filter.matches(Some("worker_1")));
    assert!(filter.matches(Some("worker_foo")));
    assert!(filter.matches(Some("worker_")));
    assert!(!filter.matches(Some("supervisor")));
    assert!(!filter.matches(Some("my_worker_1"))); // Doesn't start with worker_
}

#[test]
fn filter_add_pattern___glob_question___matches_single_char() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("worker_?");

    assert!(filter.matches(Some("worker_1")));
    assert!(filter.matches(Some("worker_a")));
    assert!(!filter.matches(Some("worker_12"))); // Two chars
    assert!(!filter.matches(Some("worker_"))); // Zero chars
}

#[test]
fn filter_add_pattern___star___enables_trace_all() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("*");

    assert!(filter.is_active());
    assert!(filter.matches(Some("anything")));
    assert!(filter.matches(None));

    let patterns = filter.patterns();
    assert_eq!(patterns, vec!["*"]);
}

#[test]
fn filter_remove_pattern___existing___removes() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("worker_1");
    filter.add_pattern("worker_2");

    assert!(filter.remove_pattern("worker_1"));
    assert!(!filter.matches(Some("worker_1")));
    assert!(filter.matches(Some("worker_2")));
}

#[test]
fn filter_remove_pattern___nonexistent___returns_false() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("worker_1");

    assert!(!filter.remove_pattern("nonexistent"));
    assert!(filter.matches(Some("worker_1")));
}

#[test]
fn filter_remove_pattern___star___disables_trace_all() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("*");

    assert!(filter.remove_pattern("*"));
    assert!(!filter.is_active());
    assert!(!filter.matches(Some("anything")));
}

#[test]
fn filter_clear___removes_all() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("worker_*");
    filter.add_pattern("supervisor");
    filter.add_pattern("*");

    filter.clear();

    assert!(!filter.is_active());
    assert!(!filter.matches(Some("worker_1")));
    assert!(!filter.matches(Some("supervisor")));
}

#[test]
fn filter_patterns___returns_all_patterns() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("worker_*");
    filter.add_pattern("supervisor");

    let patterns = filter.patterns();
    assert_eq!(patterns.len(), 2);
    assert!(patterns.contains(&"worker_*"));
    assert!(patterns.contains(&"supervisor"));
}

#[test]
fn filter_matches_id___works_for_actor_ids() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("0.*");

    assert!(filter.matches_id("0.1"));
    assert!(filter.matches_id("0.123"));
    assert!(!filter.matches_id("1.1"));
}

#[test]
fn filter_matches___none_name___returns_false_unless_trace_all() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("worker_*");

    assert!(!filter.matches(None));

    filter.add_pattern("*");
    assert!(filter.matches(None));
}

#[test]
fn filter_multiple_patterns___any_match___returns_true() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("worker_*");
    filter.add_pattern("supervisor_*");
    filter.add_pattern("monitor");

    assert!(filter.matches(Some("worker_1")));
    assert!(filter.matches(Some("supervisor_main")));
    assert!(filter.matches(Some("monitor")));
    assert!(!filter.matches(Some("other")));
}

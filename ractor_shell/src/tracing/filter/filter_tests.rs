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

// MinLevel tests

#[test]
fn min_level___from_str___parses_valid_levels() {
    assert_eq!("TRACE".parse::<MinLevel>().unwrap(), MinLevel::Trace);
    assert_eq!("trace".parse::<MinLevel>().unwrap(), MinLevel::Trace);
    assert_eq!("DEBUG".parse::<MinLevel>().unwrap(), MinLevel::Debug);
    assert_eq!("debug".parse::<MinLevel>().unwrap(), MinLevel::Debug);
    assert_eq!("INFO".parse::<MinLevel>().unwrap(), MinLevel::Info);
    assert_eq!("info".parse::<MinLevel>().unwrap(), MinLevel::Info);
    assert_eq!("WARN".parse::<MinLevel>().unwrap(), MinLevel::Warn);
    assert_eq!("warn".parse::<MinLevel>().unwrap(), MinLevel::Warn);
    assert_eq!("WARNING".parse::<MinLevel>().unwrap(), MinLevel::Warn);
    assert_eq!("ERROR".parse::<MinLevel>().unwrap(), MinLevel::Error);
    assert_eq!("error".parse::<MinLevel>().unwrap(), MinLevel::Error);
}

#[test]
fn min_level___from_str___rejects_invalid_levels() {
    assert!("INVALID".parse::<MinLevel>().is_err());
    assert!("".parse::<MinLevel>().is_err());
    assert!("123".parse::<MinLevel>().is_err());
}

#[test]
fn min_level___display___formats_correctly() {
    assert_eq!(MinLevel::Trace.to_string(), "TRACE");
    assert_eq!(MinLevel::Debug.to_string(), "DEBUG");
    assert_eq!(MinLevel::Info.to_string(), "INFO");
    assert_eq!(MinLevel::Warn.to_string(), "WARN");
    assert_eq!(MinLevel::Error.to_string(), "ERROR");
}

#[test]
fn min_level_trace___allows___all_levels() {
    let level = MinLevel::Trace;

    assert!(level.allows(Level::TRACE));
    assert!(level.allows(Level::DEBUG));
    assert!(level.allows(Level::INFO));
    assert!(level.allows(Level::WARN));
    assert!(level.allows(Level::ERROR));
}

#[test]
fn min_level_debug___allows___debug_and_above() {
    let level = MinLevel::Debug;

    assert!(!level.allows(Level::TRACE));
    assert!(level.allows(Level::DEBUG));
    assert!(level.allows(Level::INFO));
    assert!(level.allows(Level::WARN));
    assert!(level.allows(Level::ERROR));
}

#[test]
fn min_level_info___allows___info_and_above() {
    let level = MinLevel::Info;

    assert!(!level.allows(Level::TRACE));
    assert!(!level.allows(Level::DEBUG));
    assert!(level.allows(Level::INFO));
    assert!(level.allows(Level::WARN));
    assert!(level.allows(Level::ERROR));
}

#[test]
fn min_level_warn___allows___warn_and_above() {
    let level = MinLevel::Warn;

    assert!(!level.allows(Level::TRACE));
    assert!(!level.allows(Level::DEBUG));
    assert!(!level.allows(Level::INFO));
    assert!(level.allows(Level::WARN));
    assert!(level.allows(Level::ERROR));
}

#[test]
fn min_level_error___allows___only_error() {
    let level = MinLevel::Error;

    assert!(!level.allows(Level::TRACE));
    assert!(!level.allows(Level::DEBUG));
    assert!(!level.allows(Level::INFO));
    assert!(!level.allows(Level::WARN));
    assert!(level.allows(Level::ERROR));
}

#[test]
fn filter_new___default_level___is_trace() {
    let filter = TraceFilter::new();

    assert_eq!(filter.min_level(), MinLevel::Trace);
}

#[test]
fn filter_set_min_level___updates_level() {
    let mut filter = TraceFilter::new();

    filter.set_min_level(MinLevel::Info);

    assert_eq!(filter.min_level(), MinLevel::Info);
}

#[test]
fn filter_level_allowed___respects_min_level() {
    let mut filter = TraceFilter::new();
    filter.set_min_level(MinLevel::Info);

    assert!(!filter.level_allowed(Level::TRACE));
    assert!(!filter.level_allowed(Level::DEBUG));
    assert!(filter.level_allowed(Level::INFO));
    assert!(filter.level_allowed(Level::WARN));
    assert!(filter.level_allowed(Level::ERROR));
}

#[test]
fn filter_clear___preserves_min_level() {
    let mut filter = TraceFilter::new();
    filter.set_min_level(MinLevel::Error);
    filter.add_pattern("*");

    filter.clear();

    assert_eq!(filter.min_level(), MinLevel::Error);
}

// matches_any_field tests

#[test]
fn filter_matches_any_field___empty_filter___returns_false() {
    let filter = TraceFilter::new();
    let fields = vec![
        ("node".to_string(), "node_a".to_string()),
        ("term".to_string(), "1".to_string()),
    ];

    assert!(!filter.matches_any_field(&fields));
}

#[test]
fn filter_matches_any_field___trace_all___returns_true() {
    let filter = TraceFilter::all();
    let fields = vec![("node".to_string(), "node_a".to_string())];

    assert!(filter.matches_any_field(&fields));
}

#[test]
fn filter_matches_any_field___exact_match___returns_true() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("node_a");
    let fields = vec![
        ("node".to_string(), "node_a".to_string()),
        ("term".to_string(), "1".to_string()),
    ];

    assert!(filter.matches_any_field(&fields));
}

#[test]
fn filter_matches_any_field___glob_match___returns_true() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("node_*");
    let fields = vec![
        ("node".to_string(), "node_b".to_string()),
        ("term".to_string(), "5".to_string()),
    ];

    assert!(filter.matches_any_field(&fields));
}

#[test]
fn filter_matches_any_field___no_match___returns_false() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("worker_*");
    let fields = vec![
        ("node".to_string(), "node_a".to_string()),
        ("term".to_string(), "1".to_string()),
    ];

    assert!(!filter.matches_any_field(&fields));
}

#[test]
fn filter_matches_any_field___quoted_value___strips_quotes() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("node_a");
    let fields = vec![("node".to_string(), "\"node_a\"".to_string())];

    assert!(filter.matches_any_field(&fields));
}

#[test]
fn filter_matches_any_field___empty_fields___returns_false() {
    let mut filter = TraceFilter::new();
    filter.add_pattern("node_*");
    let fields: Vec<(String, String)> = vec![];

    assert!(!filter.matches_any_field(&fields));
}

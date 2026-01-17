//! Tests for table formatting utilities

use super::*;
use tabled::Tabled;

#[derive(Tabled)]
struct TestRow {
    name: String,
    value: i32,
}

#[test]
fn build_table___empty_rows___returns_empty_string() {
    let rows: Vec<TestRow> = vec![];
    let result = build_table(rows);
    assert!(result.is_empty());
}

#[test]
fn build_table___single_row___returns_formatted_table() {
    let rows = vec![TestRow {
        name: "test".to_string(),
        value: 42,
    }];
    let result = build_table(rows);
    assert!(result.contains("test"));
    assert!(result.contains("42"));
}

#[test]
fn get_terminal_width___always___returns_reasonable_value() {
    let width = get_terminal_width();
    // Should be at least the default or actual terminal width
    assert!(width >= 40);
}

#[test]
fn PaginatedResult___page_0___returns_first_items() {
    let items: Vec<i32> = (0..50).collect();
    let result = PaginatedResult::from_slice(&items, 0, 10);

    assert_eq!(result.items.len(), 10);
    assert_eq!(*result.items[0], 0);
    assert_eq!(result.page, 0);
    assert_eq!(result.total_pages, 5);
    assert_eq!(result.total_items, 50);
}

#[test]
fn PaginatedResult___last_page___returns_remaining_items() {
    let items: Vec<i32> = (0..25).collect();
    let result = PaginatedResult::from_slice(&items, 2, 10);

    assert_eq!(result.items.len(), 5);
    assert_eq!(*result.items[0], 20);
}

#[test]
fn PaginatedResult___beyond_range___returns_empty() {
    let items: Vec<i32> = (0..10).collect();
    let result = PaginatedResult::from_slice(&items, 5, 10);

    assert!(result.items.is_empty());
}

#[test]
fn format_pagination_info___single_page___shows_all_items() {
    let items: Vec<i32> = (0..5).collect();
    let result = PaginatedResult::from_slice(&items, 0, 10);
    let info = format_pagination_info(&result);

    assert!(info.contains("all 5 items"));
}

#[test]
fn format_pagination_info___multi_page___shows_range() {
    let items: Vec<i32> = (0..25).collect();
    let result = PaginatedResult::from_slice(&items, 1, 10);
    let info = format_pagination_info(&result);

    assert!(info.contains("11-20"));
    assert!(info.contains("page 2/3"));
}

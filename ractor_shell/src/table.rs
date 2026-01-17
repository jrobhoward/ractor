//! Table formatting utilities with terminal-aware output
//!
//! Provides helpers for creating tables that adapt to terminal width
//! and support pagination for large result sets.

use tabled::settings::object::Columns;
use tabled::settings::{Modify, Width};
use tabled::{Table, Tabled};
use terminal_size::{terminal_size, Width as TermWidth};

/// Default terminal width when detection fails
const DEFAULT_TERMINAL_WIDTH: usize = 120;

/// Default page size for pagination
pub const DEFAULT_PAGE_SIZE: usize = 20;

/// Get the terminal width, falling back to a default value
pub fn get_terminal_width() -> usize {
    terminal_size()
        .map(|(TermWidth(w), _)| w as usize)
        .unwrap_or(DEFAULT_TERMINAL_WIDTH)
}

/// Build a table with terminal-aware width settings
///
/// This creates a table that fits within the terminal width by
/// wrapping long content in columns.
pub fn build_table<T: Tabled>(rows: Vec<T>) -> String {
    if rows.is_empty() {
        return String::new();
    }

    let term_width = get_terminal_width();

    // Build the table
    let mut table = Table::new(rows);

    // Set max width to fit terminal
    table.with(Width::wrap(term_width).priority::<tabled::settings::peaker::PriorityMax>());

    table.to_string()
}

/// Build a table with specific column truncation
///
/// Useful when certain columns should be truncated while others wrap.
pub fn build_table_truncated<T: Tabled>(rows: Vec<T>, truncate_columns: &[usize]) -> String {
    if rows.is_empty() {
        return String::new();
    }

    let term_width = get_terminal_width();
    let max_col_width = term_width / 3; // Max 1/3 of terminal for any column

    let mut table = Table::new(rows);

    // Truncate specified columns
    for &col in truncate_columns {
        table.with(
            Modify::new(Columns::single(col)).with(Width::truncate(max_col_width).suffix("...")),
        );
    }

    // Wrap remaining content to fit terminal
    table.with(Width::wrap(term_width));

    table.to_string()
}

/// A paginated result for displaying large data sets
pub struct PaginatedResult<T> {
    /// Current page items
    pub items: Vec<T>,
    /// Current page number (0-indexed)
    pub page: usize,
    /// Total number of pages
    pub total_pages: usize,
    /// Total number of items
    pub total_items: usize,
    /// Items per page
    pub page_size: usize,
}

impl<T> PaginatedResult<T> {
    /// Create a paginated result from a slice
    pub fn from_slice(items: &[T], page: usize, page_size: usize) -> PaginatedResult<&T> {
        let total_items = items.len();
        let total_pages = total_items.div_ceil(page_size);
        let start = page * page_size;
        let end = std::cmp::min(start + page_size, total_items);

        let page_items: Vec<&T> = if start < total_items {
            items[start..end].iter().collect()
        } else {
            Vec::new()
        };

        PaginatedResult {
            items: page_items,
            page,
            total_pages,
            total_items,
            page_size,
        }
    }

    /// Check if there's a next page
    pub fn has_next(&self) -> bool {
        self.page + 1 < self.total_pages
    }

    /// Check if there's a previous page
    pub fn has_prev(&self) -> bool {
        self.page > 0
    }
}

/// Sort direction for tables
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

/// Format pagination info for display
pub fn format_pagination_info<T>(result: &PaginatedResult<T>) -> String {
    if result.total_pages <= 1 {
        format!("Showing all {} items", result.total_items)
    } else {
        let start = result.page * result.page_size + 1;
        let end = std::cmp::min((result.page + 1) * result.page_size, result.total_items);
        format!(
            "Showing {}-{} of {} items (page {}/{})",
            start,
            end,
            result.total_items,
            result.page + 1,
            result.total_pages
        )
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod table_tests;

//! TUI module for the `top` command - actor dashboard visualization.
//!
//! This module provides a terminal user interface for real-time actor monitoring,
//! inspired by Erlang's observer_cli and tokio-console.
//!
//! # Architecture
//!
//! - [`App`]: Main application state and event loop
//! - [`metrics`]: Actor metrics collection via tracing
//! - [`ui`]: UI rendering with ratatui
//!
//! # Usage
//!
//! ```rust,ignore
//! use ractor_shell::tui::App;
//!
//! // Run the TUI (blocks until user exits)
//! App::run().await?;
//! ```

mod app;
mod metrics;
mod ui;

pub use app::App;
pub use metrics::{ActorMetrics, ActorMetricsCollector};

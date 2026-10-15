//! An asynchronous terminal client for Music Player Daemon.

pub mod app;
pub mod browser;
pub mod config;
pub mod connection;
pub mod event_handler;
pub mod list;
pub mod tui;
pub mod ui;
pub mod utils;

/// The application's error type.
pub type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

//! rmptui — a keyboard-driven MPD client for the terminal.
//!
//! The crate is split into an MPD client (`mpd`), configuration (`config`),
//! the application loop (`app`) and its UI components (`screens`, `popups`,
//! `ui`), plus the lyrics and visualizer services.

pub mod action;
pub mod app;
pub mod config;
pub mod event;
pub mod logging;
pub mod lyrics;
pub mod mpd;
pub mod popups;
pub mod screens;
pub mod state;
pub mod tags;
pub mod ui;
pub mod util;
pub mod visualizer;

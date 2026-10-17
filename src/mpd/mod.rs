//! MPD client: protocol, connection, typed models and background workers.

pub mod commands;
mod connection;
mod error;
pub mod protocol;
mod selection;
mod types;
mod worker;

pub use connection::{Address, Connection};
pub use error::{Ack, MpdError, MpdResult};
pub use protocol::Command;
pub use selection::Selection;
pub use types::{Album, DirEntry, OnOff, PlayState, PlaylistInfo, Song, Status, Subsystem};
pub use worker::{MpdEvent, MpdHandle, Query, Reply, Request, spawn};

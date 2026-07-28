//! Graxus index — JSON storage, SQLite storage, and snapshot management.
//!
//! This crate provides three core capabilities for the graxus toolchain:
//!
//! - **[`IndexStore`]**: file-system based JSON persistence with snapshot/rollback.
//! - **[`SqliteStore`]**: SQLite-backed indexed storage for code metadata.
//! - **Snapshots**: point-in-time backups of project files.

pub mod schema;
pub mod snapshots;
pub mod sqlite;
pub mod store;

pub use snapshots::{Snapshot, SnapshotFile, SnapshotMeta};
pub use sqlite::{
    CallRecord, FileRecord, ImportRecord, ParserFactRecord, ParserResultRecord, SqliteStore,
    SymbolRecord,
};
pub use store::IndexStore;

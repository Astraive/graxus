//! SQLite storage backend for Graxus index.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::Path;

pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    pub fn new(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open SQLite at {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let store = Self { conn };
        store.create_tables()?;
        Ok(store)
    }

    fn create_tables(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                language TEXT NOT NULL,
                hash TEXT NOT NULL,
                size INTEGER NOT NULL,
                modified_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS symbols (
                id TEXT PRIMARY KEY,
                file TEXT NOT NULL,
                language TEXT NOT NULL,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                exported BOOLEAN NOT NULL,
                line_start INTEGER NOT NULL,
                line_end INTEGER NOT NULL,
                visibility TEXT NOT NULL,
                signature TEXT DEFAULT '',
                is_test BOOLEAN DEFAULT FALSE,
                usage_count INTEGER DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
            CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file);
            CREATE TABLE IF NOT EXISTS imports (
                id TEXT PRIMARY KEY,
                file TEXT NOT NULL,
                language TEXT NOT NULL,
                kind TEXT NOT NULL,
                source TEXT NOT NULL,
                local_name TEXT,
                imported_name TEXT,
                resolved_file TEXT,
                line INTEGER NOT NULL,
                confidence TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_imports_file ON imports(file);
            CREATE TABLE IF NOT EXISTS calls (
                id TEXT PRIMARY KEY,
                file TEXT NOT NULL,
                language TEXT NOT NULL,
                kind TEXT NOT NULL,
                caller_symbol TEXT,
                callee_text TEXT NOT NULL,
                object TEXT,
                resolved_symbol TEXT,
                line INTEGER NOT NULL,
                column INTEGER NOT NULL,
                confidence TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_calls_file ON calls(file);
            CREATE TABLE IF NOT EXISTS doc_nodes (
                id TEXT PRIMARY KEY,
                node_type TEXT NOT NULL,
                path TEXT NOT NULL,
                title TEXT NOT NULL,
                tags TEXT,
                headings TEXT
            );
            CREATE TABLE IF NOT EXISTS doc_edges (
                from_id TEXT NOT NULL,
                to_id TEXT NOT NULL,
                edge_type TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS bridge_edges (
                from_id TEXT NOT NULL,
                to_id TEXT NOT NULL,
                edge_type TEXT NOT NULL,
                confidence TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

    pub fn insert_symbol(
        &self, id: &str, file: &str, language: &str, kind: &str, name: &str,
        exported: bool, line_start: usize, line_end: usize, visibility: &str,
        signature: &str, is_test: bool, usage_count: usize,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO symbols (id, file, language, kind, name, exported, line_start, line_end, visibility, signature, is_test, usage_count) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![id, file, language, kind, name, exported, line_start as i64, line_end as i64, visibility, signature, is_test, usage_count as i64],
        )?;
        Ok(())
    }

    pub fn insert_import(
        &self, id: &str, file: &str, language: &str, kind: &str, source: &str,
        local_name: Option<&str>, imported_name: Option<&str>, resolved_file: Option<&str>,
        line: usize, confidence: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO imports (id, file, language, kind, source, local_name, imported_name, resolved_file, line, confidence) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![id, file, language, kind, source, local_name, imported_name, resolved_file, line as i64, confidence],
        )?;
        Ok(())
    }

    pub fn insert_call(
        &self, id: &str, file: &str, language: &str, kind: &str,
        caller_symbol: Option<&str>, callee_text: &str, object: Option<&str>,
        resolved_symbol: Option<&str>, line: usize, column: usize, confidence: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO calls (id, file, language, kind, caller_symbol, callee_text, object, resolved_symbol, line, column, confidence) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![id, file, language, kind, caller_symbol, callee_text, object, resolved_symbol, line as i64, column as i64, confidence],
        )?;
        Ok(())
    }

    pub fn insert_doc_node(&self, id: &str, node_type: &str, path: &str, title: &str, tags: &str, headings: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO doc_nodes (id, node_type, path, title, tags, headings) VALUES (?1,?2,?3,?4,?5,?6)",
            params![id, node_type, path, title, tags, headings],
        )?;
        Ok(())
    }

    pub fn insert_doc_edge(&self, from_id: &str, to_id: &str, edge_type: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO doc_edges (from_id, to_id, edge_type) VALUES (?1,?2,?3)",
            params![from_id, to_id, edge_type],
        )?;
        Ok(())
    }

    pub fn insert_bridge_edge(&self, from_id: &str, to_id: &str, edge_type: &str, confidence: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO bridge_edges (from_id, to_id, edge_type, confidence) VALUES (?1,?2,?3,?4)",
            params![from_id, to_id, edge_type, confidence],
        )?;
        Ok(())
    }

    pub fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_metadata(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM metadata WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get(0))?;
        match rows.next() {
            Some(Ok(val)) => Ok(Some(val)),
            _ => Ok(None),
        }
    }

    pub fn symbol_count(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    pub fn import_count(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM imports", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    pub fn call_count(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM calls", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    pub fn file_count(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
        Ok(count as usize)
    }
}

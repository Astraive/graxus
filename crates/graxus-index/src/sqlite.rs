//! SQLite storage backend for Graxus index.
//!
//! Provides a persistent, indexed store for code metadata (files, symbols,
//! imports, call relationships, and semantic facts) backed by an SQLite
//! database with WAL journaling and foreign-key enforcement enabled.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

use crate::schema;

/// A row from the `files` table.
#[derive(Debug, Clone)]
pub struct FileRecord {
    pub path: String,
    pub language: String,
    pub hash: String,
    pub size: i64,
    pub modified_at: String,
}

/// A row from the `symbols` table.
#[derive(Debug, Clone)]
pub struct SymbolRecord {
    pub id: String,
    pub file: String,
    pub language: String,
    pub kind: String,
    pub name: String,
    pub exported: bool,
    pub line_start: usize,
    pub line_end: usize,
    pub visibility: String,
    pub signature: String,
    pub is_test: bool,
    pub usage_count: usize,
}

/// A row from the `imports` table.
#[derive(Debug, Clone)]
pub struct ImportRecord {
    pub id: String,
    pub file: String,
    pub language: String,
    pub kind: String,
    pub source: String,
    pub local_name: Option<String>,
    pub imported_name: Option<String>,
    pub resolved_file: Option<String>,
    pub line: usize,
    pub confidence: String,
}

/// A row from the `calls` table.
#[derive(Debug, Clone)]
pub struct CallRecord {
    pub id: String,
    pub file: String,
    pub language: String,
    pub kind: String,
    pub caller_symbol: Option<String>,
    pub callee_text: String,
    pub object: Option<String>,
    pub resolved_symbol: Option<String>,
    pub line: usize,
    pub column: usize,
    pub confidence: String,
}

/// Parser backend provenance persisted for one file.
#[derive(Debug, Clone)]
pub struct ParserResultRecord {
    pub file: String,
    pub requested_backend: String,
    pub used_backend: String,
    pub fallback_reason: Option<String>,
    pub diagnostics_json: String,
}

/// Lossless parser-native fact persisted alongside normalized facts.
#[derive(Debug, Clone)]
pub struct ParserFactRecord {
    pub id: String,
    pub file: String,
    pub kind: String,
    pub data_json: String,
}

/// A persisted HTTP/API route fact.
///
/// `middleware` is decoded from the JSON array stored in SQLite. It defaults
/// to an empty list when deserializing older serialized records that omit it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RouteRecord {
    pub id: String,
    pub file: String,
    pub language: String,
    pub method: String,
    pub path: String,
    pub handler: String,
    pub handler_file: Option<String>,
    pub line: usize,
    pub framework: String,
    #[serde(default)]
    pub middleware: Vec<String>,
}

/// A persisted trait, interface, inheritance, or extension fact.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeImplRecord {
    pub id: String,
    pub file: String,
    pub language: String,
    pub implementing_type: String,
    pub trait_or_interface: String,
    pub line: usize,
    /// Canonical snake_case relationship kind from the codemap fact.
    pub kind: String,
}

/// A persisted dependency-injection binding fact.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DIBindingRecord {
    pub id: String,
    pub file: String,
    pub language: String,
    pub abstract_type: String,
    pub concrete_type: String,
    pub lifetime: Option<String>,
    pub line: usize,
    pub framework: String,
}

/// SQLite-backed index store.
///
/// Opens (or creates) an SQLite database at the given path with WAL journal mode
/// and foreign keys enabled. All tables are created automatically on open.
pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    /// Open or create an SQLite database at `path`.
    ///
    /// Enables WAL journal mode and foreign-key enforcement, then creates all
    /// required tables if they do not already exist.
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

    /// Create all tables and indexes required by the index store.
    fn create_tables(&self) -> Result<()> {
        self.conn.execute_batch(&schema::all_statements())?;
        Ok(())
    }

    /// Insert or replace a file record.
    ///
    /// # Cast Safety
    /// `size` is cast from `usize` to `i64`. On 64-bit platforms `usize` can
    /// exceed `i64::MAX`, but file sizes never approach 8 exabytes, so this
    /// is safe in practice.
    pub fn insert_file(
        &self,
        path: &str,
        language: &str,
        hash: &str,
        size: usize,
        modified_at: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO files (path, language, hash, size, modified_at) VALUES (?1,?2,?3,?4,?5)",
            params![path, language, hash, size as i64, modified_at],
        )?;
        Ok(())
    }

    /// Insert or replace a symbol record.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_symbol(
        &self,
        id: &str,
        file: &str,
        language: &str,
        kind: &str,
        name: &str,
        exported: bool,
        line_start: usize,
        line_end: usize,
        visibility: &str,
        signature: &str,
        is_test: bool,
        usage_count: usize,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO symbols (id, file, language, kind, name, exported, line_start, line_end, visibility, signature, is_test, usage_count) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![id, file, language, kind, name, exported, line_start as i64, line_end as i64, visibility, signature, is_test, usage_count as i64],
        )?;
        Ok(())
    }

    /// Insert or replace an import record.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_import(
        &self,
        id: &str,
        file: &str,
        language: &str,
        kind: &str,
        source: &str,
        local_name: Option<&str>,
        imported_name: Option<&str>,
        resolved_file: Option<&str>,
        line: usize,
        confidence: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO imports (id, file, language, kind, source, local_name, imported_name, resolved_file, line, confidence) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![id, file, language, kind, source, local_name, imported_name, resolved_file, line as i64, confidence],
        )?;
        Ok(())
    }

    /// Insert or replace a call record.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_call(
        &self,
        id: &str,
        file: &str,
        language: &str,
        kind: &str,
        caller_symbol: Option<&str>,
        callee_text: &str,
        object: Option<&str>,
        resolved_symbol: Option<&str>,
        line: usize,
        column: usize,
        confidence: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO calls (id, file, language, kind, caller_symbol, callee_text, object, resolved_symbol, line, column, confidence) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![id, file, language, kind, caller_symbol, callee_text, object, resolved_symbol, line as i64, column as i64, confidence],
        )?;
        Ok(())
    }

    /// Insert or update one HTTP/API route fact.
    ///
    /// Middleware is serialized with `serde_json`, so arbitrary middleware
    /// names are stored as a valid JSON array rather than interpolated SQL.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_route(
        &self,
        id: &str,
        file: &str,
        language: &str,
        method: &str,
        path: &str,
        handler: &str,
        handler_file: Option<&str>,
        line: usize,
        framework: &str,
        middleware: &[String],
    ) -> Result<()> {
        let middleware_json =
            serde_json::to_string(middleware).context("failed to serialize route middleware")?;
        let line = i64::try_from(line).context("route line exceeds SQLite INTEGER range")?;
        self.conn.execute(
            "INSERT INTO routes (id, file, language, method, path, handler, handler_file, line, framework, middleware) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) ON CONFLICT(id) DO UPDATE SET file = excluded.file, language = excluded.language, method = excluded.method, path = excluded.path, handler = excluded.handler, handler_file = excluded.handler_file, line = excluded.line, framework = excluded.framework, middleware = excluded.middleware",
            params![id, file, language, method, path, handler, handler_file, line, framework, middleware_json],
        )?;
        Ok(())
    }

    /// Insert or update one trait, interface, inheritance, or extension fact.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_type_impl(
        &self,
        id: &str,
        file: &str,
        language: &str,
        implementing_type: &str,
        trait_or_interface: &str,
        line: usize,
        kind: &str,
    ) -> Result<()> {
        let line =
            i64::try_from(line).context("type implementation line exceeds SQLite INTEGER range")?;
        self.conn.execute(
            "INSERT INTO type_impls (id, file, language, implementing_type, trait_or_interface, line, kind) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT(id) DO UPDATE SET file = excluded.file, language = excluded.language, implementing_type = excluded.implementing_type, trait_or_interface = excluded.trait_or_interface, line = excluded.line, kind = excluded.kind",
            params![id, file, language, implementing_type, trait_or_interface, line, kind],
        )?;
        Ok(())
    }

    /// Insert or update one dependency-injection binding fact.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_di_binding(
        &self,
        id: &str,
        file: &str,
        language: &str,
        abstract_type: &str,
        concrete_type: &str,
        lifetime: Option<&str>,
        line: usize,
        framework: &str,
    ) -> Result<()> {
        let line = i64::try_from(line).context("DI binding line exceeds SQLite INTEGER range")?;
        self.conn.execute(
            "INSERT INTO di_bindings (id, file, language, abstract_type, concrete_type, lifetime, line, framework) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT(id) DO UPDATE SET file = excluded.file, language = excluded.language, abstract_type = excluded.abstract_type, concrete_type = excluded.concrete_type, lifetime = excluded.lifetime, line = excluded.line, framework = excluded.framework",
            params![id, file, language, abstract_type, concrete_type, lifetime, line, framework],
        )?;
        Ok(())
    }

    /// Insert or replace parser backend provenance for a file.
    pub fn insert_parser_result(
        &self,
        file: &str,
        requested_backend: &str,
        used_backend: &str,
        fallback_reason: Option<&str>,
        diagnostics_json: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO parser_results (file, requested_backend, used_backend, fallback_reason, diagnostics_json) VALUES (?1,?2,?3,?4,?5)",
            params![
                file,
                requested_backend,
                used_backend,
                fallback_reason,
                diagnostics_json
            ],
        )?;
        Ok(())
    }

    /// Insert or replace a lossless parser-native fact.
    pub fn insert_parser_fact(
        &self,
        id: &str,
        file: &str,
        kind: &str,
        data_json: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO parser_facts (id, file, kind, data_json) VALUES (?1,?2,?3,?4)",
            params![id, file, kind, data_json],
        )?;
        Ok(())
    }

    /// Persist one serialized codemap `parser_results` entry and all of its facts.
    pub fn insert_parser_result_value(&self, result: &serde_json::Value) -> Result<()> {
        let file = result
            .get("file")
            .and_then(|value| value.as_str())
            .context("parser result is missing file")?;
        let requested_backend = result
            .get("requested_backend")
            .and_then(|value| value.as_str())
            .context("parser result is missing requested_backend")?;
        let used_backend = result
            .get("used_backend")
            .and_then(|value| value.as_str())
            .context("parser result is missing used_backend")?;
        let fallback_reason = result
            .get("fallback_reason")
            .and_then(|value| value.as_str());
        let diagnostics = result
            .get("diagnostics")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
        let diagnostics_json = serde_json::to_string(&diagnostics)?;
        self.insert_parser_result(
            file,
            requested_backend,
            used_backend,
            fallback_reason,
            &diagnostics_json,
        )?;

        if let Some(facts) = result.get("facts").and_then(|value| value.as_array()) {
            for fact in facts {
                let id = fact
                    .get("id")
                    .and_then(|value| value.as_str())
                    .context("parser fact is missing id")?;
                let kind = fact
                    .get("kind")
                    .and_then(|value| value.as_str())
                    .context("parser fact is missing kind")?;
                let data = fact.get("data").cloned().unwrap_or(serde_json::Value::Null);
                let data_json = serde_json::to_string(&data)?;
                self.insert_parser_fact(id, file, kind, &data_json)?;
            }
        }
        Ok(())
    }

    /// Insert or replace a documentation node record.
    pub fn insert_doc_node(
        &self,
        id: &str,
        node_type: &str,
        path: &str,
        title: &str,
        tags: &str,
        headings: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO doc_nodes (id, node_type, path, title, tags, headings) VALUES (?1,?2,?3,?4,?5,?6)",
            params![id, node_type, path, title, tags, headings],
        )?;
        Ok(())
    }

    /// Insert a documentation edge (relationship between two doc nodes).
    pub fn insert_doc_edge(&self, from_id: &str, to_id: &str, edge_type: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO doc_edges (from_id, to_id, edge_type) VALUES (?1,?2,?3)",
            params![from_id, to_id, edge_type],
        )?;
        Ok(())
    }

    /// Insert a bridge edge (cross-graph relationship with confidence).
    pub fn insert_bridge_edge(
        &self,
        from_id: &str,
        to_id: &str,
        edge_type: &str,
        confidence: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO bridge_edges (from_id, to_id, edge_type, confidence) VALUES (?1,?2,?3,?4)",
            params![from_id, to_id, edge_type, confidence],
        )?;
        Ok(())
    }

    /// Set a metadata key-value pair (upsert).
    pub fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Get a metadata value by key, returning `None` if not found.
    pub fn get_metadata(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM metadata WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get(0))?;
        match rows.next() {
            Some(Ok(val)) => Ok(Some(val)),
            _ => Ok(None),
        }
    }

    /// Return the number of symbols in the database.
    pub fn symbol_count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM symbols", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    /// Return the number of imports in the database.
    pub fn import_count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM imports", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    /// Return the number of call records in the database.
    pub fn call_count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM calls", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    /// Return the number of files in the database.
    pub fn file_count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0))?;
        Ok(count as usize)
    }

    /// Alias for [`file_count`](Self::file_count).
    pub fn get_file_count(&self) -> Result<usize> {
        self.file_count()
    }

    /// Alias for [`symbol_count`](Self::symbol_count).
    pub fn get_symbol_count(&self) -> Result<usize> {
        self.symbol_count()
    }

    // ── Delete methods ─────────────────────────────────────────────

    /// Delete all indexed code facts and parser data for a full rebuild.
    ///
    /// Documentation and metadata tables are intentionally preserved.
    pub fn clear_code_data(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM parser_facts;
             DELETE FROM parser_results;
             DELETE FROM symbols;
             DELETE FROM imports;
             DELETE FROM calls;
             DELETE FROM routes;
             DELETE FROM type_impls;
             DELETE FROM di_bindings;",
        )?;
        Ok(())
    }

    /// Delete all indexed code, semantic facts, and parser data for a file path.
    pub fn delete_file_data(&self, path: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM symbols WHERE file = ?1", params![path])?;
        self.conn
            .execute("DELETE FROM imports WHERE file = ?1", params![path])?;
        self.conn
            .execute("DELETE FROM calls WHERE file = ?1", params![path])?;
        self.conn
            .execute("DELETE FROM routes WHERE file = ?1", params![path])?;
        self.conn
            .execute("DELETE FROM type_impls WHERE file = ?1", params![path])?;
        self.conn
            .execute("DELETE FROM di_bindings WHERE file = ?1", params![path])?;
        self.conn
            .execute("DELETE FROM parser_facts WHERE file = ?1", params![path])?;
        self.conn
            .execute("DELETE FROM parser_results WHERE file = ?1", params![path])?;
        Ok(())
    }

    // ── Query methods ─────────────────────────────────────────────

    /// Return all file records.
    pub fn get_files(&self) -> Result<Vec<FileRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, language, hash, size, modified_at FROM files ORDER BY path")?;
        let rows = stmt.query_map([], |row| {
            Ok(FileRecord {
                path: row.get(0)?,
                language: row.get(1)?,
                hash: row.get(2)?,
                size: row.get(3)?,
                modified_at: row.get(4)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Return all symbols belonging to a specific file.
    pub fn get_symbols_by_file(&self, file: &str) -> Result<Vec<SymbolRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file, language, kind, name, exported, line_start, line_end, visibility, signature, is_test, usage_count FROM symbols WHERE file = ?1 ORDER BY line_start",
        )?;
        let rows = stmt.query_map(params![file], |row| {
            Ok(SymbolRecord {
                id: row.get(0)?,
                file: row.get(1)?,
                language: row.get(2)?,
                kind: row.get(3)?,
                name: row.get(4)?,
                exported: row.get(5)?,
                line_start: row.get::<_, i64>(6)? as usize,
                line_end: row.get::<_, i64>(7)? as usize,
                visibility: row.get(8)?,
                signature: row.get(9)?,
                is_test: row.get(10)?,
                usage_count: row.get::<_, i64>(11)? as usize,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Return all symbols with the given exact name.
    pub fn get_symbols_by_name(&self, name: &str) -> Result<Vec<SymbolRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file, language, kind, name, exported, line_start, line_end, visibility, signature, is_test, usage_count FROM symbols WHERE name = ?1 ORDER BY file, line_start",
        )?;
        let rows = stmt.query_map(params![name], |row| {
            Ok(SymbolRecord {
                id: row.get(0)?,
                file: row.get(1)?,
                language: row.get(2)?,
                kind: row.get(3)?,
                name: row.get(4)?,
                exported: row.get(5)?,
                line_start: row.get::<_, i64>(6)? as usize,
                line_end: row.get::<_, i64>(7)? as usize,
                visibility: row.get(8)?,
                signature: row.get(9)?,
                is_test: row.get(10)?,
                usage_count: row.get::<_, i64>(11)? as usize,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Search symbols by name using a SQL LIKE pattern (e.g. `%query%`).
    ///
    /// User input wildcards (`%` and `_`) are escaped to prevent pattern injection.
    pub fn search_symbols(&self, query: &str) -> Result<Vec<SymbolRecord>> {
        let escaped = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{}%", escaped);
        let mut stmt = self.conn.prepare(
            "SELECT id, file, language, kind, name, exported, line_start, line_end, visibility, signature, is_test, usage_count FROM symbols WHERE name LIKE ?1 ESCAPE '\\' ORDER BY name, file",
        )?;
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(SymbolRecord {
                id: row.get(0)?,
                file: row.get(1)?,
                language: row.get(2)?,
                kind: row.get(3)?,
                name: row.get(4)?,
                exported: row.get(5)?,
                line_start: row.get::<_, i64>(6)? as usize,
                line_end: row.get::<_, i64>(7)? as usize,
                visibility: row.get(8)?,
                signature: row.get(9)?,
                is_test: row.get(10)?,
                usage_count: row.get::<_, i64>(11)? as usize,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Return all imports belonging to a specific file.
    pub fn get_imports_by_file(&self, file: &str) -> Result<Vec<ImportRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file, language, kind, source, local_name, imported_name, resolved_file, line, confidence FROM imports WHERE file = ?1 ORDER BY line",
        )?;
        let rows = stmt.query_map(params![file], |row| {
            Ok(ImportRecord {
                id: row.get(0)?,
                file: row.get(1)?,
                language: row.get(2)?,
                kind: row.get(3)?,
                source: row.get(4)?,
                local_name: row.get(5)?,
                imported_name: row.get(6)?,
                resolved_file: row.get(7)?,
                line: row.get::<_, i64>(8)? as usize,
                confidence: row.get(9)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Return all call records whose callee text matches the given symbol name.
    pub fn get_calls_to(&self, symbol: &str) -> Result<Vec<CallRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file, language, kind, caller_symbol, callee_text, object, resolved_symbol, line, column, confidence FROM calls WHERE callee_text = ?1 ORDER BY file, line",
        )?;
        let rows = stmt.query_map(params![symbol], |row| {
            Ok(CallRecord {
                id: row.get(0)?,
                file: row.get(1)?,
                language: row.get(2)?,
                kind: row.get(3)?,
                caller_symbol: row.get(4)?,
                callee_text: row.get(5)?,
                object: row.get(6)?,
                resolved_symbol: row.get(7)?,
                line: row.get::<_, i64>(8)? as usize,
                column: row.get::<_, i64>(9)? as usize,
                confidence: row.get(10)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Return all route facts registered for an exact route path.
    ///
    /// Results are ordered by registration file, line, and identifier.
    pub fn get_routes_by_path(&self, path: &str) -> Result<Vec<RouteRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file, language, method, path, handler, handler_file, line, framework, middleware FROM routes WHERE path = ?1 ORDER BY file, line, id",
        )?;
        let rows = stmt.query_map(params![path], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Option<String>>(9)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (
                id,
                file,
                language,
                method,
                path,
                handler,
                handler_file,
                line,
                framework,
                middleware_json,
            ) = row?;
            let middleware_json = middleware_json.unwrap_or_else(|| "[]".to_owned());
            let middleware: Vec<String> = serde_json::from_str(&middleware_json)
                .with_context(|| format!("failed to decode route middleware for {id}"))?;
            result.push(RouteRecord {
                id,
                file,
                language,
                method,
                path,
                handler,
                handler_file,
                line: usize::try_from(line).context("route line is negative")?,
                framework,
                middleware,
            });
        }
        Ok(result)
    }

    /// Return all type implementation facts for an exact trait or interface.
    ///
    /// Results are ordered by implementation file, line, and identifier.
    pub fn get_type_impls_by_trait(&self, trait_or_interface: &str) -> Result<Vec<TypeImplRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file, language, implementing_type, trait_or_interface, line, kind FROM type_impls WHERE trait_or_interface = ?1 ORDER BY file, line, id",
        )?;
        let rows = stmt.query_map(params![trait_or_interface], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (id, file, language, implementing_type, trait_or_interface, line, kind) = row?;
            result.push(TypeImplRecord {
                id,
                file,
                language,
                implementing_type,
                trait_or_interface,
                line: usize::try_from(line).context("type implementation line is negative")?,
                kind,
            });
        }
        Ok(result)
    }

    /// Return all DI bindings for an exact abstract type or interface.
    ///
    /// Results are ordered by registration file, line, and identifier.
    pub fn get_di_bindings_by_abstract_type(
        &self,
        abstract_type: &str,
    ) -> Result<Vec<DIBindingRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file, language, abstract_type, concrete_type, lifetime, line, framework FROM di_bindings WHERE abstract_type = ?1 ORDER BY file, line, id",
        )?;
        let rows = stmt.query_map(params![abstract_type], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (id, file, language, abstract_type, concrete_type, lifetime, line, framework) =
                row?;
            result.push(DIBindingRecord {
                id,
                file,
                language,
                abstract_type,
                concrete_type,
                lifetime,
                line: usize::try_from(line).context("DI binding line is negative")?,
                framework,
            });
        }
        Ok(result)
    }

    /// Return parser backend provenance for a file.
    pub fn get_parser_result(&self, file: &str) -> Result<Option<ParserResultRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT file, requested_backend, used_backend, fallback_reason, diagnostics_json FROM parser_results WHERE file = ?1",
        )?;
        let mut rows = stmt.query(params![file])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(ParserResultRecord {
            file: row.get(0)?,
            requested_backend: row.get(1)?,
            used_backend: row.get(2)?,
            fallback_reason: row.get(3)?,
            diagnostics_json: row.get(4)?,
        }))
    }

    /// Return all lossless parser-native facts for a file.
    pub fn get_parser_facts_by_file(&self, file: &str) -> Result<Vec<ParserFactRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file, kind, data_json FROM parser_facts WHERE file = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![file], |row| {
            Ok(ParserFactRecord {
                id: row.get(0)?,
                file: row.get(1)?,
                kind: row.get(2)?,
                data_json: row.get(3)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_db() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("graxus-index-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("test.db")
    }

    #[test]
    fn test_create_store() {
        let path = temp_db();
        let store = SqliteStore::new(&path).unwrap();
        assert_eq!(store.file_count().unwrap(), 0);
        assert_eq!(store.symbol_count().unwrap(), 0);
    }

    #[test]
    fn test_wal_mode_is_set() {
        let path = temp_db();
        let store = SqliteStore::new(&path).unwrap();
        let mode: String = store
            .conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let path = temp_db();
        let store = SqliteStore::new(&path).unwrap();
        let fk: i64 = store
            .conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn test_insert_and_get_files() {
        let path = temp_db();
        let store = SqliteStore::new(&path).unwrap();

        store
            .insert_file(
                "src/main.rs",
                "rust",
                "abc123",
                1024,
                "2026-01-01T00:00:00Z",
            )
            .unwrap();
        store
            .insert_file("src/lib.rs", "rust", "def456", 2048, "2026-01-02T00:00:00Z")
            .unwrap();

        assert_eq!(store.file_count().unwrap(), 2);
        assert_eq!(store.get_file_count().unwrap(), 2);

        let files = store.get_files().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "src/lib.rs"); // alphabetical
        assert_eq!(files[1].path, "src/main.rs");
        assert_eq!(files[0].language, "rust");
        assert_eq!(files[0].size, 2048);
    }

    #[test]
    fn test_insert_and_get_symbols() {
        let path = temp_db();
        let store = SqliteStore::new(&path).unwrap();

        store
            .insert_symbol(
                "s1",
                "main.rs",
                "rust",
                "function",
                "main",
                true,
                1,
                5,
                "pub",
                "fn main()",
                false,
                10,
            )
            .unwrap();
        store
            .insert_symbol(
                "s2",
                "main.rs",
                "rust",
                "function",
                "helper",
                false,
                7,
                10,
                "fn",
                "fn helper()",
                false,
                3,
            )
            .unwrap();
        store
            .insert_symbol(
                "s3",
                "lib.rs",
                "rust",
                "struct",
                "Config",
                true,
                1,
                20,
                "pub",
                "struct Config",
                false,
                50,
            )
            .unwrap();

        assert_eq!(store.symbol_count().unwrap(), 3);
        assert_eq!(store.get_symbol_count().unwrap(), 3);

        // By file
        let main_syms = store.get_symbols_by_file("main.rs").unwrap();
        assert_eq!(main_syms.len(), 2);
        assert_eq!(main_syms[0].name, "main");
        assert_eq!(main_syms[1].name, "helper");

        // By name
        let config_syms = store.get_symbols_by_name("Config").unwrap();
        assert_eq!(config_syms.len(), 1);
        assert_eq!(config_syms[0].file, "lib.rs");

        // Search
        let search_results = store.search_symbols("ain").unwrap();
        assert_eq!(search_results.len(), 1); // "main" matches, "helper" does not, "Config" does not
        assert_eq!(search_results[0].name, "main");
    }

    #[test]
    fn test_insert_and_get_imports() {
        let path = temp_db();
        let store = SqliteStore::new(&path).unwrap();

        store
            .insert_import(
                "i1",
                "main.rs",
                "rust",
                "use",
                "std::io",
                None,
                Some("BufReader"),
                None,
                1,
                "high",
            )
            .unwrap();
        store
            .insert_import(
                "i2",
                "main.rs",
                "rust",
                "use",
                "serde",
                Some("serde"),
                Some("Deserialize"),
                None,
                2,
                "high",
            )
            .unwrap();

        assert_eq!(store.import_count().unwrap(), 2);

        let imports = store.get_imports_by_file("main.rs").unwrap();
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].source, "std::io");
        assert_eq!(imports[1].source, "serde");
    }

    #[test]
    fn test_insert_and_get_calls() {
        let path = temp_db();
        let store = SqliteStore::new(&path).unwrap();

        store
            .insert_call(
                "c1",
                "main.rs",
                "rust",
                "function",
                Some("main"),
                "println!",
                None,
                None,
                3,
                4,
                "high",
            )
            .unwrap();
        store
            .insert_call(
                "c2",
                "main.rs",
                "rust",
                "function",
                Some("main"),
                "helper",
                None,
                Some("s2"),
                4,
                4,
                "high",
            )
            .unwrap();
        store
            .insert_call(
                "c3",
                "lib.rs",
                "rust",
                "method",
                Some("Config"),
                "helper",
                Some("self"),
                Some("s2"),
                15,
                8,
                "medium",
            )
            .unwrap();

        assert_eq!(store.call_count().unwrap(), 3);

        let calls = store.get_calls_to("helper").unwrap();
        assert_eq!(calls.len(), 2);
        // Results ordered by file: lib.rs before main.rs
        assert_eq!(calls[0].file, "lib.rs");
        assert_eq!(calls[1].file, "main.rs");
    }

    #[test]
    fn test_metadata_round_trip() {
        let path = temp_db();
        let store = SqliteStore::new(&path).unwrap();

        store.set_metadata("version", "0.4.0").unwrap();
        store.set_metadata("project", "graxus").unwrap();

        assert_eq!(store.get_metadata("version").unwrap(), Some("0.4.0".into()));
        assert_eq!(
            store.get_metadata("project").unwrap(),
            Some("graxus".into())
        );
        assert_eq!(store.get_metadata("missing").unwrap(), None);

        // Upsert
        store.set_metadata("version", "0.5.0").unwrap();
        assert_eq!(store.get_metadata("version").unwrap(), Some("0.5.0".into()));
    }

    #[test]
    fn test_insert_file_upsert() {
        let path = temp_db();
        let store = SqliteStore::new(&path).unwrap();

        store
            .insert_file("a.rs", "rust", "h1", 100, "2026-01-01")
            .unwrap();
        store
            .insert_file("a.rs", "rust", "h2", 200, "2026-01-02")
            .unwrap();

        assert_eq!(store.file_count().unwrap(), 1);
        let files = store.get_files().unwrap();
        assert_eq!(files[0].hash, "h2");
        assert_eq!(files[0].size, 200);
    }

    #[test]
    fn test_parser_results_and_native_facts_round_trip() {
        let path = temp_db();
        let store = SqliteStore::new(&path).unwrap();
        let result = serde_json::json!({
            "file": "src/view.tsx",
            "requested_backend": "ripex",
            "used_backend": "ripex",
            "fallback_reason": null,
            "diagnostics": [],
            "facts": [{
                "id": "symbol:src/view.tsx:view",
                "kind": "symbol",
                "data": {
                    "name": "view",
                    "is_async": false,
                    "base_classes": []
                }
            }]
        });

        store.insert_parser_result_value(&result).unwrap();
        let provenance = store.get_parser_result("src/view.tsx").unwrap().unwrap();
        assert_eq!(provenance.used_backend, "ripex");
        assert_eq!(provenance.diagnostics_json, "[]");

        let facts = store.get_parser_facts_by_file("src/view.tsx").unwrap();
        assert_eq!(facts.len(), 1);
        let data: serde_json::Value = serde_json::from_str(&facts[0].data_json).unwrap();
        assert_eq!(data["name"], "view");

        store.delete_file_data("src/view.tsx").unwrap();
        assert!(store.get_parser_result("src/view.tsx").unwrap().is_none());
        assert!(store
            .get_parser_facts_by_file("src/view.tsx")
            .unwrap()
            .is_empty());
    }
    #[test]
    fn test_clear_code_data_removes_code_facts_but_preserves_metadata() {
        let path = temp_db();
        let store = SqliteStore::new(&path).unwrap();

        store
            .insert_file("src/keep.rs", "rust", "hash", 10, "2026-01-01")
            .unwrap();
        store.set_metadata("version", "test").unwrap();
        store
            .insert_symbol(
                "symbol:stale",
                "src/stale.rs",
                "rust",
                "function",
                "stale",
                false,
                1,
                1,
                "private",
                "fn stale()",
                false,
                0,
            )
            .unwrap();
        store
            .insert_import(
                "import:stale",
                "src/stale.rs",
                "rust",
                "use",
                "std::io",
                None,
                None,
                None,
                1,
                "high",
            )
            .unwrap();
        store
            .insert_call(
                "call:stale",
                "src/stale.rs",
                "rust",
                "function",
                None,
                "stale",
                None,
                None,
                1,
                1,
                "high",
            )
            .unwrap();
        store
            .insert_route(
                "route:stale",
                "src/stale.rs",
                "rust",
                "GET",
                "/stale",
                "stale",
                None,
                1,
                "test",
                &[],
            )
            .unwrap();
        store
            .insert_type_impl(
                "type_impl:stale",
                "src/stale.rs",
                "rust",
                "Stale",
                "Trait",
                1,
                "trait_impl",
            )
            .unwrap();
        store
            .insert_di_binding(
                "di:stale",
                "src/stale.rs",
                "rust",
                "Trait",
                "Stale",
                Some("singleton"),
                1,
                "test",
            )
            .unwrap();
        store
            .insert_parser_result_value(&serde_json::json!({
                "file": "src/stale.rs",
                "requested_backend": "ripex",
                "used_backend": "ripex",
                "diagnostics": [],
                "facts": [{
                    "id": "parser:stale",
                    "kind": "symbol",
                    "data": {}
                }]
            }))
            .unwrap();

        store.clear_code_data().unwrap();

        for table in [
            "symbols",
            "imports",
            "calls",
            "routes",
            "type_impls",
            "di_bindings",
            "parser_results",
            "parser_facts",
        ] {
            let count: i64 = store
                .conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(
                count, 0,
                "{table} should be empty after a full rebuild reset"
            );
        }
        assert_eq!(store.file_count().unwrap(), 1);
        assert_eq!(store.get_metadata("version").unwrap(), Some("test".into()));
    }
}

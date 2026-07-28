pub const STATEMENTS: &str = r#"
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
"#;

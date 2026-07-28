pub const STATEMENTS: &str = r#"
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
"#;

pub const STATEMENTS: &str = r#"
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
"#;

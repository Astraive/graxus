pub const STATEMENTS: &str = r#"
CREATE TABLE IF NOT EXISTS parser_results (
    file TEXT PRIMARY KEY,
    requested_backend TEXT NOT NULL,
    used_backend TEXT NOT NULL,
    fallback_reason TEXT,
    diagnostics_json TEXT NOT NULL DEFAULT '[]'
);
CREATE TABLE IF NOT EXISTS parser_facts (
    id TEXT PRIMARY KEY,
    file TEXT NOT NULL,
    kind TEXT NOT NULL,
    data_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_parser_facts_file ON parser_facts(file);
CREATE INDEX IF NOT EXISTS idx_parser_facts_kind ON parser_facts(kind);
"#;

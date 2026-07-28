pub const STATEMENTS: &str = r#"
CREATE TABLE IF NOT EXISTS di_bindings (
    id TEXT PRIMARY KEY,
    file TEXT NOT NULL,
    language TEXT NOT NULL,
    abstract_type TEXT NOT NULL,
    concrete_type TEXT NOT NULL,
    lifetime TEXT,
    line INTEGER NOT NULL,
    framework TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_di_bindings_abstract_type ON di_bindings(abstract_type);
CREATE INDEX IF NOT EXISTS idx_di_bindings_framework ON di_bindings(framework);
"#;

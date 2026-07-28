pub const STATEMENTS: &str = r#"
CREATE TABLE IF NOT EXISTS type_impls (
    id TEXT PRIMARY KEY,
    file TEXT NOT NULL,
    language TEXT NOT NULL,
    implementing_type TEXT NOT NULL,
    trait_or_interface TEXT NOT NULL,
    line INTEGER NOT NULL,
    kind TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_type_impls_trait ON type_impls(trait_or_interface);
CREATE INDEX IF NOT EXISTS idx_type_impls_type ON type_impls(implementing_type);
"#;

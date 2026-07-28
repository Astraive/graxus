pub const STATEMENTS: &str = r#"
CREATE TABLE IF NOT EXISTS routes (
    id TEXT PRIMARY KEY,
    file TEXT NOT NULL,
    language TEXT NOT NULL,
    method TEXT NOT NULL,
    path TEXT NOT NULL,
    handler TEXT NOT NULL,
    handler_file TEXT,
    line INTEGER NOT NULL,
    framework TEXT NOT NULL,
    middleware TEXT DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS idx_routes_framework ON routes(framework);
CREATE INDEX IF NOT EXISTS idx_routes_path ON routes(path);
"#;

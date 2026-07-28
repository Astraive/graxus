//! CLI error classification and display formatting.

use std::fmt;

/// Typed error categories for the CLI.
///
/// Commands return `anyhow::Result`, but we tag errors with a category
/// via `.context(CliCategory::...)` so `classify_error` can match on
/// the error chain instead of fragile string matching.
///
/// Commands can opt into typed error classification by adding
/// `.context(CliCategory::Config)` etc. to their error propagation.
#[derive(Debug)]
#[allow(dead_code)]
pub enum CliCategory {
    /// Project not initialized or bad config.
    Config,
    /// Index is stale or missing.
    Index,
    /// Safety validation violation (path traversal, unauthorized, etc.).
    Safety,
}

impl fmt::Display for CliCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliCategory::Config => write!(f, "Configuration error"),
            CliCategory::Index => write!(f, "Index error"),
            CliCategory::Safety => write!(f, "Safety validation error"),
        }
    }
}

impl std::error::Error for CliCategory {}

/// Classify an error into an exit code for the CLI process.
///
/// Walks the error chain looking for a [`CliCategory`] context.
/// Falls back to string matching on the full error message for
/// errors that don't use typed categories.
///
/// Exit codes:
/// - 1: general/runtime error
/// - 2: config/init error (project not found, bad config)
/// - 3: index stale or missing
/// - 4: safety validation violation
pub fn classify_error(e: &anyhow::Error) -> i32 {
    // First, try to find a CliCategory in the error chain
    for cause in e.chain() {
        if let Some(cat) = cause.downcast_ref::<CliCategory>() {
            return match cat {
                CliCategory::Config => 2,
                CliCategory::Index => 3,
                CliCategory::Safety => 4,
            };
        }
    }

    // Fallback: string matching for errors without typed categories
    let msg = format!("{:#}", e).to_lowercase();
    if msg.contains("not a graxus project") || msg.contains("graxus.yaml") || msg.contains("config")
    {
        2
    } else if msg.contains("index") || msg.contains("codemap") || msg.contains("stale") {
        3
    } else if msg.contains("safety") || msg.contains("traversal") || msg.contains("unauthorized")
    {
        4
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_config_error() {
        let err = anyhow::anyhow!("something failed").context(CliCategory::Config);
        assert_eq!(classify_error(&err), 2);
    }

    #[test]
    fn classify_index_error() {
        let err = anyhow::anyhow!("something failed").context(CliCategory::Index);
        assert_eq!(classify_error(&err), 3);
    }

    #[test]
    fn classify_safety_error() {
        let err = anyhow::anyhow!("something failed").context(CliCategory::Safety);
        assert_eq!(classify_error(&err), 4);
    }

    #[test]
    fn classify_fallback_string_match() {
        let err = anyhow::anyhow!("not a graxus project found");
        assert_eq!(classify_error(&err), 2);
    }

    #[test]
    fn classify_unknown_error() {
        let err = anyhow::anyhow!("random error");
        assert_eq!(classify_error(&err), 1);
    }
}

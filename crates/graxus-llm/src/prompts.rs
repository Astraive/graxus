//! Prompt templates for LLM documentation generation.
//!
//! Each function returns a `(system, user)` tuple of prompt strings
//! tailored to a specific documentation task.

/// Generate a prompt pair for module-level documentation.
///
/// Produces a system prompt for technical writing and a user prompt containing
/// the file path, language, symbols, and imports for the target module.
pub fn module_summary_prompt(
    file_path: &str,
    language: &str,
    symbols: &str,
    imports: &str,
) -> (String, String) {
    let system = "You are a technical documentation writer. Generate concise, accurate documentation for source code modules. Output valid Obsidian-compatible markdown with YAML frontmatter.".to_string();
    let user = format!(
        "Generate documentation for this module:\n\nFile: {}\nLanguage: {}\nSymbols:\n{}\nImports:\n{}\n\nWrite a brief description (2-3 sentences), key exports, and dependencies.",
        file_path, language, symbols, imports
    );
    (system, user)
}

/// Generate a prompt pair for function-level documentation.
///
/// Produces a system prompt for docstring generation and a user prompt containing
/// the function name, source code, and caller information.
pub fn function_doc_prompt(name: &str, source: &str, callers: &str) -> (String, String) {
    let system = "You are a technical writer generating docstrings for code functions. Be concise and accurate. Do not invent behavior not present in the code.".to_string();
    let user = format!(
        "Generate documentation for this function:\n\nFunction: {}\n\nSource:\n{}\n\nCalled by:\n{}\n\nGenerate a brief description, parameters, return value, and side effects.",
        name, source, callers
    );
    (system, user)
}

/// Generate a prompt pair for project architecture documentation.
///
/// Produces a system prompt for architecture writing and a user prompt containing
/// project-level statistics (file count, symbol count, languages).
pub fn architecture_prompt(
    project_name: &str,
    file_count: usize,
    symbol_count: usize,
    languages: &str,
) -> (String, String) {
    let system = "You are a software architect. Generate high-level architecture documentation from code structure data. Output valid Obsidian-compatible markdown.".to_string();
    let user = format!(
        "Generate an ARCHITECTURE.md for project '{}':\n\nTotal files: {}\nTotal symbols: {}\nLanguages: {}\n\nDescribe the high-level architecture, module responsibilities, data flow, and key design decisions.",
        project_name, file_count, symbol_count, languages
    );
    (system, user)
}

/// Generate a prompt pair for stale documentation detection.
///
/// Produces a system prompt for documentation updating and a user prompt containing
/// the current documentation and the current code state to compare against.
pub fn stale_check_prompt(doc_content: &str, code_state: &str) -> (String, String) {
    let system = "You are updating documentation that has become stale due to code changes. Preserve the original author's style. Only update what has changed.".to_string();
    let user = format!(
        "This documentation may be stale. Compare it against the current code state and suggest updates.\n\nDocument:\n{}\n\nCurrent code state:\n{}\n\nSuggest minimal updates to make the documentation accurate again.",
        doc_content, code_state
    );
    (system, user)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_summary_includes_all_params() {
        let (system, user) = module_summary_prompt("src/main.rs", "Rust", "fn main", "use std");
        assert!(system.contains("documentation"));
        assert!(user.contains("src/main.rs"));
        assert!(user.contains("Rust"));
        assert!(user.contains("fn main"));
        assert!(user.contains("use std"));
    }

    #[test]
    fn module_summary_system_prompt_is_technical_writer() {
        let (system, _) = module_summary_prompt("a.rs", "Rust", "", "");
        assert!(system.contains("technical documentation writer"));
    }

    #[test]
    fn function_doc_includes_function_details() {
        let (system, user) = function_doc_prompt(
            "calculate_sum",
            "fn calculate_sum(a: i32, b: i32) -> i32",
            "main, test",
        );
        assert!(system.contains("docstrings"));
        assert!(user.contains("calculate_sum"));
        assert!(user.contains("fn calculate_sum"));
        assert!(user.contains("main, test"));
    }

    #[test]
    fn architecture_prompt_includes_project_stats() {
        let (system, user) = architecture_prompt("graxus", 42, 500, "Rust, TypeScript");
        assert!(system.contains("architect"));
        assert!(user.contains("graxus"));
        assert!(user.contains("42"));
        assert!(user.contains("500"));
        assert!(user.contains("Rust, TypeScript"));
    }

    #[test]
    fn stale_check_prompt_includes_both_inputs() {
        let (system, user) = stale_check_prompt("# Old docs", "fn new_code() {}");
        assert!(system.contains("stale"));
        assert!(user.contains("# Old docs"));
        assert!(user.contains("fn new_code"));
    }

    #[test]
    fn all_prompts_return_nonempty_strings() {
        let (s, u) = module_summary_prompt("a.rs", "Rust", "s", "i");
        assert!(!s.is_empty());
        assert!(!u.is_empty());

        let (s, u) = function_doc_prompt("f", "src", "c");
        assert!(!s.is_empty());
        assert!(!u.is_empty());

        let (s, u) = architecture_prompt("p", 1, 2, "Rust");
        assert!(!s.is_empty());
        assert!(!u.is_empty());

        let (s, u) = stale_check_prompt("doc", "code");
        assert!(!s.is_empty());
        assert!(!u.is_empty());
    }
}

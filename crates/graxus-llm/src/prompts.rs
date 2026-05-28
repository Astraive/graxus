/// Prompt templates for LLM documentation generation.

pub fn module_summary_prompt(file_path: &str, language: &str, symbols: &str, imports: &str) -> (String, String) {
    let system = "You are a technical documentation writer. Generate concise, accurate documentation for source code modules. Output valid Obsidian-compatible markdown with YAML frontmatter.".to_string();
    let user = format!(
        "Generate documentation for this module:\n\nFile: {}\nLanguage: {}\nSymbols:\n{}\nImports:\n{}\n\nWrite a brief description (2-3 sentences), key exports, and dependencies.",
        file_path, language, symbols, imports
    );
    (system, user)
}

pub fn function_doc_prompt(name: &str, source: &str, callers: &str) -> (String, String) {
    let system = "You are a technical writer generating docstrings for code functions. Be concise and accurate. Do not invent behavior not present in the code.".to_string();
    let user = format!(
        "Generate documentation for this function:\n\nFunction: {}\n\nSource:\n{}\n\nCalled by:\n{}\n\nGenerate a brief description, parameters, return value, and side effects.",
        name, source, callers
    );
    (system, user)
}

pub fn architecture_prompt(project_name: &str, file_count: usize, symbol_count: usize, languages: &str) -> (String, String) {
    let system = "You are a software architect. Generate high-level architecture documentation from code structure data. Output valid Obsidian-compatible markdown.".to_string();
    let user = format!(
        "Generate an ARCHITECTURE.md for project '{}':\n\nTotal files: {}\nTotal symbols: {}\nLanguages: {}\n\nDescribe the high-level architecture, module responsibilities, data flow, and key design decisions.",
        project_name, file_count, symbol_count, languages
    );
    (system, user)
}

pub fn stale_check_prompt(doc_content: &str, code_state: &str) -> (String, String) {
    let system = "You are updating documentation that has become stale due to code changes. Preserve the original author's style. Only update what has changed.".to_string();
    let user = format!(
        "This documentation may be stale. Compare it against the current code state and suggest updates.\n\nDocument:\n{}\n\nCurrent code state:\n{}\n\nSuggest minimal updates to make the documentation accurate again.",
        doc_content, code_state
    );
    (system, user)
}

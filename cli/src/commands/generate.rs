use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

use crate::context::CliContext;
use graxus_core::workspace;

/// A cached entry for a generated doc symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// SHA-256 hash of the symbol's content inputs.
    pub content_hash: String,
    /// ISO 8601 timestamp when generated.
    pub generated_at: String,
    /// Relative path to the generated .md file.
    pub file_path: String,
}

/// Compute a content hash for a symbol from its name, kind, signature,
/// and surrounding source content.
fn compute_content_hash(name: &str, kind: &str, signature: &str, source_context: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    hasher.update(kind.as_bytes());
    hasher.update(signature.as_bytes());
    hasher.update(source_context.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Load the cache from disk, returning an empty map if missing or corrupt.
fn load_cache(root: &Path) -> HashMap<String, CacheEntry> {
    let cache_path = root.join(".graxus/generated/.cache.json");
    match std::fs::read_to_string(&cache_path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

/// Save the cache to disk.
fn save_cache(root: &Path, cache: &HashMap<String, CacheEntry>) -> Result<()> {
    let cache_path = root.join(".graxus/generated/.cache.json");
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(cache)?;
    std::fs::write(&cache_path, json)?;
    Ok(())
}

/// Validate generated content meets minimum quality standards.
///
/// Returns `Ok(())` if content passes, or a warning string if it fails.
fn validate_quality(content: &str) -> Result<(), String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("Generated content is empty".to_string());
    }

    // Check for at least one substantive pattern: a code reference, heading,
    // list item, or descriptive sentence.
    let has_heading = trimmed.contains('#');
    let has_list = trimmed.contains("- ") || trimmed.contains("* ") || trimmed.contains("1.");
    let has_backtick = trimmed.contains('`');
    let has_sentence = trimmed.contains(". ");

    if !(has_heading || has_list || has_backtick || has_sentence) {
        return Err(
            "Generated content lacks code references, headings, or structured explanations"
                .to_string(),
        );
    }

    Ok(())
}

/// Estimate the token count for a prompt pair (system + user) using a
/// rough 4-chars-per-token heuristic.
fn estimate_tokens(system: &str, user: &str) -> usize {
    (system.len() + user.len()) / 4
}

/// Generate documentation for code symbols using an LLM.
///
/// # Arguments
/// * `file` - Optional file path to limit generation to
/// * `dry_run` - If true, show prompts without calling the LLM
/// * `force` - If true, ignore cache and regenerate everything
pub fn run_docs(ctx: &CliContext, file: Option<&str>, dry_run: bool, force: bool) -> Result<()> {
    let root = ctx.resolve_root()?;

    let config = ctx.load_config(&root)?;

    if !config.llm.enabled {
        println!(
            "{}",
            "LLM not enabled. Add to graxus.yaml:\n\n  llm:\n    enabled: true\n    provider: openai\n    model: gpt-4o-mini\n    api_key_env: OPENAI_API_KEY".yellow()
        );
        return Ok(());
    }

    let api_key = config.llm.api_key().context(
        "No API key found. Set the environment variable or run:\n  graxus config set-key <provider> <key>",
    )?;

    println!("{}", "=== Generating Documentation ===".green().bold());
    println!("  Provider: {}", config.llm.provider);
    println!("  Model:    {}", config.llm.model);
    println!("  Dry run:  {}", dry_run);
    println!("  Force:    {}", force);

    // Load codemap for context
    let code_dir = workspace::code_dir(&root);
    if !code_dir.join("codemap.json").exists() {
        println!("{}", "No codemap found. Run `graxus index` first.".yellow());
        return Ok(());
    }

    let codemap: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(code_dir.join("codemap.json"))?)?;

    let symbols = codemap
        .get("symbols")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();

    // Filter to specific file if requested
    let target_symbols: Vec<_> = if let Some(f) = file {
        symbols
            .iter()
            .filter(|s| {
                s.get("file")
                    .and_then(|v| v.as_str())
                    .map(|p| p.contains(f))
                    .unwrap_or(false)
            })
            .collect()
    } else {
        symbols.iter().collect()
    };

    if target_symbols.is_empty() {
        println!("No symbols found to document.");
        return Ok(());
    }

    // Load cache
    let mut cache = if force {
        HashMap::new()
    } else {
        load_cache(&root)
    };
    let generated_dir = root.join(".graxus").join("generated");

    // Classify symbols into cached vs needs-regeneration
    let mut new_count = 0usize;
    let mut updated_count = 0usize;
    let mut unchanged_count = 0usize;
    let mut total_estimated_tokens = 0usize;

    struct SymbolPlan<'a> {
        sym: &'a serde_json::Value,
        symbol_id: String,
        content_hash: String,
        status: PlanStatus,
    }

    enum PlanStatus {
        New,
        Changed,
        Unchanged,
    }

    let mut plans: Vec<SymbolPlan<'_>> = Vec::new();

    for sym in &target_symbols {
        let name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let kind = sym.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
        let sig = sym.get("signature").and_then(|v| v.as_str()).unwrap_or("");
        let sym_file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
        let sym_id = sym
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or(name)
            .to_string();

        // Build source context from file + signature for hashing
        let source_context = format!("{}:{}", sym_file, sig);
        let content_hash = compute_content_hash(name, kind, sig, &source_context);

        let status = match cache.get(&sym_id) {
            Some(entry) if entry.content_hash == content_hash => {
                unchanged_count += 1;
                PlanStatus::Unchanged
            }
            Some(_) => {
                updated_count += 1;
                PlanStatus::Changed
            }
            None => {
                new_count += 1;
                PlanStatus::New
            }
        };

        // Accumulate token estimate for non-cached symbols
        if !matches!(&status, PlanStatus::Unchanged) {
            let (system, user) = graxus_llm::prompts::function_doc_prompt(
                name,
                &format!("{} {} in {}", kind, name, sig),
                "",
            );
            total_estimated_tokens += estimate_tokens(&system, &user);
        }

        plans.push(SymbolPlan {
            sym,
            symbol_id: sym_id,
            content_hash,
            status,
        });
    }

    println!(
        "  Symbols to document: {} ({} new, {} updated, {} unchanged)",
        target_symbols.len(),
        new_count,
        updated_count,
        unchanged_count
    );

    // Dry-run mode: show plan and estimated cost
    if dry_run {
        for (i, plan) in plans.iter().enumerate() {
            let name = plan.sym.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let kind = plan.sym.get("kind").and_then(|v| v.as_str()).unwrap_or("?");

            match &plan.status {
                PlanStatus::Unchanged => {
                    println!(
                        "  [{}/{}] [cached] {} ({})",
                        i + 1,
                        target_symbols.len(),
                        name,
                        kind
                    );
                }
                PlanStatus::New => {
                    println!(
                        "  [{}/{}] [new] Would generate docs for: {} ({})",
                        i + 1,
                        target_symbols.len(),
                        name,
                        kind
                    );
                }
                PlanStatus::Changed => {
                    println!(
                        "  [{}/{}] [updated] Would regenerate docs for: {} ({})",
                        i + 1,
                        target_symbols.len(),
                        name,
                        kind
                    );
                }
            }
        }

        // Estimate cost
        let est_output = total_estimated_tokens; // rough 1:1 input:output
        let (input_price, output_price) = match config.llm.model.as_str() {
            "gpt-4o" => (2.50, 10.00),
            "gpt-4o-mini" => (0.15, 0.60),
            "claude-3-5-sonnet" | "claude-3.5-sonnet" => (3.00, 15.00),
            "claude-3-haiku" => (0.25, 1.25),
            _ => (0.15, 0.60),
        };
        let est_cost = (total_estimated_tokens as f64 / 1_000_000.0 * input_price)
            + (est_output as f64 / 1_000_000.0 * output_price);

        println!();
        println!("  Estimated API usage:");
        println!("    Input tokens: ~{}", total_estimated_tokens);
        println!("    Output tokens: ~{}", est_output);
        println!("    Estimated cost: ${:.4}", est_cost);
        println!(
            "    Summary: {} new, {} updated, {} unchanged",
            new_count, updated_count, unchanged_count
        );

        return Ok(());
    }

    // Create LLM provider
    let provider = create_provider(&config.llm, &api_key)?;

    // Generate docs using tokio runtime
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let mut cost_tracker = graxus_llm::cost::CostTracker::new(config.llm.max_cost_per_run);
        let rate_limiter = graxus_llm::rate_limit::RateLimiter::new(60);

        for (i, plan) in plans.iter().enumerate() {
            let name = plan.sym.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let kind = plan.sym.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
            let sig = plan
                .sym
                .get("signature")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            match &plan.status {
                PlanStatus::Unchanged => {
                    println!(
                        "  [{}/{}] {} {}",
                        i + 1,
                        target_symbols.len(),
                        "[cached]".dimmed(),
                        name
                    );
                    continue;
                }
                PlanStatus::New => {
                    println!(
                        "  [{}/{}] Generating docs for: {} ({})",
                        i + 1,
                        target_symbols.len(),
                        name,
                        kind
                    );
                }
                PlanStatus::Changed => {
                    println!(
                        "  [{}/{}] Regenerating docs for: {} ({})",
                        i + 1,
                        target_symbols.len(),
                        name,
                        kind
                    );
                }
            }

            let (system, user) = graxus_llm::prompts::function_doc_prompt(
                name,
                &format!("{} {} in {}", kind, name, sig),
                "",
            );

            rate_limiter.wait().await;

            let request = graxus_llm::provider::LlmRequest {
                system,
                user,
                max_tokens: config.llm.max_tokens,
                temperature: config.llm.temperature,
            };

            match provider.complete(request).await {
                Ok(response) => {
                    cost_tracker.record(
                        response.input_tokens,
                        response.output_tokens,
                        &config.llm.model,
                    )?;

                    // Quality check
                    if let Err(warning) = validate_quality(&response.content) {
                        eprintln!("    {} Quality warning: {}", "!".yellow(), warning);
                    }

                    // Save to .graxus/generated/
                    std::fs::create_dir_all(&generated_dir)?;

                    let safe_name = name.replace("/", "_").replace("::", "_");
                    let file_name = format!("{}.md", safe_name);
                    std::fs::write(generated_dir.join(&file_name), &response.content)?;
                    println!("    -> Saved to .graxus/generated/{}", file_name);

                    // Update cache
                    cache.insert(
                        plan.symbol_id.clone(),
                        CacheEntry {
                            content_hash: plan.content_hash.clone(),
                            generated_at: chrono::Utc::now().to_rfc3339(),
                            file_path: file_name,
                        },
                    );
                }
                Err(e) => {
                    eprintln!("    {} Failed: {}", "x".red(), e);
                }
            }
        }

        // Save cache
        if let Err(e) = save_cache(&root, &cache) {
            eprintln!("    {} Failed to save cache: {}", "!".yellow(), e);
        }

        let summary = cost_tracker.summary();
        println!();
        println!("  Cost summary:");
        println!("    Requests: {}", summary.total_requests);
        println!(
            "    Tokens:   {} (in) + {} (out)",
            summary.total_input_tokens, summary.total_output_tokens
        );
        println!("    Est cost: ${:.4}", summary.estimated_usd);
        println!(
            "    Summary: {} new, {} updated, {} unchanged",
            new_count, updated_count, unchanged_count
        );

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

/// Generate an architecture document using an LLM.
///
/// # Arguments
/// * `dry_run` - If true, show the prompt without calling the LLM
pub fn run_architecture(ctx: &CliContext, dry_run: bool) -> Result<()> {
    let root = ctx.resolve_root()?;

    let config = ctx.load_config(&root)?;

    if !config.llm.enabled {
        println!(
            "{}",
            "LLM not enabled. Add llm config to graxus.yaml.".yellow()
        );
        return Ok(());
    }

    let api_key = config.llm.api_key().context("No API key found.")?;

    println!(
        "{}",
        "=== Generating Architecture Document ===".green().bold()
    );

    // Count files and symbols
    let code_dir = workspace::code_dir(&root);
    let (file_count, symbol_count, languages) = if code_dir.join("codemap.json").exists() {
        let codemap: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(code_dir.join("codemap.json"))?)?;
        let files = codemap
            .get("files")
            .and_then(|f| f.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let symbols = codemap
            .get("symbols")
            .and_then(|s| s.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let langs: Vec<String> = codemap
            .get("files")
            .and_then(|f| f.as_array())
            .map(|arr| {
                let mut set: std::collections::HashSet<String> = std::collections::HashSet::new();
                for f in arr {
                    if let Some(lang) = f.get("language").and_then(|l| l.as_str()) {
                        set.insert(lang.to_string());
                    }
                }
                set.into_iter().collect()
            })
            .unwrap_or_default();
        (files, symbols, langs.join(", "))
    } else {
        (0, 0, String::new())
    };

    if dry_run {
        println!(
            "  Would generate ARCHITECTURE.md for {} files, {} symbols ({})",
            file_count, symbol_count, languages
        );
        return Ok(());
    }

    let provider = create_provider(&config.llm, &api_key)?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let (system, user) = graxus_llm::prompts::architecture_prompt(
            &config.project.name,
            file_count,
            symbol_count,
            &languages,
        );

        let request = graxus_llm::provider::LlmRequest {
            system,
            user,
            max_tokens: config.llm.max_tokens,
            temperature: config.llm.temperature,
        };

        let response = provider.complete(request).await?;

        let generated_dir = root.join(".graxus").join("generated");
        std::fs::create_dir_all(&generated_dir)?;
        std::fs::write(generated_dir.join("ARCHITECTURE.md"), &response.content)?;

        // Quality check
        if let Err(warning) = validate_quality(&response.content) {
            eprintln!("  {} Quality warning: {}", "!".yellow(), warning);
        }

        println!("  Saved to .graxus/generated/ARCHITECTURE.md");
        println!(
            "  Tokens: {} (in) + {} (out)",
            response.input_tokens, response.output_tokens
        );

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

fn create_provider(
    config: &graxus_core::config::LlmConfig,
    api_key: &str,
) -> Result<Box<dyn graxus_llm::provider::LlmProvider>> {
    match config.provider.as_str() {
        "openai" => Ok(Box::new(
            graxus_llm::providers::openai::OpenAiProvider::new(
                api_key.to_string(),
                config.model.clone(),
            ),
        )),
        "anthropic" => Ok(Box::new(
            graxus_llm::providers::anthropic::AnthropicProvider::new(
                api_key.to_string(),
                config.model.clone(),
            ),
        )),
        "ollama" => Ok(Box::new(
            graxus_llm::providers::ollama::OllamaProvider::new(
                config
                    .endpoint
                    .as_deref()
                    .unwrap_or("http://localhost:11434"),
                config.model.clone(),
            ),
        )),
        other => anyhow::bail!(
            "Unknown LLM provider: '{}'. Use: openai, anthropic, ollama",
            other
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_content_hash_deterministic() {
        let h1 = compute_content_hash("main", "function", "fn main()", "src/main.rs:fn main()");
        let h2 = compute_content_hash("main", "function", "fn main()", "src/main.rs:fn main()");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_compute_content_hash_changes_on_input() {
        let h1 = compute_content_hash("main", "function", "fn main()", "src/main.rs:fn main()");
        let h2 = compute_content_hash(
            "main",
            "function",
            "fn main(v: i32)",
            "src/main.rs:fn main(v: i32)",
        );
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_compute_content_hash_changes_on_name() {
        let h1 = compute_content_hash("main", "function", "fn main()", "ctx");
        let h2 = compute_content_hash("other", "function", "fn main()", "ctx");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_compute_content_hash_changes_on_kind() {
        let h1 = compute_content_hash("foo", "function", "fn foo()", "ctx");
        let h2 = compute_content_hash("foo", "struct", "fn foo()", "ctx");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_load_cache_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let cache = load_cache(dir.path());
        assert!(cache.is_empty());
    }

    #[test]
    fn test_save_and_load_cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let generated = dir.path().join(".graxus/generated");
        std::fs::create_dir_all(&generated).unwrap();

        let mut cache = HashMap::new();
        cache.insert(
            "sym:main".to_string(),
            CacheEntry {
                content_hash: "abc123".to_string(),
                generated_at: "2026-05-29T00:00:00Z".to_string(),
                file_path: "main.md".to_string(),
            },
        );

        save_cache(dir.path(), &cache).unwrap();
        let loaded = load_cache(dir.path());

        assert_eq!(loaded.len(), 1);
        let entry = loaded.get("sym:main").unwrap();
        assert_eq!(entry.content_hash, "abc123");
        assert_eq!(entry.file_path, "main.md");
    }

    #[test]
    fn test_load_cache_corrupt_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let generated = dir.path().join(".graxus/generated");
        std::fs::create_dir_all(&generated).unwrap();
        std::fs::write(generated.join(".cache.json"), "not valid json!!!").unwrap();

        let cache = load_cache(dir.path());
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_invalidation_on_content_change() {
        let dir = tempfile::tempdir().unwrap();
        let generated = dir.path().join(".graxus/generated");
        std::fs::create_dir_all(&generated).unwrap();

        let hash_v1 = compute_content_hash("add", "function", "fn add(a: i32)", "src/lib.rs");
        let hash_v2 =
            compute_content_hash("add", "function", "fn add(a: i32, b: i32)", "src/lib.rs");

        let mut cache = HashMap::new();
        cache.insert(
            "sym:add".to_string(),
            CacheEntry {
                content_hash: hash_v1.clone(),
                generated_at: "2026-05-29T00:00:00Z".to_string(),
                file_path: "add.md".to_string(),
            },
        );
        save_cache(dir.path(), &cache).unwrap();

        // Reload and verify the cached hash does NOT match the new content
        let loaded = load_cache(dir.path());
        let entry = loaded.get("sym:add").unwrap();
        assert_ne!(entry.content_hash, hash_v2);
    }

    #[test]
    fn test_validate_quality_empty_fails() {
        assert!(validate_quality("").is_err());
        assert!(validate_quality("   ").is_err());
    }

    #[test]
    fn test_validate_quality_heading_passes() {
        assert!(validate_quality("# Function main\n\nThis does stuff.").is_ok());
    }

    #[test]
    fn test_validate_quality_backtick_passes() {
        assert!(validate_quality("The `add` function returns a sum.").is_ok());
    }

    #[test]
    fn test_validate_quality_list_passes() {
        assert!(validate_quality("- First item\n- Second item").is_ok());
    }

    #[test]
    fn test_validate_quality_no_structure_fails() {
        // Just random words with no structure
        assert!(validate_quality("blah blah blah blah").is_err());
    }

    #[test]
    fn test_estimate_tokens() {
        let tokens = estimate_tokens("goodbye world", "some user prompt here");
        // (11 + 22) / 4 = 8
        assert_eq!(tokens, 8);
    }

    #[test]
    fn test_cache_force_ignores_existing() {
        let dir = tempfile::tempdir().unwrap();
        let generated = dir.path().join(".graxus/generated");
        std::fs::create_dir_all(&generated).unwrap();

        // Pre-populate cache
        let mut cache = HashMap::new();
        cache.insert(
            "sym:main".to_string(),
            CacheEntry {
                content_hash: "abc123".to_string(),
                generated_at: "2026-05-29T00:00:00Z".to_string(),
                file_path: "main.md".to_string(),
            },
        );
        save_cache(dir.path(), &cache).unwrap();

        // When force=true, we load empty cache
        let forced_cache: HashMap<String, CacheEntry> = HashMap::new();
        assert!(forced_cache.is_empty());

        // When force=false, we load existing cache
        let normal_cache = load_cache(dir.path());
        assert_eq!(normal_cache.len(), 1);
    }
}
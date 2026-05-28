use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::path::Path;

use graxus_core::{config::GraxusConfig, workspace};

pub fn run_docs(file: Option<&str>, dry_run: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let config = GraxusConfig::load(&root)?;

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
            .filter(|s| s.get("file").and_then(|v| v.as_str()).map(|p| p.contains(f)).unwrap_or(false))
            .collect()
    } else {
        symbols.iter().collect()
    };

    if target_symbols.is_empty() {
        println!("No symbols found to document.");
        return Ok(());
    }

    println!("  Symbols to document: {}", target_symbols.len());

    // Create LLM provider
    let provider = create_provider(&config.llm, &api_key)?;

    // Generate docs using tokio runtime
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let mut cost_tracker = graxus_llm::cost::CostTracker::new(config.llm.max_cost_per_run);
        let rate_limiter = graxus_llm::rate_limit::RateLimiter::new(60);

        for (i, sym) in target_symbols.iter().enumerate() {
            let name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let kind = sym.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
            let _sym_file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("?");
            let sig = sym.get("signature").and_then(|v| v.as_str()).unwrap_or("");

            let (system, user) = graxus_llm::prompts::function_doc_prompt(
                name,
                &format!("{} {} in {}", kind, name, sig),
                "",
            );

            rate_limiter.wait().await;

            if dry_run {
                println!("  [{}/{}] Would generate docs for: {} ({})", i + 1, target_symbols.len(), name, kind);
                println!("    Prompt: {}", truncate(&user, 100));
            } else {
                println!("  [{}/{}] Generating docs for: {} ({})", i + 1, target_symbols.len(), name, kind);

                let request = graxus_llm::provider::LlmRequest {
                    system,
                    user,
                    max_tokens: config.llm.max_tokens,
                    temperature: config.llm.temperature,
                };

                match provider.complete(request).await {
                    Ok(response) => {
                        cost_tracker.record(response.input_tokens, response.output_tokens, &config.llm.model)?;

                        // Save to .graxus/generated/
                        let generated_dir = root.join(".graxus").join("generated");
                        std::fs::create_dir_all(&generated_dir)?;

                        let safe_name = name.replace("/", "_").replace("::", "_");
                        let file_name = format!("{}.md", safe_name);
                        std::fs::write(generated_dir.join(&file_name), &response.content)?;
                        println!("    → Saved to .graxus/generated/{}", file_name);
                    }
                    Err(e) => {
                        eprintln!("    {} Failed: {}", "✗".red(), e);
                    }
                }
            }
        }

        if !dry_run {
            let summary = cost_tracker.summary();
            println!();
            println!("  Cost summary:");
            println!("    Requests: {}", summary.total_requests);
            println!("    Tokens:   {} (in) + {} (out)", summary.total_input_tokens, summary.total_output_tokens);
            println!("    Est cost: ${:.4}", summary.estimated_usd);
        }

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

pub fn run_architecture(dry_run: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let config = GraxusConfig::load(&root)?;

    if !config.llm.enabled {
        println!(
            "{}",
            "LLM not enabled. Add llm config to graxus.yaml.".yellow()
        );
        return Ok(());
    }

    let api_key = config.llm.api_key().context("No API key found.")?;

    println!("{}", "=== Generating Architecture Document ===".green().bold());

    // Count files and symbols
    let code_dir = workspace::code_dir(&root);
    let (file_count, symbol_count, languages) = if code_dir.join("codemap.json").exists() {
        let codemap: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(code_dir.join("codemap.json"))?)?;
        let files = codemap.get("files").and_then(|f| f.as_array()).map(|a| a.len()).unwrap_or(0);
        let symbols = codemap.get("symbols").and_then(|s| s.as_array()).map(|a| a.len()).unwrap_or(0);
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
        println!("  Would generate ARCHITECTURE.md for {} files, {} symbols ({})", file_count, symbol_count, languages);
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

        println!("  Saved to .graxus/generated/ARCHITECTURE.md");
        println!("  Tokens: {} (in) + {} (out)", response.input_tokens, response.output_tokens);

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

fn create_provider(
    config: &graxus_core::config::LlmConfig,
    api_key: &str,
) -> Result<Box<dyn graxus_llm::provider::LlmProvider>> {
    match config.provider.as_str() {
        "openai" => Ok(Box::new(graxus_llm::providers::openai::OpenAiProvider::new(
            api_key.to_string(),
            config.model.clone(),
        ))),
        "anthropic" => Ok(Box::new(graxus_llm::providers::anthropic::AnthropicProvider::new(
            api_key.to_string(),
            config.model.clone(),
        ))),
        "ollama" => Ok(Box::new(graxus_llm::providers::ollama::OllamaProvider::new(
            config.endpoint.as_deref().unwrap_or("http://localhost:11434"),
            config.model.clone(),
        ))),
        other => anyhow::bail!("Unknown LLM provider: '{}'. Use: openai, anthropic, ollama", other),
    }
}

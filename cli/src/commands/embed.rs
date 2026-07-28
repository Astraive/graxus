use anyhow::{Context, Result};
use colored::Colorize;

use crate::context::CliContext;
use graxus_core::{scanner, workspace};

/// Generate vector embeddings for semantic search.
///
/// # Arguments
/// * `refresh` - If true, re-embed everything ignoring the cache
/// * `_dry_run` - If true, show what would be embedded without doing it
pub fn run(ctx: &CliContext, refresh: bool, _dry_run: bool) -> Result<()> {
    let root = ctx.resolve_root()?;

    let config = ctx.load_config(&root)?;

    if !config.embeddings.enabled {
        println!(
            "{}",
            "Embeddings not enabled. Add to graxus.yaml:\n\n  embeddings:\n    enabled: true\n    provider: openai\n    model: text-embedding-3-small\n    api_key_env: OPENAI_API_KEY".yellow()
        );
        return Ok(());
    }

    let api_key = config.embeddings.api_key().context(
        "No API key found. Set the environment variable or run:\n  graxus config set-key <provider> <key>",
    )?;

    println!("{}", "=== Generating Embeddings ===".green().bold());
    println!("  Provider: {}", config.embeddings.provider);
    println!("  Model:    {}", config.embeddings.model);
    println!("  Refresh:  {}", refresh);
    println!();

    // Scan files
    let (docs, code, _) = scanner::scan_categorized(&root, &config)?;
    let _all_files: Vec<_> = docs.into_iter().chain(code).collect();

    // Load existing doc graph for text extraction
    let docs_dir = workspace::docs_dir(&root);
    let code_dir = workspace::code_dir(&root);

    let mut items: Vec<(String, String, String)> = Vec::new();

    // Extract doc content
    if docs_dir.join("graph.json").exists() {
        let graph: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(docs_dir.join("graph.json"))?)?;
        if let Some(nodes) = graph.get("nodes").and_then(|n| n.as_array()) {
            for node in nodes {
                let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let title = node.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let text = format!("{}: {}", id, title);
                items.push((id.to_string(), "doc".to_string(), text));
            }
        }
    }

    // Extract code symbols
    if code_dir.join("codemap.json").exists() {
        let codemap: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(code_dir.join("codemap.json"))?)?;
        if let Some(symbols) = codemap.get("symbols").and_then(|s| s.as_array()) {
            for sym in symbols {
                let name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
                let kind = sym.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                let sig = sym.get("signature").and_then(|v| v.as_str()).unwrap_or("");
                let text = format!("{} {} in {} - {}", kind, name, file, sig);
                items.push((
                    format!("symbol:{}:{}", file, name),
                    "symbol".to_string(),
                    text,
                ));
            }
        }
    }

    if items.is_empty() {
        println!("No content to embed. Run `graxus index` first.");
        return Ok(());
    }

    println!("  Items to embed: {}", items.len());

    // Build embedding pipeline
    let store_path = root.join(".graxus").join("embeddings").join("vectors.json");

    // Use tokio runtime for async embedding
    let rt = tokio::runtime::Runtime::new()?;
    let embed_config = config.embeddings.clone();
    rt.block_on(async move {
        let provider = create_provider(&embed_config, &api_key)?;
        let mut pipeline = graxus_embed::EmbeddingPipeline::new(provider, embed_config.batch_size);

        let stats = pipeline.embed_texts(items).await?;

        println!();
        println!("  Embedded: {}", stats.embedded);
        println!("  Skipped:  {}", stats.skipped);
        println!("  Errors:   {}", stats.errors);

        // Save
        if let Some(parent) = store_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        pipeline.store().save(&store_path)?;
        println!("  Saved to: {}", store_path.display());

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

fn create_provider(
    config: &graxus_core::config::EmbeddingsConfig,
    api_key: &str,
) -> Result<Box<dyn graxus_embed::EmbeddingProvider>> {
    match config.provider.as_str() {
        "openai" => Ok(Box::new(graxus_embed::providers::OpenAIProvider::new(
            api_key.to_string(),
            Some(config.model.clone()),
        ))),
        "cohere" => Ok(Box::new(graxus_embed::providers::CohereProvider::new(
            api_key.to_string(),
            Some(config.model.clone()),
        ))),
        "ollama" => Ok(Box::new(graxus_embed::providers::OllamaProvider::new(
            config.endpoint.clone(),
            Some(config.model.clone()),
        ))),
        other => anyhow::bail!(
            "Unknown embedding provider: '{}'. Use: openai, cohere, ollama",
            other
        ),
    }
}

use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::path::Path;

use graxus_core::{config::GraxusConfig, workspace};

pub fn run(query: &str, top_k: usize) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let config = GraxusConfig::load(&root)?;

    if !config.embeddings.enabled {
        println!(
            "{}",
            "Embeddings not enabled. Run `graxus embed` first, or add embeddings config to graxus.yaml.".yellow()
        );
        return Ok(());
    }

    let store_path = root.join(".graxus").join("embeddings").join("vectors.json");
    if !store_path.exists() {
        println!("{}", "No embeddings found. Run `graxus embed` first.".yellow());
        return Ok(());
    }

    let api_key = config.embeddings.api_key().context(
        "No API key found. Set the environment variable or run:\n  graxus config set-key <provider> <key>",
    )?;

    let store = graxus_embed::VectorStore::load(&store_path)?;
    let embed_config = config.embeddings.clone();

    // Embed the query
    let rt = tokio::runtime::Runtime::new()?;
    let results = rt.block_on(async move {
        let provider = create_provider(&embed_config, &api_key)?;
        let query_vec = provider.embed(&[query.to_string()]).await?;
        let query_embedding = query_vec.into_iter().next().unwrap_or_default();
        let search_results = store.search(&query_embedding, top_k);
        Ok::<Vec<(String, f32, String)>, anyhow::Error>(
            search_results.into_iter().map(|(r, s)| (r.id.clone(), s, r.text.clone())).collect()
        )
    })?;

    if results.is_empty() {
        println!("No semantic matches for '{}'", query);
        return Ok(());
    }

    println!(
        "{}",
        format!("=== Semantic results for '{}' ===", query).green().bold()
    );

    for (i, (id, score, text)) in results.iter().enumerate() {
        println!(
            "  {}. [{}] {} — {}",
            i + 1,
            format!("{:.2}", score).cyan(),
            id,
            truncate(text, 80)
        );
    }

    println!("\n  Total: {} results", results.len());
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
        other => anyhow::bail!("Unknown embedding provider: '{}'. Use: openai, cohere, ollama", other),
    }
}

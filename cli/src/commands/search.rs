use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::context::CliContext;
use graxus_core::{config::GraxusConfig, workspace, workspaces};

/// A search result with its source project information.
#[derive(Debug, Clone)]
struct ProjectSearchResult {
    id: String,
    score: f32,
    text: String,
    project: String,
}

/// Perform search using the specified mode.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `query` - Search query string
/// * `top_k` - Number of results to return
/// * `min_score` - Minimum similarity score (0.0-1.0); results below this are dropped
/// * `mode` - Search mode: "vector", "keyword", or "hybrid"
/// * `file_filter` - Optional file path to limit search results
pub fn run(
    ctx: &CliContext,
    query: &str,
    top_k: usize,
    min_score: f64,
    mode: &str,
    file_filter: Option<&str>,
) -> Result<()> {
    let root = ctx.resolve_root()?;

    let mut results = match mode {
        "keyword" => keyword_search(query, &root, top_k * 2),
        "vector" => run_vector_search(query, top_k, &root)?,
        "hybrid" => {
            let keyword_results = keyword_search(query, &root, top_k * 2);
            let vector_results = run_vector_search(query, top_k, &root).unwrap_or_default();
            if vector_results.is_empty() {
                keyword_results
            } else {
                rrf_merge(keyword_results, vector_results, 60)
            }
        }
        other => anyhow::bail!(
            "Unknown search mode '{}'. Use: vector, keyword, hybrid",
            other
        ),
    };

    // Drop results below the --min-score threshold (scores are normalized to 0.0-1.0).
    if min_score > 0.0 {
        results.retain(|(_, score, _)| *score as f64 >= min_score);
    }

    // Apply file filter
    if let Some(filter) = file_filter {
        let filter_lower = filter.to_lowercase();
        results.retain(|(id, _, _)| id.to_lowercase().contains(&filter_lower));
    }

    results.truncate(top_k);
    display_results(&results, query, mode);

    Ok(())
}

/// Search across all sub-projects in a workspace.
///
/// Merges results from multiple projects, deduplicating by symbol name,
/// and shows which project each result comes from.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `query` - Search query string
/// * `top_k` - Number of results to return per project
/// * `min_score` - Minimum similarity score (0.0-1.0); results below this are dropped
pub fn run_workspace(ctx: &CliContext, query: &str, top_k: usize, min_score: f64) -> Result<()> {
    let root = ctx.resolve_root()?;

    let ws_info = workspaces::detect_workspace(&root);

    if !ws_info.is_monorepo {
        println!(
            "{}",
            "Not a monorepo. Use `graxus search` for single-project search.".yellow()
        );
        return run(ctx, query, top_k, min_score, "vector", None);
    }

    let config = ctx.load_config(&root)?;

    if !config.embeddings.enabled {
        println!(
            "{}",
            "Embeddings not enabled. Run `graxus embed` first, or add embeddings config to graxus.yaml.".yellow()
        );
        return Ok(());
    }

    let api_key = config.embeddings.api_key().context(
        "No API key found. Set the environment variable or run:\n  graxus config set-key <provider> <key>",
    )?;

    println!(
        "{}",
        format!("=== Workspace search for '{}' ===", query)
            .green()
            .bold()
    );
    println!(
        "  Searching {} sub-projects...\n",
        ws_info.sub_projects.len()
    );

    let mut all_results: Vec<ProjectSearchResult> = Vec::new();

    // Search each sub-project
    for sub_path in &ws_info.sub_projects {
        let sub_name = sub_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let store_path = sub_path
            .join(".graxus")
            .join("embeddings")
            .join("vectors.json");

        if !store_path.exists() {
            println!("  {} No embeddings for {}", "-".yellow(), sub_name);
            continue;
        }

        let store = match graxus_embed::VectorStore::load(&store_path) {
            Ok(s) => s,
            Err(e) => {
                println!("  {} Failed to load {}: {}", "!".yellow(), sub_name, e);
                continue;
            }
        };

        let embed_config = config.embeddings.clone();
        let query_owned = query.to_string();
        let api_key_owned = api_key.clone();

        let rt = tokio::runtime::Runtime::new()?;
        let results = rt.block_on(async move {
            let provider = create_provider(&embed_config, &api_key_owned)?;
            let query_vec = provider.embed(&[query_owned]).await?;
            let query_embedding = query_vec.into_iter().next().unwrap_or_default();
            let search_results = store.search(&query_embedding, top_k);
            Ok::<Vec<(String, f32, String)>, anyhow::Error>(
                search_results
                    .into_iter()
                    .map(|(r, s)| (r.id.clone(), s, r.text.clone()))
                    .collect(),
            )
        });

        match results {
            Ok(hits) => {
                for (id, score, text) in hits {
                    all_results.push(ProjectSearchResult {
                        id,
                        score,
                        text,
                        project: sub_name.clone(),
                    });
                }
            }
            Err(e) => {
                println!("  {} Search failed for {}: {}", "!".yellow(), sub_name, e);
            }
        }
    }

    if all_results.is_empty() {
        println!("No semantic matches for '{}' across workspace", query);
        return Ok(());
    }

    // Deduplicate by symbol name, keeping highest score
    let mut deduped: HashMap<String, ProjectSearchResult> = HashMap::new();
    for result in all_results {
        let entry = deduped.entry(result.id.clone()).or_insert(result.clone());
        if result.score > entry.score {
            *entry = result;
        }
    }

    // Sort by score descending
    let mut sorted: Vec<ProjectSearchResult> = deduped.into_values().collect();
    sorted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Drop results below the --min-score threshold.
    if min_score > 0.0 {
        sorted.retain(|r| r.score as f64 >= min_score);
    }

    // Truncate to top_k
    sorted.truncate(top_k);

    for (i, result) in sorted.iter().enumerate() {
        println!(
            "  {}. [{}] {} ({}) — {}",
            i + 1,
            format!("{:.2}", result.score).cyan(),
            result.id,
            result.project.blue(),
            truncate(&result.text, 80)
        );
    }

    println!("\n  Total: {} results (deduplicated)", sorted.len());
    Ok(())
}

/// Run vector (semantic) search using embeddings.
fn run_vector_search(query: &str, top_k: usize, root: &Path) -> Result<Vec<(String, f32, String)>> {
    let config = GraxusConfig::load(root)?;

    if !config.embeddings.enabled {
        println!(
            "{}",
            "Embeddings not enabled. Run `graxus embed` first, or add embeddings config to graxus.yaml.".yellow()
        );
        return Ok(vec![]);
    }

    let store_path = root.join(".graxus").join("embeddings").join("vectors.json");
    if !store_path.exists() {
        println!(
            "{}",
            "No embeddings found. Run `graxus embed` first.".yellow()
        );
        return Ok(vec![]);
    }

    let api_key = config.embeddings.api_key().context(
        "No API key found. Set the environment variable or run:\n  graxus config set-key <provider> <key>",
    )?;

    let store = graxus_embed::VectorStore::load(&store_path)?;
    let embed_config = config.embeddings.clone();

    let rt = tokio::runtime::Runtime::new()?;
    let results = rt.block_on(async move {
        let provider = create_provider(&embed_config, &api_key)?;
        let query_vec = provider.embed(&[query.to_string()]).await?;
        let query_embedding = query_vec.into_iter().next().unwrap_or_default();
        let search_results = store.search(&query_embedding, top_k);
        Ok::<Vec<(String, f32, String)>, anyhow::Error>(
            search_results
                .into_iter()
                .map(|(r, s)| (r.id.clone(), s, r.text.clone()))
                .collect(),
        )
    })?;

    Ok(results)
}

/// Keyword search through codemap symbols and docgraph nodes.
///
/// Scores results by match quality:
/// - Exact name match (case-insensitive) = 1.0
/// - Query is a substring of name = 0.7
/// - Name is a substring of query or word overlap = 0.5
fn keyword_search(query: &str, root: &Path, top_k: usize) -> Vec<(String, f32, String)> {
    let mut results: Vec<(String, f32, String)> = Vec::new();
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();

    // Search codemap symbols
    let codemap_path = workspace::code_dir(root).join("codemap.json");
    if let Ok(content) = std::fs::read_to_string(&codemap_path) {
        if let Ok(codemap) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(symbols) = codemap.get("symbols").and_then(|s| s.as_array()) {
                for sym in symbols {
                    let name = sym.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let file = sym.get("file").and_then(|v| v.as_str()).unwrap_or("");
                    let kind = sym.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                    let signature = sym.get("signature").and_then(|v| v.as_str()).unwrap_or("");
                    let name_lower = name.to_lowercase();

                    let score = keyword_score(&name_lower, &query_lower, &query_words);
                    if score > 0.0 {
                        let id = format!("sym:{}:{}", file, name);
                        let sig_part = if signature.is_empty() {
                            String::new()
                        } else {
                            format!(" {}", signature)
                        };
                        let text = format!("{} {}{} ({})", kind, name, sig_part, file);
                        results.push((id, score, text));
                    }
                }
            }
        }
    }

    // Search docgraph nodes
    let docs_dir = workspace::docs_dir(root);
    if let Ok(graph) = graxus_docgraph::graph::DocGraph::load(&docs_dir) {
        for node in &graph.nodes {
            let title_lower = node.title.to_lowercase();

            let title_score = keyword_score(&title_lower, &query_lower, &query_words);

            let heading_score = node
                .headings
                .iter()
                .filter_map(|h| {
                    let s = keyword_score(&h.text.to_lowercase(), &query_lower, &query_words);
                    if s > 0.0 {
                        Some(s)
                    } else {
                        None
                    }
                })
                .fold(0.0f32, f32::max);

            let path_lower = node.path.to_lowercase();
            let path_score = if !query_lower.is_empty() && path_lower.contains(&query_lower) {
                0.5
            } else {
                0.0
            };

            let best_score = title_score.max(heading_score).max(path_score);
            if best_score > 0.0 {
                let id = node.id.clone();
                let text = format!("{} -- {}", node.title, node.path);
                results.push((id, best_score, text));
            }
        }
    }

    // Deduplicate by ID, keeping highest score
    results.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    results.dedup_by(|a, b| a.0 == b.0);
    results.truncate(top_k);
    results
}

/// Calculate keyword match score for text against a query.
///
/// Returns:
/// - 1.0 for exact match
/// - 0.7 if query is a substring of text
/// - 0.5 if text is a substring of query or any query word matches
/// - 0.0 for no match
fn keyword_score(text_lower: &str, query_lower: &str, query_words: &[&str]) -> f32 {
    if query_lower.is_empty() || text_lower.is_empty() {
        return 0.0;
    }
    if text_lower == query_lower {
        return 1.0;
    }
    if text_lower.contains(query_lower) {
        return 0.7;
    }
    if query_lower.contains(text_lower) {
        return 0.5;
    }
    if query_words
        .iter()
        .any(|w| !w.is_empty() && text_lower.contains(w))
    {
        return 0.5;
    }
    0.0
}

/// Merge two result lists using Reciprocal Rank Fusion (RRF).
///
/// For each result in each list, adds `1 / (k + rank)` to its score.
/// Results appearing in both lists get accumulated scores (higher is better).
/// Deduplicates by ID, keeping the merged score and first occurrence's text.
fn rrf_merge(
    list_a: Vec<(String, f32, String)>,
    list_b: Vec<(String, f32, String)>,
    k: usize,
) -> Vec<(String, f32, String)> {
    let mut scores: HashMap<String, f32> = HashMap::new();
    let mut texts: HashMap<String, String> = HashMap::new();

    for (rank, (id, _score, text)) in list_a.iter().enumerate() {
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (k as f32 + rank as f32 + 1.0);
        texts.entry(id.clone()).or_insert_with(|| text.clone());
    }

    for (rank, (id, _score, text)) in list_b.iter().enumerate() {
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (k as f32 + rank as f32 + 1.0);
        texts.entry(id.clone()).or_insert_with(|| text.clone());
    }

    let mut merged: Vec<(String, f32, String)> = scores
        .into_iter()
        .filter_map(|(id, score)| texts.remove(&id).map(|text| (id, score, text)))
        .collect();

    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    merged
}

/// Extract the file path portion from a result ID.
///
/// Handles IDs like "sym:src/main.rs:func", "doc:notes/readme.md", "file:src/main.rs:func"
fn extract_file(id: &str) -> &str {
    let rest = id
        .strip_prefix("sym:")
        .or_else(|| id.strip_prefix("doc:"))
        .or_else(|| id.strip_prefix("file:"))
        .unwrap_or(id);
    rest.rsplit_once(':').map(|(path, _)| path).unwrap_or(rest)
}

/// Display search results grouped by file.
fn display_results(results: &[(String, f32, String)], query: &str, mode: &str) {
    if results.is_empty() {
        println!("No matches for '{}'", query);
        return;
    }

    let mode_label = match mode {
        "vector" => "Semantic",
        "keyword" => "Keyword",
        "hybrid" => "Hybrid",
        _ => "Search",
    };

    println!(
        "{}",
        format!("=== {} results for '{}' ===", mode_label, query)
            .green()
            .bold()
    );

    // Group results by file path for readability
    let mut by_file: BTreeMap<String, Vec<(usize, f32, &String)>> = BTreeMap::new();
    for (i, (id, score, text)) in results.iter().enumerate() {
        let file = extract_file(id).to_string();
        by_file.entry(file).or_default().push((i, *score, text));
    }

    for (file, items) in &by_file {
        println!("\n  {}:", file.cyan());
        for (i, score, text) in items {
            println!(
                "    {}. [{}] {}",
                i + 1,
                format!("{:.4}", score).cyan(),
                truncate(text, 100)
            );
        }
    }

    println!("\n  Total: {} results", results.len());
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
        other => anyhow::bail!(
            "Unknown embedding provider: '{}'. Use: openai, cohere, ollama",
            other
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn setup_test_project(dir: &Path) {
        let code_dir = dir.join(".graxus").join("code");
        fs::create_dir_all(&code_dir).unwrap();
        let codemap = serde_json::json!({
            "symbols": [
                {"name": "search", "kind": "function", "file": "src/search.rs", "line_start": 10, "visibility": "public", "signature": "fn search()", "is_test": false, "usage_count": 5},
                {"name": "vector_store", "kind": "struct", "file": "src/store.rs", "line_start": 20, "visibility": "public", "signature": "struct VectorStore", "is_test": false, "usage_count": 3},
                {"name": "run", "kind": "function", "file": "src/main.rs", "line_start": 1, "visibility": "public", "signature": "fn run()", "is_test": false, "usage_count": 1},
                {"name": "keyword_search", "kind": "function", "file": "src/search.rs", "line_start": 50, "visibility": "pub", "signature": "fn keyword_search()", "is_test": false, "usage_count": 2}
            ],
            "files": [],
            "imports": [],
            "calls": []
        });
        fs::write(
            code_dir.join("codemap.json"),
            serde_json::to_string(&codemap).unwrap(),
        )
        .unwrap();

        let docs_dir = dir.join(".graxus").join("docs");
        fs::create_dir_all(&docs_dir).unwrap();
        let graph = serde_json::json!({
            "nodes": [
                {
                    "id": "doc:search.md",
                    "node_type": "DOCUMENT",
                    "path": "search.md",
                    "title": "Search Guide",
                    "tags": ["search", "api"],
                    "frontmatter": null,
                    "headings": [{"level": 1, "text": "How to Search", "line": 1}],
                    "wiki_links": []
                },
                {
                    "id": "doc:vectors.md",
                    "node_type": "DOCUMENT",
                    "path": "vectors.md",
                    "title": "Vector Embeddings",
                    "tags": ["embeddings"],
                    "frontmatter": null,
                    "headings": [{"level": 1, "text": "Embedding Search", "line": 1}, {"level": 2, "text": "Cosine Similarity", "line": 10}],
                    "wiki_links": []
                },
                {
                    "id": "doc:api.md",
                    "node_type": "DOCUMENT",
                    "path": "api.md",
                    "title": "API Reference",
                    "tags": ["api"],
                    "frontmatter": null,
                    "headings": [{"level": 1, "text": "REST Endpoints", "line": 1}],
                    "wiki_links": []
                }
            ],
            "edges": []
        });
        fs::write(
            docs_dir.join("graph.json"),
            serde_json::to_string(&graph).unwrap(),
        )
        .unwrap();
    }

    // -- keyword_score tests --

    #[test]
    fn keyword_score_exact_match() {
        assert!((keyword_score("search", "search", &["search"]) - 1.0).abs() < 0.001);
    }

    #[test]
    fn keyword_score_substring() {
        assert!((keyword_score("vector_store", "vector", &["vector"]) - 0.7).abs() < 0.001);
    }

    #[test]
    fn keyword_score_reverse_substring() {
        assert!((keyword_score("run", "run_function", &["run", "function"]) - 0.5).abs() < 0.001);
    }

    #[test]
    fn keyword_score_word_overlap() {
        assert!((keyword_score("search_results", "search", &["search"]) - 0.7).abs() < 0.001);
    }

    #[test]
    fn keyword_score_no_match() {
        assert_eq!(keyword_score("unrelated", "search", &["search"]), 0.0);
    }

    #[test]
    fn keyword_score_empty_query() {
        assert_eq!(keyword_score("anything", "", &[]), 0.0);
    }

    #[test]
    fn keyword_score_empty_text() {
        assert_eq!(keyword_score("", "query", &["query"]), 0.0);
    }

    #[test]
    fn keyword_score_case_insensitive() {
        // keyword_score expects pre-lowered inputs; verify that matching lowered strings score 1.0
        assert!(
            (keyword_score("vectorstore", "vectorstore", &["vectorstore"]) - 1.0).abs() < 0.001
        );
    }

    // -- keyword_search tests --

    #[test]
    fn keyword_search_exact_symbol_match() {
        let dir = tempdir().unwrap();
        setup_test_project(dir.path());

        let results = keyword_search("search", dir.path(), 10);
        let exact = results
            .iter()
            .find(|(id, score, _)| id == "sym:src/search.rs:search" && (*score - 1.0).abs() < 0.01);
        assert!(exact.is_some(), "Expected exact match for 'search' symbol");
    }

    #[test]
    fn keyword_search_substring_symbol_match() {
        let dir = tempdir().unwrap();
        setup_test_project(dir.path());

        let results = keyword_search("vector", dir.path(), 10);
        let substring = results
            .iter()
            .find(|(id, score, _)| id.contains("vector_store") && (*score - 0.7).abs() < 0.01);
        assert!(
            substring.is_some(),
            "Expected substring match for 'vector' in 'vector_store'"
        );
    }

    #[test]
    fn keyword_search_doc_title_match() {
        let dir = tempdir().unwrap();
        setup_test_project(dir.path());

        let results = keyword_search("embedding", dir.path(), 10);
        let doc_match = results
            .iter()
            .find(|(id, score, _)| id.contains("vectors.md") && *score > 0.0);
        assert!(doc_match.is_some(), "Expected doc match for 'embedding'");
    }

    #[test]
    fn keyword_search_heading_match() {
        let dir = tempdir().unwrap();
        setup_test_project(dir.path());

        let results = keyword_search("cosine", dir.path(), 10);
        let heading_match = results
            .iter()
            .find(|(id, score, _)| id.contains("vectors.md") && (*score - 0.7).abs() < 0.01);
        assert!(
            heading_match.is_some(),
            "Expected heading match for 'cosine'"
        );
    }

    #[test]
    fn keyword_search_no_match() {
        let dir = tempdir().unwrap();
        setup_test_project(dir.path());

        let results = keyword_search("xyznonexistent", dir.path(), 10);
        assert!(
            results.is_empty(),
            "Expected no results for nonexistent query"
        );
    }

    #[test]
    fn keyword_search_respects_top_k() {
        let dir = tempdir().unwrap();
        setup_test_project(dir.path());

        let results = keyword_search("search", dir.path(), 1);
        assert!(results.len() <= 1, "Expected at most 1 result with top_k=1");
    }

    // -- rrf_merge tests --

    #[test]
    fn rrf_merge_deduplicates() {
        let list_a = vec![
            ("a".into(), 0.9, "text a".into()),
            ("b".into(), 0.8, "text b".into()),
        ];
        let list_b = vec![
            ("b".into(), 0.7, "text b".into()),
            ("c".into(), 0.6, "text c".into()),
        ];

        let merged = rrf_merge(list_a, list_b, 60);
        assert_eq!(merged.len(), 3, "Expected 3 unique results after merge");
    }

    #[test]
    fn rrf_merge_accumulates_scores() {
        let list_a = vec![("x".into(), 0.9, "text x".into())];
        let list_b = vec![("x".into(), 0.5, "text x".into())];

        let merged = rrf_merge(list_a, list_b, 60);
        assert_eq!(merged.len(), 1);
        // RRF score: 1/(60+0+1) + 1/(60+0+1) = 2/61
        let expected = 2.0 / 61.0;
        assert!(
            (merged[0].1 - expected).abs() < 0.001,
            "Expected RRF score ~{:.4}, got {:.4}",
            expected,
            merged[0].1
        );
    }

    #[test]
    fn rrf_merge_preserves_ordering() {
        let list_a = vec![
            ("a".into(), 0.9, "text a".into()),
            ("b".into(), 0.8, "text b".into()),
        ];
        let list_b = vec![
            ("b".into(), 0.7, "text b".into()),
            ("c".into(), 0.6, "text c".into()),
        ];

        let merged = rrf_merge(list_a, list_b, 60);
        // "b" appears in both lists at rank 0 and 1, so should have highest RRF score
        assert_eq!(merged[0].0, "b", "Expected 'b' to have highest RRF score");
    }

    #[test]
    fn rrf_merge_empty_lists() {
        let merged = rrf_merge(vec![], vec![], 60);
        assert!(merged.is_empty());
    }

    #[test]
    fn rrf_merge_single_list() {
        let list = vec![
            ("a".into(), 0.9, "text a".into()),
            ("b".into(), 0.8, "text b".into()),
        ];
        let merged = rrf_merge(list.clone(), vec![], 60);
        assert_eq!(merged.len(), 2);
        // First item should have score 1/(60+1), second 1/(60+2)
        assert!((merged[0].1 - 1.0 / 61.0).abs() < 0.001);
        assert!((merged[1].1 - 1.0 / 62.0).abs() < 0.001);
    }

    // -- file filter tests --

    #[test]
    fn file_filter_limits_results() {
        let dir = tempdir().unwrap();
        setup_test_project(dir.path());

        let results = keyword_search("search", dir.path(), 10);
        let filter = "search.rs";
        let filter_lower = filter.to_lowercase();
        let filtered: Vec<_> = results
            .into_iter()
            .filter(|(id, _, _)| id.to_lowercase().contains(&filter_lower))
            .collect();

        assert!(
            filtered.iter().all(|(id, _, _)| id.contains("search.rs")),
            "All filtered results should contain 'search.rs'"
        );
        assert!(
            !filtered.is_empty(),
            "Expected at least one result for search.rs"
        );
    }

    #[test]
    fn file_filter_no_matches() {
        let dir = tempdir().unwrap();
        setup_test_project(dir.path());

        let results = keyword_search("search", dir.path(), 10);
        let filter = "nonexistent.rs";
        let filter_lower = filter.to_lowercase();
        let filtered: Vec<_> = results
            .into_iter()
            .filter(|(id, _, _)| id.to_lowercase().contains(&filter_lower))
            .collect();

        assert!(
            filtered.is_empty(),
            "Expected no results for nonexistent file filter"
        );
    }

    // -- extract_file tests --

    #[test]
    fn extract_file_from_sym_id() {
        assert_eq!(extract_file("sym:src/main.rs:func"), "src/main.rs");
    }

    #[test]
    fn extract_file_from_doc_id() {
        assert_eq!(extract_file("doc:notes/readme.md"), "notes/readme.md");
    }

    #[test]
    fn extract_file_from_file_id() {
        assert_eq!(extract_file("file:src/lib.rs:struct_name"), "src/lib.rs");
    }

    #[test]
    fn extract_file_unknown_prefix() {
        // For unknown prefixes, rsplit_once still extracts the path portion
        assert_eq!(extract_file("unknown:something"), "unknown");
    }
}

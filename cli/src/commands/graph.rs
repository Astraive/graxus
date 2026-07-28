use anyhow::{Context, Result};
use colored::Colorize;

use graxus_core::workspace;
use graxus_docgraph::graph::DocGraph;

use crate::context::CliContext;

/// Show the docs graph structure or details for a specific file.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `json` - Output as JSON
/// * `file` - Optional file path to show details for
/// * `tag` - Optional tag to filter documents by
/// * `max_notes` - Maximum number of notes to display
/// * `depth` - Traversal depth (currently unused, reserved for future)
pub fn run_docs(
    ctx: &CliContext,
    json: bool,
    file: Option<&str>,
    tag: Option<&str>,
    max_notes: usize,
) -> Result<()> {
    let root = ctx.resolve_root()?;

    let docs_dir = workspace::docs_dir(&root);
    let graph =
        DocGraph::load(&docs_dir).context("Docs graph not found. Run `graxus index` first.")?;

    if json {
        if let Some(f) = file {
            let node = graph
                .find_by_path(f)
                .with_context(|| format!("Document not found: {}", f))?;
            println!("{}", serde_json::to_string_pretty(node)?);
        } else {
            println!("{}", serde_json::to_string_pretty(&graph)?);
        }
        return Ok(());
    }

    println!("{}", "=== Docs Graph ===".green().bold());
    println!("  Nodes: {}", graph.nodes.len());
    println!("  Edges: {}", graph.edges.len());

    if let Some(f) = file {
        let node = graph
            .find_by_path(f)
            .with_context(|| format!("Document not found: {}", f))?;
        println!("\n{}", format!("Document: {}", node.title).cyan().bold());
        println!("  Path:    {}", node.path);
        println!("  ID:      {}", node.id);
        if !node.tags.is_empty() {
            println!("  Tags:    {}", node.tags.join(", "));
        }
        if !node.headings.is_empty() {
            println!("  Headings:");
            for h in &node.headings {
                let indent = "  ".repeat(h.level as usize);
                println!("    {}{} (line {})", indent, h.text, h.line);
            }
        }
        if !node.wiki_links.is_empty() {
            println!("  Wiki links:");
            for link in &node.wiki_links {
                let alias_str = link.alias.as_deref().unwrap_or("");
                if alias_str.is_empty() {
                    println!("    [[{}]]", link.target);
                } else {
                    println!("    [[{}|{}]]", link.target, alias_str);
                }
            }
        }
    } else {
        println!("\n{}", "Documents:".cyan().bold());
        let nodes: Vec<_> = if let Some(t) = tag {
            graph
                .nodes
                .iter()
                .filter(|n| n.tags.iter().any(|tag| tag == t))
                .take(max_notes)
                .collect()
        } else {
            graph.nodes.iter().take(max_notes).collect()
        };
        for node in &nodes {
            let tags_str = if node.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", node.tags.join(", "))
            };
            println!("  {}{}", node.path, tags_str);
        }
        let total = if tag.is_some() {
            graph
                .nodes
                .iter()
                .filter(|n| n.tags.iter().any(|t| t == tag.unwrap()))
                .count()
        } else {
            graph.nodes.len()
        };
        if total > max_notes {
            println!("  ... and {} more", total - max_notes);
        }
    }

    Ok(())
}

/// Show all wiki-links that point to a given file.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `file` - File path to find backlinks for
/// * `max_notes` - Maximum number of backlinks to display
pub fn run_backlinks(ctx: &CliContext, file: &str, max_notes: usize) -> Result<()> {
    let root = ctx.resolve_root()?;

    let docs_dir = workspace::docs_dir(&root);
    let graph =
        DocGraph::load(&docs_dir).context("Docs graph not found. Run `graxus index` first.")?;

    let doc_id = if file.starts_with("doc:") {
        file.to_string()
    } else {
        format!("doc:{}", file)
    };

    let backlinks = graph.get_backlinks(&doc_id);

    println!(
        "{}",
        format!("=== Backlinks for {} ===", file).green().bold()
    );
    if backlinks.is_empty() {
        println!("  No backlinks found.");
    } else {
        for node in backlinks.iter().take(max_notes) {
            println!("  {} — {}", node.path, node.title);
        }
        if backlinks.len() > max_notes {
            println!("  ... and {} more", backlinks.len() - max_notes);
        }
    }

    Ok(())
}

/// List all tags used across documents.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `tag` - Optional tag to filter by
/// * `min_count` - Minimum number of documents with this tag
pub fn run_tags(ctx: &CliContext, tag: Option<&str>, min_count: usize) -> Result<()> {
    let root = ctx.resolve_root()?;

    let docs_dir = workspace::docs_dir(&root);
    let graph =
        DocGraph::load(&docs_dir).context("Docs graph not found. Run `graxus index` first.")?;

    // Count occurrences of each tag
    let mut tag_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for node in &graph.nodes {
        for t in &node.tags {
            *tag_counts.entry(t.clone()).or_insert(0) += 1;
        }
    }

    let mut tags: Vec<_> = tag_counts.into_iter().collect();
    tags.sort_by(|a, b| b.1.cmp(&a.1));

    println!("{}", "=== Tags ===".green().bold());
    let mut shown = 0;
    for (tag_name, count) in &tags {
        if let Some(filter) = tag {
            if tag_name != filter {
                continue;
            }
        }
        if *count >= min_count {
            println!("  #{} ({})", tag_name, count);
            shown += 1;
        }
    }
    if shown == 0 {
        println!("  No tags found.");
    } else {
        println!("\n  Total: {} tags", shown);
    }

    Ok(())
}

/// Export docs graph data in various formats.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `format` - Output format: "json", "csv", "markdown"
/// * `output` - Optional output file path (stdout if omitted)
/// * `save` - If true, save to .graxus/exports/ with auto-generated filename
pub fn run_export(ctx: &CliContext, format: &str, output: Option<&str>, save: bool) -> Result<()> {
    let root = ctx.resolve_root()?;

    let docs_dir = workspace::docs_dir(&root);
    let graph =
        DocGraph::load(&docs_dir).context("Docs graph not found. Run `graxus index` first.")?;

    let ext = match format {
        "json" => "json",
        "csv" => "csv",
        "markdown" | "md" => "md",
        _ => anyhow::bail!("Unknown format: {}. Use json, csv, or markdown", format),
    };

    let output_content = match format {
        "json" => serde_json::to_string_pretty(&graph)?,
        "csv" => graph_to_csv(&graph)?,
        "markdown" | "md" => graph_to_markdown(&graph)?,
        _ => unreachable!(),
    };

    // Determine save path: --output > --save > stdout
    let save_path = if let Some(path) = output {
        Some(path.to_string())
    } else if save {
        let exports_dir = root.join(".graxus").join("exports");
        std::fs::create_dir_all(&exports_dir)?;
        let filename = format!("graph.{}", ext);
        Some(exports_dir.join(&filename).to_string_lossy().to_string())
    } else {
        None
    };

    match save_path {
        Some(path) => {
            std::fs::write(&path, &output_content)?;
            println!("  {} {}", "Saved:".green(), path);
        }
        None => {
            println!("{}", output_content);
        }
    }

    Ok(())
}

fn graph_to_csv(graph: &DocGraph) -> Result<String> {
    let mut csv = String::new();
    csv.push_str("type,id,path,title,tags\n");
    for node in &graph.nodes {
        let tags = node.tags.join(";");
        csv.push_str(&format!(
            "node,{},{},{},{}\n",
            node.id, node.path, node.title, tags
        ));
    }
    csv.push_str("\ntype,from,to,edge_type\n");
    for edge in &graph.edges {
        csv.push_str(&format!(
            "edge,{},{},{:?}\n",
            edge.from, edge.to, edge.edge_type
        ));
    }
    Ok(csv)
}

fn graph_to_markdown(graph: &DocGraph) -> Result<String> {
    let mut md = String::new();
    md.push_str("# Documentation Graph\n\n");
    md.push_str(&format!(
        "**Nodes:** {} | **Edges:** {}\n\n",
        graph.nodes.len(),
        graph.edges.len()
    ));

    md.push_str("## Documents\n\n");
    for node in &graph.nodes {
        let tags = if node.tags.is_empty() {
            String::new()
        } else {
            format!(" `{}`", node.tags.join("`, `"))
        };
        md.push_str(&format!("- **{}** — `{}`{}\n", node.title, node.path, tags));
    }
    md.push('\n');

    if !graph.edges.is_empty() {
        md.push_str("## Edges\n\n");
        md.push_str("| From | To | Type |\n");
        md.push_str("|------|----|------|\n");
        for edge in &graph.edges {
            md.push_str(&format!(
                "| {} | {} | {:?} |\n",
                edge.from, edge.to, edge.edge_type
            ));
        }
    }

    Ok(md)
}

use anyhow::{Context, Result};
use colored::Colorize;
use std::env;
use std::path::Path;

use graxus_core::workspace;
use graxus_docgraph::graph::DocGraph;

pub fn run_docs(json: bool, file: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let docs_dir = workspace::docs_dir(&root);
    let graph = DocGraph::load(&docs_dir)
        .context("Docs graph not found. Run `graxus index` first.")?;

    if json {
        if let Some(f) = file {
            let node = graph.find_by_path(f)
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
        let node = graph.find_by_path(f)
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
        for node in &graph.nodes {
            let tags_str = if node.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", node.tags.join(", "))
            };
            println!("  {}{}", node.path, tags_str);
        }
    }

    Ok(())
}

pub fn run_backlinks(file: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let docs_dir = workspace::docs_dir(&root);
    let graph = DocGraph::load(&docs_dir)
        .context("Docs graph not found. Run `graxus index` first.")?;

    let doc_id = if file.starts_with("doc:") {
        file.to_string()
    } else {
        format!("doc:{}", file)
    };

    let backlinks = graph.get_backlinks(&doc_id);

    println!("{}", format!("=== Backlinks for {} ===", file).green().bold());
    if backlinks.is_empty() {
        println!("  No backlinks found.");
    } else {
        for node in backlinks {
            println!("  {} — {}", node.path, node.title);
        }
    }

    Ok(())
}

pub fn run_tags() -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let docs_dir = workspace::docs_dir(&root);
    let graph = DocGraph::load(&docs_dir)
        .context("Docs graph not found. Run `graxus index` first.")?;

    let tags = graph.get_all_tags();

    println!("{}", "=== Tags ===".green().bold());
    if tags.is_empty() {
        println!("  No tags found.");
    } else {
        for tag in &tags {
            println!("  #{}", tag);
        }
    }
    println!("\n  Total: {} tags", tags.len());

    Ok(())
}

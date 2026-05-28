use anyhow::{Context, Result};
use colored::Colorize;
use graxus_core::workspace;
use graxus_docgraph::graph::DocGraph;
use std::env;
use std::path::Path;

fn load_doc_graph(root: &Path) -> Result<DocGraph> {
    let docs_dir = workspace::docs_dir(root);
    DocGraph::load(&docs_dir).context("Docs graph not found. Run `graxus index` first.")
}

fn load_codemap(root: &Path) -> Result<serde_json::Value> {
    let codemap_path = workspace::code_dir(root).join("codemap.json");
    let content = std::fs::read_to_string(&codemap_path)
        .context("Codemap not found. Run `graxus index` first.")?;
    serde_json::from_str(&content).context("Failed to parse codemap")
}

fn write_html(output_dir: &Path, filename: &str, html: &str) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;
    let path = output_dir.join(filename);
    std::fs::write(&path, html)?;
    println!("  {} {}", "Written:".green(), path.display());
    Ok(())
}

fn viz_dir<'a>(output: Option<&'a str>, root: &'a Path) -> std::path::PathBuf {
    match output {
        Some(o) => Path::new(o).to_path_buf(),
        None => root.join(".graxus").join("viz"),
    }
}

pub fn run_docs(output: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let graph = load_doc_graph(&root)?;
    let d3 = graxus_viz::serializer::doc_graph_to_d3(&graph);
    let html = graxus_viz::template::render_html(&d3);
    let out = viz_dir(output, &root);
    write_html(&out, "docs.html", &html)?;
    println!("\n{}", "Docs visualization generated.".green().bold());
    Ok(())
}

pub fn run_codemap(output: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let graph = load_codemap(&root)?;
    let d3 = graxus_viz::serializer::code_graph_to_d3(&graph);
    let html = graxus_viz::template::render_html(&d3);
    let out = viz_dir(output, &root);
    write_html(&out, "codemap.html", &html)?;
    println!("\n{}", "Codemap visualization generated.".green().bold());
    Ok(())
}

pub fn run_callgraph(output: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let graph = load_codemap(&root)?;
    let d3 = graxus_viz::serializer::code_graph_to_d3(&graph);
    let html = graxus_viz::template::render_html(&d3);
    let out = viz_dir(output, &root);
    write_html(&out, "callgraph.html", &html)?;
    println!("\n{}", "Call graph visualization generated.".green().bold());
    Ok(())
}

pub fn run_impact(target: &str, output: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let graph = load_codemap(&root)?;
    let d3 = graxus_viz::serializer::blast_radius_to_d3(&graph, target, 3);
    let html = graxus_viz::template::render_html(&d3);
    let out = viz_dir(output, &root);
    write_html(&out, &format!("impact_{}.html", target), &html)?;
    println!("\n{}", format!("Impact visualization for '{}' generated.", target).green().bold());
    Ok(())
}

pub fn run_bridge(output: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let doc_graph = load_doc_graph(&root)?;
    let code_graph_json = load_codemap(&root)?;

    // Build bridge from doc graph only (path/symbol mentions in docs)
    let bridge = graxus_agent_api::BridgeBuilder::build(&doc_graph, &graxus_codemap::CodeGraph::default())?;
    let d3 = graxus_viz::serializer::bridge_to_d3(&doc_graph, &code_graph_json, &bridge);
    let html = graxus_viz::template::render_html(&d3);
    let out = viz_dir(output, &root);
    write_html(&out, "bridge.html", &html)?;
    println!("\n{}", "Bridge visualization generated.".green().bold());
    Ok(())
}

pub fn run_deps(output: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let graph = load_codemap(&root)?;
    let d3 = graxus_viz::serializer::deps_to_d3(&graph);
    let html = graxus_viz::template::render_html(&d3);
    let out = viz_dir(output, &root);
    write_html(&out, "deps.html", &html)?;
    println!("\n{}", "Dependency visualization generated.".green().bold());
    Ok(())
}

pub fn run_all(output: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let root = workspace::find_root(Path::new(&cwd))
        .context("Not a graxus project. Run `graxus init` first.")?;

    let out = viz_dir(output, &root);

    println!("{}", "=== Generating Visualizations ===".green().bold());

    let doc_graph = load_doc_graph(&root)?;
    let d3 = graxus_viz::serializer::doc_graph_to_d3(&doc_graph);
    write_html(&out, "docs.html", &graxus_viz::template::render_html(&d3))?;

    let code_graph = load_codemap(&root)?;
    let d3 = graxus_viz::serializer::code_graph_to_d3(&code_graph);
    write_html(&out, "codemap.html", &graxus_viz::template::render_html(&d3))?;

    let bridge = graxus_agent_api::BridgeBuilder::build(&doc_graph, &graxus_codemap::CodeGraph::default())?;
    let d3 = graxus_viz::serializer::bridge_to_d3(&doc_graph, &code_graph, &bridge);
    write_html(&out, "bridge.html", &graxus_viz::template::render_html(&d3))?;

    let d3 = graxus_viz::serializer::deps_to_d3(&code_graph);
    write_html(&out, "deps.html", &graxus_viz::template::render_html(&d3))?;

    println!("\n{}", "All visualizations generated.".green().bold());
    println!("  Output: {}", out.display());
    println!("  Open in browser: {}/*.html", out.display());

    Ok(())
}

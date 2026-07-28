use anyhow::Result;
use colored::Colorize;

use graxus_core::workspaces;

use crate::context::CliContext;

/// Show project status including file counts and configuration.
///
/// # Arguments
/// * `ctx` - Shared CLI runtime context (global args)
/// * `json` - Output as JSON
pub fn run(ctx: &CliContext, json: bool) -> Result<()> {
    let root = ctx.resolve_root()?;
    let config = ctx.load_config(&root)?;

    // Count files in .graxus subdirs
    let graxus_dir = root.join(".graxus");
    let mut subdir_counts = serde_json::Map::new();
    for subdir in &["docs", "code", "snapshots", "logs", "reports"] {
        let dir = graxus_dir.join(subdir);
        let count = if dir.is_dir() {
            std::fs::read_dir(&dir)
                .map(|entries| entries.count())
                .unwrap_or(0)
        } else {
            0
        };
        subdir_counts.insert(subdir.to_string(), serde_json::json!(count));
    }

    let ws_info = workspaces::detect_workspace(&root);

    if json {
        let status = serde_json::json!({
            "name": config.project.name,
            "root": root.display().to_string(),
            "graxus_dir": graxus_dir.display().to_string(),
            "subdirs": subdir_counts,
            "config": {
                "docs_enabled": config.docs.enabled,
                "code_enabled": config.code.enabled,
                "code_parser": config.code.parser,
                "code_languages": config.code.languages,
                "index_storage": config.index.storage,
                "edit_snapshots": config.edit.create_snapshots,
                "edit_max_files": config.edit.max_files_per_operation,
            },
            "workspace": {
                "is_monorepo": ws_info.is_monorepo,
                "kind": format!("{:?}", ws_info.kind),
                "languages": ws_info.languages,
                "sub_projects": ws_info.sub_projects.len(),
            },
        });
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("{}", "Graxus Project Status".green().bold());
        println!("  Name:    {}", config.project.name);
        println!("  Root:    {}", root.display());
        println!("  .graxus: {}", graxus_dir.display());

        for (subdir, count) in &subdir_counts {
            println!(
                "  .graxus/{}: {} files",
                subdir,
                count.as_u64().unwrap_or(0)
            );
        }

        println!("\n{}", "Config:".green().bold());
        println!("  Docs enabled:     {}", config.docs.enabled);
        println!("  Code enabled:     {}", config.code.enabled);
        println!("  Code parser:      {}", config.code.parser);
        println!("  Code languages:   {}", config.code.languages.join(", "));
        println!("  Index storage:    {}", config.index.storage);
        println!("  Edit snapshots:   {}", config.edit.create_snapshots);
        println!(
            "  Edit max files:   {}",
            config.edit.max_files_per_operation
        );

        if ws_info.is_monorepo {
            println!("\n{}", "Workspace Status:".green().bold());
            println!("  Kind:       {:?}", ws_info.kind);
            println!("  Languages:  {}", ws_info.languages.join(", "));
            println!("  Sub-projects: {}", ws_info.sub_projects.len());
        }
    }

    Ok(())
}

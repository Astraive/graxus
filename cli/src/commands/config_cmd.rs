use anyhow::Result;
use colored::Colorize;

use crate::context::CliContext;

/// Set an API key for a provider and update the config.
///
/// # Arguments
/// * `provider` - Provider name (openai, anthropic, cohere, ollama)
/// * `key` - API key value
pub fn run_set_key(ctx: &CliContext, provider: &str, key: &str) -> Result<()> {
    let root = ctx.resolve_root()?;

    // Validate provider
    let valid_providers = ["openai", "anthropic", "cohere", "ollama"];
    if !valid_providers.contains(&provider) {
        anyhow::bail!(
            "Unknown provider '{}'. Valid providers: {}",
            provider,
            valid_providers.join(", ")
        );
    }

    // Write to .graxus/secrets.env
    let secrets_path = root.join(".graxus").join("secrets.env");
    let env_var = format!("GRAXUS_{}_KEY", provider.to_uppercase());

    let mut content = if secrets_path.exists() {
        std::fs::read_to_string(&secrets_path)?
    } else {
        String::new()
    };

    // Remove existing entry for this provider
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines: Vec<String> = Vec::new();
    for line in &lines {
        if !line.starts_with(&format!("{}=", env_var)) {
            new_lines.push(line.to_string());
        }
    }
    new_lines.push(format!("{}={}", env_var, key));
    content = new_lines.join("\n") + "\n";

    std::fs::create_dir_all(secrets_path.parent().unwrap())?;
    std::fs::write(&secrets_path, &content)?;

    // Restrict file permissions to owner-only (Unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&secrets_path, std::fs::Permissions::from_mode(0o600));
    }

    // Update graxus.yaml with api_key_env
    let config_path = root.join("graxus.yaml");
    let mut config = ctx.load_config(&root)?;

    match provider {
        "openai" | "cohere" => {
            config.embeddings.api_key_env = env_var.clone();
            config.llm.api_key_env = env_var.clone();
        }
        "anthropic" => {
            config.llm.api_key_env = env_var.clone();
        }
        "ollama" => {
            config.embeddings.api_key_env = String::new();
            config.llm.api_key_env = String::new();
        }
        _ => {}
    }

    config.save(&root)?;

    println!("{}", "API key saved successfully!".green().bold());
    println!("  Provider:    {}", provider);
    println!("  Env var:     {}", env_var);
    println!("  Secrets:     {}", secrets_path.display());
    println!("  Config:      {}", config_path.display());
    println!();
    println!("  To use, set the environment variable:");
    println!("    {}=***", env_var);

    Ok(())
}

/// Show the current project configuration.
pub fn run_show(ctx: &CliContext) -> Result<()> {
    let root = ctx.resolve_root()?;

    let config = ctx.load_config(&root)?;

    println!("{}", "=== Graxus Configuration ===".green().bold());
    println!();
    println!("  Project:     {}", config.project.name);
    println!("  Storage:     {}", config.index.storage);
    println!();
    println!("  {}", "Defaults:".cyan().bold());
    println!("    Depth:           {}", config.defaults.depth);
    println!("    Max depth:       {}", config.defaults.max_depth);
    println!("    K:               {}", config.defaults.k);
    println!("    Max files:       {}", config.defaults.max_files);
    println!("    Max symbols:     {}", config.defaults.max_symbols);
    println!("    Max notes:       {}", config.defaults.max_notes);
    println!("    Max nodes:       {}", config.defaults.max_nodes);
    println!("    Max edges:       {}", config.defaults.max_edges);
    println!("    Min confidence:  {}", config.defaults.min_confidence);
    println!("    Context budget:  {}", config.defaults.context_budget);
    println!(
        "    Max chars/file:  {}",
        config.defaults.max_chars_per_file
    );
    println!();
    println!("  {}", "Embeddings:".cyan().bold());
    println!("    Enabled:   {}", config.embeddings.enabled);
    println!("    Provider:  {}", config.embeddings.provider);
    println!("    Model:     {}", config.embeddings.model);
    println!(
        "    API key:   {}",
        if config.embeddings.api_key_env.is_empty() {
            "(not set)".to_string()
        } else {
            config.embeddings.api_key_env.clone()
        }
    );
    println!("    Dims:      {}", config.embeddings.dimensions);
    println!("    Batch:     {}", config.embeddings.batch_size);
    println!();
    println!("  {}", "LLM:".cyan().bold());
    println!("    Enabled:   {}", config.llm.enabled);
    println!("    Provider:  {}", config.llm.provider);
    println!("    Model:     {}", config.llm.model);
    println!(
        "    API key:   {}",
        if config.llm.api_key_env.is_empty() {
            "(not set)".to_string()
        } else {
            config.llm.api_key_env.clone()
        }
    );
    println!("    Max tokens: {}", config.llm.max_tokens);
    println!("    Temp:      {}", config.llm.temperature);
    println!("    Max cost:  ${:.2}", config.llm.max_cost_per_run);

    // Check if API key env var is set
    if !config.embeddings.api_key_env.is_empty() {
        let set = std::env::var(&config.embeddings.api_key_env).is_ok();
        println!();
        println!(
            "  Embeddings key: {}",
            if set {
                "✓ set".green().to_string()
            } else {
                "✗ not set".red().to_string()
            }
        );
    }
    if !config.llm.api_key_env.is_empty() {
        let set = std::env::var(&config.llm.api_key_env).is_ok();
        println!(
            "  LLM key:        {}",
            if set {
                "✓ set".green().to_string()
            } else {
                "✗ not set".red().to_string()
            }
        );
    }

    Ok(())
}

/// Update a config key in graxus.yaml.
///
/// # Arguments
/// * `key` - Config key path (e.g., "code.parser")
/// * `value` - New value (parsed as string, number, or boolean)
pub fn run_update(ctx: &CliContext, key: &str, value: &str) -> Result<()> {
    let root = ctx.resolve_root()?;

    let mut config = ctx.load_config(&root)?;

    match key {
        "depth" | "defaults.depth" => config.defaults.depth = parse_usize(value)?,
        "max_depth" | "defaults.max_depth" => config.defaults.max_depth = parse_usize(value)?,
        "k" | "defaults.k" => config.defaults.k = parse_usize(value)?,
        "max_files" | "defaults.max_files" => config.defaults.max_files = parse_usize(value)?,
        "max_symbols" | "defaults.max_symbols" => config.defaults.max_symbols = parse_usize(value)?,
        "max_notes" | "defaults.max_notes" => config.defaults.max_notes = parse_usize(value)?,
        "max_nodes" | "defaults.max_nodes" => config.defaults.max_nodes = parse_usize(value)?,
        "max_edges" | "defaults.max_edges" => config.defaults.max_edges = parse_usize(value)?,
        "min_confidence" | "defaults.min_confidence" => {
            config.defaults.min_confidence = parse_f64(value)?
        }
        "context_budget" | "defaults.context_budget" => {
            config.defaults.context_budget = parse_usize(value)?
        }
        "max_chars_per_file" => config.defaults.max_chars_per_file = parse_usize(value)?,
        "context.depth" => config.context.depth = Some(parse_usize(value)?),
        "context.budget" => config.context.budget = Some(parse_usize(value)?),
        "context.max_files" => config.context.max_files = Some(parse_usize(value)?),
        "context.max_symbols" => config.context.max_symbols = Some(parse_usize(value)?),
        "context.max_notes" => config.context.max_notes = Some(parse_usize(value)?),
        "search.k" => config.search.k = Some(parse_usize(value)?),
        "search.max_results" => config.search.max_results = Some(parse_usize(value)?),
        "search.min_score" => config.search.min_score = Some(parse_f64(value)?),
        "graph.depth" => config.graph.depth = Some(parse_usize(value)?),
        "graph.max_notes" => config.graph.max_notes = Some(parse_usize(value)?),
        "graph.max_nodes" => config.graph.max_nodes = Some(parse_usize(value)?),
        "codemap.depth" => config.codemap.depth = Some(parse_usize(value)?),
        "codemap.max_files" => config.codemap.max_files = Some(parse_usize(value)?),
        "codemap.max_symbols" => config.codemap.max_symbols = Some(parse_usize(value)?),
        "embeddings.provider" => config.embeddings.provider = value.to_string(),
        "embeddings.model" => config.embeddings.model = value.to_string(),
        "llm.provider" => config.llm.provider = value.to_string(),
        "llm.model" => config.llm.model = value.to_string(),
        _ => anyhow::bail!(
            "Unknown config key: '{}'. Use dot notation like 'context.depth'",
            key
        ),
    }

    config.save(&root)?;
    println!("{} {} = {}", "Updated:".green().bold(), key, value);

    Ok(())
}

fn parse_usize(value: &str) -> Result<usize> {
    value
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid number: '{}'", value))
}

fn parse_f64(value: &str) -> Result<f64> {
    value
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid number: '{}'", value))
}

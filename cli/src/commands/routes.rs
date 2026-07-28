use anyhow::Result;
use colored::Colorize;
use graxus_core::workspace;
use serde_json::Value;

use crate::context::CliContext;

fn load_codemap(ctx: &CliContext) -> Result<Value> {
    let root = ctx.resolve_root()?;
    let codemap_path = workspace::code_dir(&root).join("codemap.json");
    if !codemap_path.exists() {
        anyhow::bail!("Codemap not found. Run `graxus index` first.");
    }
    Ok(serde_json::from_str(&std::fs::read_to_string(
        codemap_path,
    )?)?)
}

pub fn run(
    ctx: &CliContext,
    framework: Option<&str>,
    lang: Option<&str>,
    limit: usize,
    json: bool,
) -> Result<()> {
    let codemap = match load_codemap(ctx) {
        Ok(codemap) => codemap,
        Err(err) => {
            println!("{}", format!("{err:#}").yellow());
            Value::Null
        }
    };
    if codemap.is_null() {
        return Ok(());
    }

    let mut routes: Vec<&Value> = codemap
        .get("routes")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default();

    routes.retain(|route| {
        let framework_ok = framework.is_none_or(|needle| {
            route
                .get("framework")
                .and_then(Value::as_str)
                .map(|value| value.eq_ignore_ascii_case(needle))
                .unwrap_or(false)
        });
        let lang_ok = lang.is_none_or(|needle| {
            route
                .get("language")
                .and_then(Value::as_str)
                .map(|value| value.eq_ignore_ascii_case(needle))
                .unwrap_or(false)
        });
        framework_ok && lang_ok
    });
    routes.truncate(limit);

    if json {
        println!("{}", serde_json::to_string_pretty(&routes)?);
        return Ok(());
    }

    println!("{}", "=== Routes ===".green().bold());
    if routes.is_empty() {
        println!("  No framework-native routes indexed yet.");
        return Ok(());
    }

    for route in routes {
        let method = route.get("method").and_then(Value::as_str).unwrap_or("*");
        let path = route.get("path").and_then(Value::as_str).unwrap_or("?");
        let handler = route.get("handler").and_then(Value::as_str).unwrap_or("?");
        let file = route.get("file").and_then(Value::as_str).unwrap_or("?");
        let framework_name = route
            .get("framework")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        println!(
            "  {} {} -> {} [{}] ({})",
            method.cyan(),
            path,
            handler,
            framework_name,
            file
        );
    }

    Ok(())
}

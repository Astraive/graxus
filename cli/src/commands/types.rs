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

pub fn run(ctx: &CliContext, name: Option<&str>, json: bool) -> Result<()> {
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

    let matches_name = |entry: &&Value| {
        name.is_none_or(|needle| {
            [
                "trait_or_interface",
                "implementing_type",
                "abstract_type",
                "concrete_type",
            ]
            .iter()
            .any(|field| {
                entry
                    .get(field)
                    .and_then(Value::as_str)
                    .map(|value| value.contains(needle))
                    .unwrap_or(false)
            })
        })
    };

    let type_impls: Vec<&Value> = codemap
        .get("type_impls")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter(matches_name).collect())
        .unwrap_or_default();
    let di_bindings: Vec<&Value> = codemap
        .get("di_bindings")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter(matches_name).collect())
        .unwrap_or_default();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "type_impls": type_impls,
                "di_bindings": di_bindings,
            }))?
        );
        return Ok(());
    }

    println!("{}", "=== Type Relationships ===".green().bold());
    if type_impls.is_empty() {
        println!("  No trait/interface implementation facts indexed yet.");
    } else {
        for item in type_impls {
            let implementing = item
                .get("implementing_type")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let contract = item
                .get("trait_or_interface")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let file = item.get("file").and_then(Value::as_str).unwrap_or("?");
            println!("  {} -> {} ({})", implementing.cyan(), contract, file);
        }
    }

    println!("\n{}", "=== DI Bindings ===".green().bold());
    if di_bindings.is_empty() {
        println!("  No DI bindings indexed yet.");
    } else {
        for item in di_bindings {
            let abstract_type = item
                .get("abstract_type")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let concrete_type = item
                .get("concrete_type")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let framework = item
                .get("framework")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            println!(
                "  {} -> {} [{}]",
                abstract_type.cyan(),
                concrete_type,
                framework
            );
        }
    }

    Ok(())
}

use anyhow::Result;
use regex::Regex;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct CompileCommandsConfig {
    pub include_paths: Vec<String>,
    pub defines: Vec<String>,
    pub file_count: usize,
}

#[derive(Deserialize)]
struct CompileCommand {
    command: Option<String>,
    arguments: Option<Vec<String>>,
}

pub fn parse(root: &Path) -> Result<Option<CompileCommandsConfig>> {
    let path = root.join("compile_commands.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let commands: Vec<CompileCommand> = serde_json::from_str(&content)?;

    let file_count = commands.len();
    let re_include = Regex::new(r"^-I(.+)$")?;
    let re_define = Regex::new(r"^-D(.+)$")?;

    let mut include_paths = Vec::new();
    let mut defines = Vec::new();

    for cmd in &commands {
        let parsed_args: Vec<String> = if let Some(args) = &cmd.arguments {
            args.clone()
        } else if let Some(cmd_str) = &cmd.command {
            cmd_str.split_whitespace().map(String::from).collect()
        } else {
            continue;
        };

        for arg in &parsed_args {
            if let Some(caps) = re_include.captures(arg) {
                let p = caps.get(1).unwrap().as_str().to_string();
                if !include_paths.contains(&p) {
                    include_paths.push(p);
                }
            } else if let Some(caps) = re_define.captures(arg) {
                let d = caps.get(1).unwrap().as_str().to_string();
                if !defines.contains(&d) {
                    defines.push(d);
                }
            }
        }
    }

    Ok(Some(CompileCommandsConfig {
        include_paths,
        defines,
        file_count,
    }))
}

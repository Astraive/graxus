mod cargo;
mod cmake;
mod compile_commands;
mod dotnet;
mod go;
mod npm;
mod python;
mod tsconfig;

use anyhow::Result;
use std::path::Path;

pub use cargo::CargoConfig;
pub use cmake::CMakeConfig;
pub use compile_commands::CompileCommandsConfig;
pub use dotnet::DotnetConfig;
pub use go::GoConfig;
pub use npm::NpmConfig;
pub use python::PythonConfig;
pub use tsconfig::TsConfig;

#[derive(Debug, Clone)]
pub enum BuildConfig {
    Cargo(CargoConfig),
    CMake(CMakeConfig),
    CompileCommands(CompileCommandsConfig),
    Dotnet(DotnetConfig),
    Go(GoConfig),
    Npm(NpmConfig),
    Python(PythonConfig),
    TsConfig(TsConfig),
}

pub fn detect_build_configs(root: &Path) -> Result<Vec<BuildConfig>> {
    let mut configs = Vec::new();

    if let Some(cfg) = cargo::parse(root)? {
        configs.push(BuildConfig::Cargo(cfg));
    }
    if let Some(cfg) = npm::parse(root)? {
        configs.push(BuildConfig::Npm(cfg));
    }
    if let Some(cfg) = tsconfig::parse(root)? {
        configs.push(BuildConfig::TsConfig(cfg));
    }
    if let Some(cfg) = python::parse(root)? {
        configs.push(BuildConfig::Python(cfg));
    }
    if let Some(cfg) = go::parse(root)? {
        configs.push(BuildConfig::Go(cfg));
    }
    if let Some(cfg) = cmake::parse(root)? {
        configs.push(BuildConfig::CMake(cfg));
    }
    if let Some(cfg) = compile_commands::parse(root)? {
        configs.push(BuildConfig::CompileCommands(cfg));
    }
    if let Some(cfg) = dotnet::parse(root)? {
        configs.push(BuildConfig::Dotnet(cfg));
    }

    Ok(configs)
}

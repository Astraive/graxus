//! Shared CLI runtime context built from the parsed [`GlobalArgs`].
//!
//! Every subcommand receives a [`CliContext`] reference. This is the single
//! point where the global flags (`--root`, `--config`, `--quiet`, `--verbose`,
//! `--no-color`, `--timeout`) become observable to command implementations,
//! instead of being parsed by clap and then dropped on the floor.
//!
//! Precedence for project-root discovery: `--root` flag > `GRAXUS_ROOT` env var
//! > upward search for `graxus.yaml` / `.graxus/` from the current directory.
//!
//! Precedence for config: `--config` flag > `graxus.yaml` at the resolved root
//! > built-in defaults (see [`graxus_core::config::GraxusConfig`]).

use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};

use graxus_core::config::GraxusConfig;
use graxus_core::workspace;

use crate::args::GlobalArgs;

/// Shared runtime context derived from global CLI flags.
#[derive(Debug, Clone)]
pub struct CliContext {
    /// `--root` override for the project root.
    pub root: Option<PathBuf>,
    /// `--config` override pointing at a `graxus.yaml`.
    pub config: Option<PathBuf>,
    /// `--quiet`: suppress non-essential output (progress bars, banners).
    pub quiet: bool,
    /// `--verbose`: enable debug-level tracing.
    pub verbose: bool,
    /// `--no-color`: disable colored output.
    pub no_color: bool,
    /// `--timeout`: soft per-operation deadline in seconds.
    pub timeout: Option<u64>,
}

impl CliContext {
    /// Build a context from the parsed global args, applying color control as
    /// a side effect (so all downstream `colored` calls honor `--no-color`).
    pub fn from_global(global: &GlobalArgs) -> Self {
        // Respect `--no-color`, `NO_COLOR`, and `CLICOLOR=0` consistently.
        let color_disabled = global.no_color
            || env::var_os("NO_COLOR").is_some()
            || env::var("CLICOLOR").ok().as_deref() == Some("0");
        colored::control::set_override(!color_disabled);

        Self {
            root: global.root.clone(),
            config: global.config.clone(),
            quiet: global.quiet,
            verbose: global.verbose,
            no_color: color_disabled,
            timeout: global.timeout,
        }
    }

    /// Resolve the project root directory.
    ///
    /// `--root` wins; otherwise the env var `GRAXUS_ROOT` is consulted; finally
    /// we walk upward from the current directory looking for `graxus.yaml` or
    /// `.graxus/`.
    pub fn resolve_root(&self) -> Result<PathBuf> {
        if let Some(root) = &self.root {
            // Normalize to an absolute path so relative scan results are stable.
            let absolute = if root.is_absolute() {
                root.clone()
            } else {
                env::current_dir()?.join(root)
            };
            return Ok(absolute);
        }
        if let Ok(env_root) = env::var("GRAXUS_ROOT") {
            let p = PathBuf::from(env_root);
            let absolute = if p.is_absolute() {
                p
            } else {
                env::current_dir()?.join(p)
            };
            return Ok(absolute);
        }
        let cwd = env::current_dir()?;
        workspace::find_root(&cwd).context("Not a graxus project. Run `graxus init` first.")
    }

    /// Load the project configuration.
    ///
    /// If `--config` points at a `graxus.yaml`, load that file directly. The
    /// config's project root is still resolved separately via [`resolve_root`].
    pub fn load_config(&self, root: &Path) -> Result<GraxusConfig> {
        if let Some(config_path) = &self.config {
            let contents = std::fs::read_to_string(config_path)
                .with_context(|| format!("Failed to read config: {}", config_path.display()))?;
            let mut config: GraxusConfig = serde_yaml::from_str(&contents)
                .with_context(|| format!("Failed to parse config: {}", config_path.display()))?;
            config.apply_env_overrides();
            return Ok(config);
        }
        GraxusConfig::load(root)
    }

    /// Whether progress indicators (spinners, progress bars) should be shown.
    pub fn show_progress(&self) -> bool {
        !self.quiet
    }

    /// Begin a soft deadline for long-running operations.
    ///
    /// Returns `None` when `--timeout` is not set. Commands can poll
    /// [`Deadline::expired`] inside their work loops.
    pub fn deadline(&self) -> Option<Deadline> {
        self.timeout.map(|secs| Deadline {
            limit: Duration::from_secs(secs),
            start: Instant::now(),
        })
    }

    /// Convenience: returns a friendly "not implemented" error for global flags
    /// a command does not yet honor. Centralized so the message is consistent.
    pub fn unimplemented(&self, flag: &str) -> anyhow::Error {
        anyhow!("global flag `{flag}` is not yet implemented for this command")
    }
}

/// A soft per-operation deadline derived from `--timeout`.
pub struct Deadline {
    limit: Duration,
    start: Instant,
}

impl Deadline {
    /// Returns `true` if the deadline has elapsed.
    pub fn expired(&self) -> bool {
        self.start.elapsed() >= self.limit
    }

    /// Returns an error if the deadline has elapsed.
    pub fn check(&self) -> Result<()> {
        if self.expired() {
            Err(anyhow!(
                "operation timed out after {}s (--timeout)",
                self.limit.as_secs()
            ))
        } else {
            Ok(())
        }
    }
}

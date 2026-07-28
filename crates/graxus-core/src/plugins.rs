use anyhow::{bail, Context};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Default timeout for plugin execution (30 seconds).
const DEFAULT_PLUGIN_TIMEOUT_SECS: u64 = 30;

/// Plugin manifest loaded from `manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub plugin_type: PluginType,
    pub entry_point: String,
    /// Minimum graxus version required to run this plugin.
    #[serde(default)]
    pub min_graxus_version: Option<String>,
}

/// The category of a plugin (extractor, context provider, or exporter).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginType {
    Extractor,
    ContextProvider,
    Exporter,
}

/// Trait for in-process graxus plugins.
///
/// External plugins (subprocess-based) use [`PluginRegistry::execute_plugin`] instead.
pub trait GraxusPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn plugin_type(&self) -> PluginType;
    fn execute(&self, context: &PluginContext) -> anyhow::Result<PluginResult>;
}

/// Context passed to a plugin on execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginContext {
    /// Absolute path to the project root.
    pub project_root: PathBuf,
    /// Serialized graxus config (YAML string).
    pub config: serde_json::Value,
    /// Path to the current codemap file (may not exist).
    pub codemap_path: Option<PathBuf>,
    /// Path to the current docgraph file (may not exist).
    pub docgraph_path: Option<PathBuf>,
}

/// Result returned by a plugin after execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResult {
    /// Whether the plugin ran successfully.
    pub success: bool,
    /// Output data produced by the plugin (arbitrary JSON).
    pub output: serde_json::Value,
    /// List of files the plugin modified (relative to project root).
    #[serde(default)]
    pub files_modified: Vec<String>,
    /// Human-readable status message.
    #[serde(default)]
    pub message: String,
}

/// Plugin registry that discovers, installs, and executes plugins.
pub struct PluginRegistry {
    plugins: Vec<PluginManifest>,
    plugin_dir: PathBuf,
    /// Current graxus version for compatibility checks.
    graxus_version: Version,
}

impl PluginRegistry {
    /// Create a new plugin registry for the given directory.
    pub fn new(plugin_dir: PathBuf) -> Self {
        let graxus_version =
            Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 4, 0));
        Self {
            plugins: Vec::new(),
            plugin_dir,
            graxus_version,
        }
    }

    /// Scan plugin directory for manifests.
    pub fn discover(&mut self) -> anyhow::Result<()> {
        if !self.plugin_dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&self.plugin_dir)? {
            let entry = entry?;
            let manifest_path = entry.path().join("manifest.json");
            if manifest_path.exists() {
                let content = std::fs::read_to_string(&manifest_path)?;
                let manifest: PluginManifest = serde_json::from_str(&content)?;
                self.plugins.push(manifest);
            }
        }
        Ok(())
    }

    /// Return all discovered plugin manifests.
    pub fn list(&self) -> &[PluginManifest] {
        &self.plugins
    }

    /// Find a plugin by name.
    pub fn get(&self, name: &str) -> Option<&PluginManifest> {
        self.plugins.iter().find(|p| p.name == name)
    }

    /// Install a plugin from the given source directory.
    pub fn install(&mut self, source: &Path) -> anyhow::Result<()> {
        let manifest_path = source.join("manifest.json");
        if !manifest_path.exists() {
            bail!("No manifest.json found in {}", source.display());
        }
        let content = std::fs::read_to_string(&manifest_path)?;
        let manifest: PluginManifest = serde_json::from_str(&content)?;

        let dest = self.plugin_dir.join(&manifest.name);
        if dest.exists() {
            std::fs::remove_dir_all(&dest)?;
        }
        copy_dir_recursive(source, &dest)?;
        self.plugins.push(manifest);
        Ok(())
    }

    /// Remove a plugin by name.
    pub fn uninstall(&mut self, name: &str) -> anyhow::Result<()> {
        let dest = self.plugin_dir.join(name);
        if dest.exists() {
            std::fs::remove_dir_all(&dest)?;
        }
        self.plugins.retain(|p| p.name != name);
        Ok(())
    }

    /// Execute a plugin by name in a subprocess.
    ///
    /// Spawns the plugin's `entry_point` binary, passes `PluginContext` as JSON on stdin,
    /// and reads `PluginResult` as JSON from stdout.
    ///
    /// Timeout: 30 seconds by default.
    /// Environment variables set: GRAXUS_PROJECT_ROOT, GRAXUS_PLUGIN_NAME, GRAXUS_PLUGIN_VERSION.
    pub fn execute_plugin(
        &self,
        name: &str,
        context: &PluginContext,
    ) -> anyhow::Result<PluginResult> {
        let manifest = self
            .plugins
            .iter()
            .find(|p| p.name == name)
            .with_context(|| format!("Plugin '{}' not found in registry", name))?;

        // Version compatibility check
        self.check_version_compatibility(manifest)?;

        let plugin_dir = self.plugin_dir.join(name);
        let entry_path = plugin_dir.join(&manifest.entry_point);

        if !entry_path.exists() {
            bail!(
                "Entry point '{}' not found for plugin '{}'",
                manifest.entry_point,
                name
            );
        }

        // Validate that the entry point is within the plugin directory (prevent path traversal)
        if let (Ok(canonical_entry), Ok(canonical_plugin_dir)) = (
            std::fs::canonicalize(&entry_path),
            std::fs::canonicalize(&plugin_dir),
        ) {
            if !canonical_entry.starts_with(&canonical_plugin_dir) {
                bail!(
                    "Entry point '{}' escapes plugin directory for plugin '{}'",
                    manifest.entry_point,
                    name
                );
            }
        }

        let context_json =
            serde_json::to_string(context).context("Failed to serialize PluginContext")?;

        let mut cmd = Command::new(&entry_path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("GRAXUS_PROJECT_ROOT", &context.project_root)
            .env("GRAXUS_PLUGIN_NAME", &manifest.name)
            .env("GRAXUS_PLUGIN_VERSION", &manifest.version);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Failed to spawn plugin process: {}", entry_path.display()))?;

        // Write context to stdin
        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(context_json.as_bytes())
                .context("Failed to write context to plugin stdin")?;
        }
        // Close stdin to signal end of input
        child.stdin.take();

        // Read stdout and stderr before waiting to prevent pipe buffer deadlocks.
        // If the child writes more than the pipe buffer (typically 4-64KB) and we
        // don't drain it, the child blocks on write and we block on wait_with_output.
        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        let stdout_data = std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut h) = stdout_handle {
                let _ = std::io::Read::read_to_end(&mut h, &mut buf);
            }
            buf
        });
        let stderr_data = std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut h) = stderr_handle {
                let _ = std::io::Read::read_to_end(&mut h, &mut buf);
            }
            buf
        });

        // Wait with timeout via polling.
        let timeout = Duration::from_secs(DEFAULT_PLUGIN_TIMEOUT_SECS);
        let start = std::time::Instant::now();

        loop {
            match child.try_wait() {
                Ok(Some(_status)) => break,
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        child.kill().ok();
                        bail!(
                            "Plugin '{}' timed out after {} seconds",
                            name,
                            DEFAULT_PLUGIN_TIMEOUT_SECS
                        );
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => bail!("Error waiting for plugin '{}': {}", name, e),
            }
        }

        let output = child
            .wait_with_output()
            .context("Failed to read plugin output")?;

        // Combine with drained pipe data (pipes were already drained above,
        // but wait_with_output may return empty if we took the handles)
        let stdout = if !output.stdout.is_empty() {
            output.stdout
        } else {
            stdout_data.join().unwrap_or_default()
        };
        let stderr = if !output.stderr.is_empty() {
            output.stderr
        } else {
            stderr_data.join().unwrap_or_default()
        };

        if !output.status.success() {
            let stderr_str = String::from_utf8_lossy(&stderr);
            bail!(
                "Plugin '{}' exited with status {}: {}",
                name,
                output.status,
                stderr_str.trim()
            );
        }

        let stdout_str =
            String::from_utf8(stdout).context("Plugin output is not valid UTF-8")?;

        let result: PluginResult = serde_json::from_str(stdout_str.trim())
            .context("Failed to parse plugin output as PluginResult JSON")?;

        Ok(result)
    }

    /// Check that the plugin's minimum graxus version is satisfied.
    fn check_version_compatibility(&self, manifest: &PluginManifest) -> anyhow::Result<()> {
        if let Some(ref min_str) = manifest.min_graxus_version {
            let min_version = Version::parse(min_str).with_context(|| {
                format!(
                    "Invalid min_graxus_version '{}' in plugin '{}'",
                    min_str, manifest.name
                )
            })?;
            if self.graxus_version < min_version {
                bail!(
                    "Plugin '{}' requires graxus >= {} but current version is {}",
                    manifest.name,
                    min_version,
                    self.graxus_version
                );
            }
        }
        Ok(())
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dest = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;

    /// Helper to write a manifest file and return the dir.
    fn setup_plugin_dir(
        name: &str,
        entry_point: &str,
        min_graxus_version: Option<&str>,
    ) -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        let plugin_dir = dir.path().join(name);
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let min_ver_field = match min_graxus_version {
            Some(v) => format!(",\n  \"min_graxus_version\": \"{}\"", v),
            None => String::new(),
        };

        let manifest = format!(
            r#"{{
  "name": "{}",
  "version": "0.1.0",
  "description": "Test plugin",
  "plugin_type": "Extractor",
  "entry_point": "{}"
{}
}}"#,
            name, entry_point, min_ver_field
        );

        std::fs::write(plugin_dir.join("manifest.json"), manifest).unwrap();
        dir
    }

    #[test]
    fn test_discover_plugins() {
        let tmp = setup_plugin_dir("my-plugin", "my-plugin", None);
        let mut registry = PluginRegistry::new(tmp.path().to_path_buf());
        registry.discover().unwrap();

        assert_eq!(registry.list().len(), 1);
        assert_eq!(registry.list()[0].name, "my-plugin");
        assert_eq!(registry.list()[0].version, "0.1.0");
    }

    #[test]
    fn test_discover_empty_dir() {
        let tmp = tempdir().unwrap();
        let mut registry = PluginRegistry::new(tmp.path().to_path_buf());
        registry.discover().unwrap();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_discover_missing_dir() {
        let mut registry = PluginRegistry::new(PathBuf::from("/nonexistent/path"));
        registry.discover().unwrap();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_get_plugin() {
        let tmp = setup_plugin_dir("alpha", "alpha-bin", None);
        let mut registry = PluginRegistry::new(tmp.path().to_path_buf());
        registry.discover().unwrap();

        assert!(registry.get("alpha").is_some());
        assert!(registry.get("beta").is_none());
    }

    #[test]
    fn test_version_compatibility_pass() {
        let tmp = setup_plugin_dir("compat-ok", "compat-ok", Some("0.1.0"));
        let mut registry = PluginRegistry::new(tmp.path().to_path_buf());
        registry.discover().unwrap();

        // Current version is 0.4.0, so 0.1.0 minimum should pass
        let manifest = registry.get("compat-ok").unwrap();
        assert!(registry.check_version_compatibility(manifest).is_ok());
    }

    #[test]
    fn test_version_compatibility_fail() {
        let tmp = setup_plugin_dir("compat-fail", "compat-fail", Some("99.0.0"));
        let mut registry = PluginRegistry::new(tmp.path().to_path_buf());
        registry.discover().unwrap();

        let manifest = registry.get("compat-fail").unwrap();
        let err = registry.check_version_compatibility(manifest).unwrap_err();
        assert!(err.to_string().contains("requires graxus >= 99.0.0"));
    }

    #[test]
    fn test_version_compatibility_no_constraint() {
        let tmp = setup_plugin_dir("no-constraint", "no-constraint", None);
        let mut registry = PluginRegistry::new(tmp.path().to_path_buf());
        registry.discover().unwrap();

        let manifest = registry.get("no-constraint").unwrap();
        assert!(registry.check_version_compatibility(manifest).is_ok());
    }

    #[test]
    fn test_version_compatibility_invalid_semver() {
        let tmp = setup_plugin_dir("bad-ver", "bad-ver", Some("not-a-version"));
        let mut registry = PluginRegistry::new(tmp.path().to_path_buf());
        registry.discover().unwrap();

        let manifest = registry.get("bad-ver").unwrap();
        let err = registry.check_version_compatibility(manifest).unwrap_err();
        assert!(err.to_string().contains("Invalid min_graxus_version"));
    }

    #[test]
    fn test_execute_plugin_not_found() {
        let tmp = tempdir().unwrap();
        let registry = PluginRegistry::new(tmp.path().to_path_buf());
        let ctx = PluginContext {
            project_root: tmp.path().to_path_buf(),
            config: serde_json::Value::Null,
            codemap_path: None,
            docgraph_path: None,
        };
        let err = registry.execute_plugin("nonexistent", &ctx).unwrap_err();
        assert!(err.to_string().contains("not found in registry"));
    }

    #[test]
    fn test_execute_plugin_entry_point_missing() {
        let tmp = setup_plugin_dir("missing-ep", "missing-binary", None);
        let mut registry = PluginRegistry::new(tmp.path().to_path_buf());
        registry.discover().unwrap();

        let ctx = PluginContext {
            project_root: tmp.path().to_path_buf(),
            config: serde_json::Value::Null,
            codemap_path: None,
            docgraph_path: None,
        };
        let err = registry.execute_plugin("missing-ep", &ctx).unwrap_err();
        assert!(err.to_string().contains("Entry point"));
    }

    /// Write a Python script wrapped as a platform-appropriate executable.
    /// Returns the entry_point filename (e.g. "plugin.bat" on Windows, "plugin.sh" on Unix).
    fn write_python_plugin(plugin_dir: &Path, name: &str, python_code: &str) -> String {
        let script_path = plugin_dir.join(format!("{}.py", name));
        std::fs::write(&script_path, python_code).unwrap();

        if cfg!(target_os = "windows") {
            let bat_path = plugin_dir.join(format!("{}.bat", name));
            let bat_content = format!("@python \"{}\" %*", script_path.display());
            std::fs::write(&bat_path, bat_content).unwrap();
            format!("{}.bat", name)
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).ok();
            }
            format!("{}.py", name)
        }
    }

    #[test]
    fn test_execute_plugin_subprocess() {
        let tmp = tempdir().unwrap();
        let plugin_dir = tmp.path().join("echo-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let script = r#"import sys, json
ctx = json.load(sys.stdin)
result = {"success": True, "output": {"echo": True, "root": ctx.get("project_root","")}, "files_modified": [], "message": "ok"}
json.dump(result, sys.stdout)
"#;
        let ep = write_python_plugin(&plugin_dir, "main", script);

        let manifest = format!(
            r#"{{
  "name": "echo-plugin",
  "version": "0.1.0",
  "description": "Echo test plugin",
  "plugin_type": "Extractor",
  "entry_point": "{}"
}}"#,
            ep
        );
        std::fs::write(plugin_dir.join("manifest.json"), manifest).unwrap();

        let mut registry = PluginRegistry::new(tmp.path().to_path_buf());
        registry.discover().unwrap();

        let ctx = PluginContext {
            project_root: tmp.path().to_path_buf(),
            config: serde_json::Value::Null,
            codemap_path: None,
            docgraph_path: None,
        };

        let result = registry.execute_plugin("echo-plugin", &ctx).unwrap();
        assert!(result.success);
        assert_eq!(result.message, "ok");
        assert_eq!(result.output["echo"], serde_json::Value::Bool(true));
    }

    #[test]
    fn test_execute_plugin_timeout() {
        let tmp = tempdir().unwrap();
        let plugin_dir = tmp.path().join("slow-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let script = "import time\ntime.sleep(60)\n";
        let ep = write_python_plugin(&plugin_dir, "main", script);

        let manifest = format!(
            r#"{{
  "name": "slow-plugin",
  "version": "0.1.0",
  "description": "Slow test plugin",
  "plugin_type": "Extractor",
  "entry_point": "{}"
}}"#,
            ep
        );
        std::fs::write(plugin_dir.join("manifest.json"), manifest).unwrap();

        let mut registry = PluginRegistry::new(tmp.path().to_path_buf());
        registry.discover().unwrap();

        let ctx = PluginContext {
            project_root: tmp.path().to_path_buf(),
            config: serde_json::Value::Null,
            codemap_path: None,
            docgraph_path: None,
        };

        let err = registry.execute_plugin("slow-plugin", &ctx).unwrap_err();
        assert!(err.to_string().contains("timed out"));
    }

    #[test]
    fn test_execute_plugin_invalid_json_output() {
        let tmp = tempdir().unwrap();
        let plugin_dir = tmp.path().join("bad-output");
        std::fs::create_dir_all(&plugin_dir).unwrap();

        let script = "print('not json')\n";
        let ep = write_python_plugin(&plugin_dir, "main", script);

        let manifest = format!(
            r#"{{
  "name": "bad-output",
  "version": "0.1.0",
  "description": "Bad output plugin",
  "plugin_type": "Extractor",
  "entry_point": "{}"
}}"#,
            ep
        );
        std::fs::write(plugin_dir.join("manifest.json"), manifest).unwrap();

        let mut registry = PluginRegistry::new(tmp.path().to_path_buf());
        registry.discover().unwrap();

        let ctx = PluginContext {
            project_root: tmp.path().to_path_buf(),
            config: serde_json::Value::Null,
            codemap_path: None,
            docgraph_path: None,
        };

        let err = registry.execute_plugin("bad-output", &ctx).unwrap_err();
        assert!(err.to_string().contains("Failed to parse plugin output"));
    }

    #[test]
    fn test_install_and_uninstall() {
        let src = setup_plugin_dir("install-me", "install-me", None);
        let dest = tempdir().unwrap();

        let mut registry = PluginRegistry::new(dest.path().to_path_buf());
        // Install from the src directory (the child plugin dir inside src)
        let plugin_src = src.path().join("install-me");
        registry.install(&plugin_src).unwrap();

        assert_eq!(registry.list().len(), 1);
        assert!(dest.path().join("install-me/manifest.json").exists());

        registry.uninstall("install-me").unwrap();
        assert_eq!(registry.list().len(), 0);
        assert!(!dest.path().join("install-me").exists());
    }

    #[test]
    fn test_plugin_context_serialization() {
        let ctx = PluginContext {
            project_root: PathBuf::from("/test/project"),
            config: serde_json::json!({"project": {"name": "test"}}),
            codemap_path: Some(PathBuf::from("/test/.graxus/codemap.json")),
            docgraph_path: None,
        };

        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: PluginContext = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.project_root, PathBuf::from("/test/project"));
        assert!(deserialized.codemap_path.is_some());
        assert!(deserialized.docgraph_path.is_none());
    }

    #[test]
    fn test_plugin_result_serialization() {
        let result = PluginResult {
            success: true,
            output: serde_json::json!({"count": 42}),
            files_modified: vec!["src/main.rs".into()],
            message: "done".into(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: PluginResult = serde_json::from_str(&json).unwrap();
        assert!(deserialized.success);
        assert_eq!(deserialized.output["count"], 42);
        assert_eq!(deserialized.files_modified.len(), 1);
    }
}

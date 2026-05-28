use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Plugin manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub plugin_type: PluginType,
    pub entry_point: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginType {
    Extractor,
    ContextProvider,
    Exporter,
}

/// Plugin registry
pub struct PluginRegistry {
    plugins: Vec<PluginManifest>,
    plugin_dir: PathBuf,
}

impl PluginRegistry {
    pub fn new(plugin_dir: PathBuf) -> Self {
        Self {
            plugins: Vec::new(),
            plugin_dir,
        }
    }

    /// Scan plugin directory for manifests
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

    pub fn list(&self) -> &[PluginManifest] {
        &self.plugins
    }

    pub fn get(&self, name: &str) -> Option<&PluginManifest> {
        self.plugins.iter().find(|p| p.name == name)
    }

    pub fn install(&mut self, source: &Path) -> anyhow::Result<()> {
        let manifest_path = source.join("manifest.json");
        if !manifest_path.exists() {
            anyhow::bail!("No manifest.json found in {}", source.display());
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

    pub fn uninstall(&mut self, name: &str) -> anyhow::Result<()> {
        let dest = self.plugin_dir.join(name);
        if dest.exists() {
            std::fs::remove_dir_all(&dest)?;
        }
        self.plugins.retain(|p| p.name != name);
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

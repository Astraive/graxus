use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub name: String,
    pub root: PathBuf,
    pub kind: WorkspaceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkspaceKind {
    CargoWorkspace,
    NpmWorkspace,
    GoModule,
    Unknown,
}

/// Detect workspace structure from config files in the project root.
pub fn detect_workspaces(root: &Path) -> Vec<WorkspaceInfo> {
    let mut workspaces = Vec::new();

    // Check for Cargo workspace
    if root.join("Cargo.toml").exists() {
        if let Ok(content) = std::fs::read_to_string(root.join("Cargo.toml")) {
            if content.contains("[workspace]") {
                workspaces.push(WorkspaceInfo {
                    name: root
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    root: root.to_path_buf(),
                    kind: WorkspaceKind::CargoWorkspace,
                });
            }
        }
    }

    // Check for npm workspace
    if root.join("package.json").exists() {
        if let Ok(content) = std::fs::read_to_string(root.join("package.json")) {
            if content.contains("\"workspaces\"") {
                workspaces.push(WorkspaceInfo {
                    name: root
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    root: root.to_path_buf(),
                    kind: WorkspaceKind::NpmWorkspace,
                });
            }
        }
    }

    // Check for Go module
    if root.join("go.mod").exists() {
        workspaces.push(WorkspaceInfo {
            name: root
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            root: root.to_path_buf(),
            kind: WorkspaceKind::GoModule,
        });
    }

    // Recursively check subdirectories (max depth 2)
    detect_sub_workspaces(root, &mut workspaces, 0);

    workspaces
}

fn detect_sub_workspaces(dir: &Path, workspaces: &mut Vec<WorkspaceInfo>, depth: usize) {
    if depth >= 2 {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }

        // Check for nested Cargo workspace
        if path.join("Cargo.toml").exists() {
            if let Ok(content) = std::fs::read_to_string(path.join("Cargo.toml")) {
                if content.contains("[workspace]") {
                    workspaces.push(WorkspaceInfo {
                        name: name.clone(),
                        root: path.clone(),
                        kind: WorkspaceKind::CargoWorkspace,
                    });
                    continue;
                }
            }
        }

        // Check for nested npm workspace
        if path.join("package.json").exists() {
            if let Ok(content) = std::fs::read_to_string(path.join("package.json")) {
                if content.contains("\"workspaces\"") {
                    workspaces.push(WorkspaceInfo {
                        name: name.clone(),
                        root: path.clone(),
                        kind: WorkspaceKind::NpmWorkspace,
                    });
                    continue;
                }
            }
        }

        // Check for nested Go module
        if path.join("go.mod").exists() {
            workspaces.push(WorkspaceInfo {
                name: name.clone(),
                root: path.clone(),
                kind: WorkspaceKind::GoModule,
            });
            continue;
        }

        detect_sub_workspaces(&path, workspaces, depth + 1);
    }
}

/// Get which workspace a file belongs to, if any.
pub fn file_workspace<'a>(
    file_path: &str,
    workspaces: &'a [WorkspaceInfo],
) -> Option<&'a WorkspaceInfo> {
    workspaces
        .iter()
        .filter(|w| file_path.starts_with(&w.root.to_string_lossy().as_ref()))
        .max_by_key(|w| w.root.to_string_lossy().len())
}

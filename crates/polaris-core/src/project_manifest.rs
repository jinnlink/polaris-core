use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoveredProjectManifest {
    pub project_root: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: ProjectManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectManifest {
    pub schema_version: u32,
    pub project_id: String,
    pub title: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub default_pack: String,
    #[serde(default = "default_entry")]
    pub default_entry: String,
    #[serde(default)]
    pub entry: ProjectEntry,
    #[serde(default)]
    pub evidence: ProjectEvidence,
    #[serde(default)]
    pub ui: ProjectUi,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectEntry {
    #[serde(default = "default_start_label")]
    pub start_label: String,
    #[serde(default = "default_capture_label")]
    pub capture_label: String,
    #[serde(default = "default_stuck_label")]
    pub stuck_label: String,
    #[serde(default)]
    pub today_command: String,
}

impl Default for ProjectEntry {
    fn default() -> Self {
        Self {
            start_label: default_start_label(),
            capture_label: default_capture_label(),
            stuck_label: default_stuck_label(),
            today_command: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectEvidence {
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectUi {
    pub preferred_shell: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectManifestError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: String,
        source: toml::de::Error,
    },
    #[error("unsupported p-os.toml schema_version {found}; supported version is 1")]
    UnsupportedSchemaVersion { found: u32 },
    #[error("invalid p-os.toml field {field}: {reason}")]
    InvalidField {
        field: &'static str,
        reason: &'static str,
    },
}

pub fn discover_project_manifest(
    start: impl AsRef<Path>,
) -> Result<Option<DiscoveredProjectManifest>, ProjectManifestError> {
    let start = start.as_ref();
    let mut cursor = if start.is_file() {
        start.parent()
    } else {
        Some(start)
    };

    while let Some(dir) = cursor {
        let manifest_path = dir.join("p-os.toml");
        if manifest_path.is_file() {
            let manifest = load_project_manifest(&manifest_path)?;
            return Ok(Some(DiscoveredProjectManifest {
                project_root: dir.to_path_buf(),
                manifest_path,
                manifest,
            }));
        }
        cursor = dir.parent();
    }

    Ok(None)
}

pub fn discover_learning_projects(
    root: impl AsRef<Path>,
    max_depth: usize,
) -> Result<Vec<DiscoveredProjectManifest>, ProjectManifestError> {
    let root = root.as_ref();
    let mut projects = Vec::new();
    scan_learning_projects(root, root, 0, max_depth, &mut projects)?;
    projects.sort_by(|left, right| {
        left.project_root
            .cmp(&right.project_root)
            .then_with(|| left.manifest.project_id.cmp(&right.manifest.project_id))
    });
    Ok(projects)
}

pub fn load_project_manifest(
    path: impl AsRef<Path>,
) -> Result<ProjectManifest, ProjectManifestError> {
    let path = path.as_ref();
    let source = std::fs::read_to_string(path).map_err(|source| ProjectManifestError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let manifest: ProjectManifest =
        toml::from_str(&source).map_err(|source| ProjectManifestError::Parse {
            path: path.display().to_string(),
            source,
        })?;
    validate_manifest(manifest)
}

fn scan_learning_projects(
    root: &Path,
    dir: &Path,
    depth: usize,
    max_depth: usize,
    projects: &mut Vec<DiscoveredProjectManifest>,
) -> Result<(), ProjectManifestError> {
    if should_skip_scan_dir(root, dir) {
        return Ok(());
    }

    let manifest_path = dir.join("p-os.toml");
    if manifest_path.is_file() {
        let manifest = load_project_manifest(&manifest_path)?;
        projects.push(DiscoveredProjectManifest {
            project_root: dir.to_path_buf(),
            manifest_path,
            manifest,
        });
        return Ok(());
    }

    if depth >= max_depth {
        return Ok(());
    }

    let mut children = Vec::new();
    let read_dir = std::fs::read_dir(dir).map_err(|source| ProjectManifestError::Read {
        path: dir.display().to_string(),
        source,
    })?;
    for entry in read_dir {
        let entry = entry.map_err(|source| ProjectManifestError::Read {
            path: dir.display().to_string(),
            source,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source| ProjectManifestError::Read {
                path: entry.path().display().to_string(),
                source,
            })?;
        if file_type.is_dir() && !file_type.is_symlink() {
            children.push(entry.path());
        }
    }
    children.sort();

    for child in children {
        scan_learning_projects(root, &child, depth + 1, max_depth, projects)?;
    }
    Ok(())
}

fn should_skip_scan_dir(root: &Path, dir: &Path) -> bool {
    if dir == root {
        return false;
    }
    let Some(name) = dir.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    matches!(
        name,
        ".git"
            | ".hg"
            | ".svn"
            | ".agents"
            | ".claude"
            | ".cursor"
            | ".codewhale"
            | ".deepseek"
            | "_worktrees"
            | "_library"
            | "_local-runtimes"
            | "target"
            | "node_modules"
            | ".venv"
            | "venv"
    )
}

fn validate_manifest(manifest: ProjectManifest) -> Result<ProjectManifest, ProjectManifestError> {
    if manifest.schema_version != 1 {
        return Err(ProjectManifestError::UnsupportedSchemaVersion {
            found: manifest.schema_version,
        });
    }
    require_nonempty("project_id", &manifest.project_id)?;
    require_nonempty("title", &manifest.title)?;
    require_nonempty("kind", &manifest.kind)?;
    require_nonempty("default_pack", &manifest.default_pack)?;
    require_nonempty("default_entry", &manifest.default_entry)?;
    require_nonempty("entry.today_command", &manifest.entry.today_command)?;
    Ok(manifest)
}

fn require_nonempty(field: &'static str, value: &str) -> Result<(), ProjectManifestError> {
    if value.trim().is_empty() {
        return Err(ProjectManifestError::InvalidField {
            field,
            reason: "must not be empty",
        });
    }
    Ok(())
}

fn default_kind() -> String {
    "course".to_owned()
}

fn default_entry() -> String {
    "today".to_owned()
}

fn default_start_label() -> String {
    "继续今天".to_owned()
}

fn default_capture_label() -> String {
    "记录我刚学到的".to_owned()
}

fn default_stuck_label() -> String {
    "我卡住了".to_owned()
}

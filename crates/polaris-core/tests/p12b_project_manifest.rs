use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use polaris_core::project_manifest::{
    discover_learning_projects, discover_project_manifest, load_project_manifest,
    ProjectManifestError,
};

#[test]
fn discovers_nearest_p_os_manifest_by_walking_upward() {
    let root = TestDir::new("discover");
    write_manifest(
        root.path(),
        r#"
schema_version = 1
project_id = "rust-mastery-lab"
title = "Rust 与软件工程训练"
kind = "course"
default_pack = "rust"
default_entry = "today"

[entry]
start_label = "继续今天"
capture_label = "记录我刚学到的"
stuck_label = "我卡住了"
today_command = "cargo run -p labctl -- today --date {today}"

[evidence]
include = ["course/**", "exercises/**"]
ignore = ["target/**", ".git/**"]

[ui]
preferred_shell = "aura"
"#,
    );
    let nested = root.path().join("exercises/day01/src");
    fs::create_dir_all(&nested).unwrap();

    let discovered = discover_project_manifest(&nested).unwrap().unwrap();

    assert_eq!(discovered.manifest_path, root.path().join("p-os.toml"));
    assert_eq!(discovered.project_root, root.path());
    assert_eq!(discovered.manifest.project_id, "rust-mastery-lab");
    assert_eq!(discovered.manifest.default_pack, "rust");
    assert_eq!(discovered.manifest.default_entry, "today");
    assert_eq!(
        discovered.manifest.entry.today_command,
        "cargo run -p labctl -- today --date {today}"
    );
    assert_eq!(
        discovered.manifest.evidence.include,
        vec!["course/**", "exercises/**"]
    );
    assert_eq!(
        discovered.manifest.ui.preferred_shell.as_deref(),
        Some("aura")
    );
}

#[test]
fn project_manifest_requires_supported_schema_and_core_fields() {
    let root = TestDir::new("invalid");
    write_manifest(
        root.path(),
        r#"
schema_version = 2
project_id = ""
title = "Bad"
default_pack = "rust"
"#,
    );

    let err = load_project_manifest(root.path().join("p-os.toml")).unwrap_err();

    assert!(matches!(
        err,
        ProjectManifestError::UnsupportedSchemaVersion { found: 2 }
    ));
}

#[test]
fn discovery_returns_none_when_no_manifest_exists() {
    let root = TestDir::new("none");
    let nested = root.path().join("notes/day01");
    fs::create_dir_all(&nested).unwrap();

    let discovered = discover_project_manifest(&nested).unwrap();

    assert!(discovered.is_none());
}

#[test]
fn scans_learning_projects_under_root() {
    let root = TestDir::new("scan");
    let course = root.path().join("rust-mastery-lab");
    fs::create_dir_all(course.join("course/day01")).unwrap();
    write_manifest(&course, TEST_MANIFEST);

    let worktree = root.path().join("_worktrees/old-copy");
    fs::create_dir_all(&worktree).unwrap();
    write_manifest(
        &worktree,
        r#"
schema_version = 1
project_id = "ignored-worktree-copy"
title = "Ignored"
kind = "course"
default_pack = "rust"
default_entry = "today"

[entry]
today_command = "ignored"
"#,
    );

    let nested_fixture = course.join("engine/polaris-core/examples/project-manifests/fixture");
    fs::create_dir_all(&nested_fixture).unwrap();
    write_manifest(
        &nested_fixture,
        r#"
schema_version = 1
project_id = "ignored-nested-fixture"
title = "Ignored Nested"
kind = "course"
default_pack = "rust"
default_entry = "today"

[entry]
today_command = "ignored"
"#,
    );

    let projects = discover_learning_projects(root.path(), 3).unwrap();

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].project_root, course);
    assert_eq!(projects[0].manifest.project_id, "rust-mastery-lab");
}

fn write_manifest(root: &Path, content: &str) {
    fs::write(root.join("p-os.toml"), content.trim_start()).unwrap();
}

const TEST_MANIFEST: &str = r#"
schema_version = 1
project_id = "rust-mastery-lab"
title = "Rust 与软件工程训练"
kind = "course"
default_pack = "rust"
default_entry = "today"

[entry]
start_label = "继续今天"
capture_label = "记录我刚学到的"
stuck_label = "我卡住了"
today_command = "cargo run -p labctl -- today --date {today}"

[evidence]
include = ["course/**", "exercises/**"]
ignore = ["target/**", ".git/**"]

[ui]
preferred_shell = "aura"
"#;

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "polaris-p12b-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

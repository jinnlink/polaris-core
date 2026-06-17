use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use polaris_core::sandbox::{run_pack_sandbox, SandboxLearner, SandboxOptions, SandboxStatus};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn template_pack_sandbox_runs_without_deadlock() {
    let _lock = env_lock();
    let report = run_pack_sandbox(
        SandboxOptions::new(workspace_path("packs/template"))
            .with_learner(SandboxLearner::Mixed)
            .with_days(7),
    )
    .unwrap();

    assert_eq!(report.pack_id, "template");
    assert_eq!(report.pack_title, "Template Course Pack");
    assert_eq!(report.learner, SandboxLearner::Mixed);
    assert_eq!(report.days, 7);
    assert_eq!(report.theta_mode, "isolated");
    assert_eq!(report.validation.concept_count, 5);
    assert!(
        matches!(report.status, SandboxStatus::Pass | SandboxStatus::Warn),
        "{report:#?}"
    );
    assert!(report.deadlock_days.is_empty(), "{report:#?}");
    assert!(
        report.final_mean_p_known >= report.initial_mean_p_known,
        "{report:#?}"
    );
    assert!(!report.hmm_state_lock, "{report:#?}");
}

#[test]
fn strong_learner_sandbox_passes_template_pack() {
    let _lock = env_lock();
    let report = run_pack_sandbox(
        SandboxOptions::new(workspace_path("packs/template"))
            .with_learner(SandboxLearner::Strong)
            .with_days(7),
    )
    .unwrap();

    assert_eq!(report.status, SandboxStatus::Pass, "{report:#?}");
    assert!(report.final_mean_p_known > report.initial_mean_p_known);
    assert!(report.early_transfer_violations.is_empty(), "{report:#?}");
}

#[test]
fn sandbox_rejects_zero_day_runs() {
    let _lock = env_lock();
    let err = run_pack_sandbox(
        SandboxOptions::new(workspace_path("packs/template"))
            .with_learner(SandboxLearner::Mixed)
            .with_days(0),
    )
    .unwrap_err()
    .to_string();

    assert!(
        err.contains("sandbox.days"),
        "error should name the invalid parameter: {err}"
    );
}

#[test]
fn invalid_pack_fails_before_simulation() {
    let _lock = env_lock();
    let err = run_pack_sandbox(SandboxOptions::new(workspace_path("packs/does-not-exist")))
        .unwrap_err()
        .to_string();

    assert!(
        err.contains("pack") || err.contains("missing"),
        "error should come from pack validation/loading: {err}"
    );
}

#[test]
fn sandbox_restores_external_model_environment() {
    let _lock = env_lock();
    let _guard = EnvGuard::set(&[
        ("POLARIS_TIER0_ONLY", "0"),
        ("POLARIS_LLM_FAST_BASE_URL", "https://llm.invalid"),
        ("POLARIS_LLM_FAST_MODEL", "fast-model"),
        ("POLARIS_LLM_FAST_API_KEY", "fast-key"),
        ("POLARIS_EMBED_BASE_URL", "https://embed.invalid"),
        ("POLARIS_EMBED_MODEL", "embed-model"),
        ("POLARIS_EMBED_API_KEY", "embed-key"),
    ]);

    let report = run_pack_sandbox(
        SandboxOptions::new(workspace_path("packs/template"))
            .with_learner(SandboxLearner::Mixed)
            .with_days(1),
    )
    .unwrap();

    assert_eq!(report.mode, "sandbox");
    assert!(!report.llm_used);
    assert_eq!(std::env::var("POLARIS_TIER0_ONLY").unwrap(), "0");
    assert_eq!(
        std::env::var("POLARIS_LLM_FAST_BASE_URL").unwrap(),
        "https://llm.invalid"
    );
    assert_eq!(std::env::var("POLARIS_EMBED_API_KEY").unwrap(), "embed-key");
}

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(relative)
}

fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct EnvGuard {
    values: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    fn set(pairs: &[(&'static str, &'static str)]) -> Self {
        let values = pairs
            .iter()
            .map(|(key, value)| {
                let old = std::env::var(key).ok();
                std::env::set_var(key, value);
                (*key, old)
            })
            .collect();
        Self { values }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.values {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

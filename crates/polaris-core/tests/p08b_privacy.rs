use std::collections::BTreeSet;
use std::sync::{Mutex, MutexGuard};

use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::grader::LlmConfig;
use polaris_core::privacy::{tier0_only_enabled, PrivacyCallInventory};
use rusqlite::Connection;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn tier0_only_disables_llm_config_even_when_env_is_present() {
    let _lock = env_lock();
    let _guard = EnvGuard::set(&[
        ("POLARIS_TIER0_ONLY", "TrUe"),
        ("POLARIS_LLM_FAST_BASE_URL", "https://llm.invalid"),
        ("POLARIS_LLM_FAST_MODEL", "fast-model"),
        ("POLARIS_LLM_FAST_API_KEY", "secret"),
    ]);

    assert!(tier0_only_enabled());
    assert_eq!(LlmConfig::from_env(), LlmConfig::Unavailable);
}

#[test]
fn tier0_only_disables_embedding_refresh_even_when_env_is_present() {
    let _lock = env_lock();
    let _guard = EnvGuard::set(&[
        ("POLARIS_TIER0_ONLY", "1"),
        ("POLARIS_EMBED_BASE_URL", "https://embed.invalid"),
        ("POLARIS_EMBED_MODEL", "embed-model"),
        ("POLARIS_EMBED_API_KEY", "secret"),
    ]);
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();

    let summary = engine.refresh_missing_embeddings().unwrap();

    assert!(summary.disabled);
    assert_eq!(summary.refreshed, 0);
}

#[test]
fn privacy_inventory_contains_required_external_channels() {
    let ids = PrivacyCallInventory::all()
        .calls
        .iter()
        .map(|call| call.id)
        .collect::<BTreeSet<_>>();

    assert!(ids.contains("llm_grade_attempt"));
    assert!(ids.contains("llm_mirror_narrative"));
    assert!(ids.contains("embed_concept"));
}

#[test]
fn privacy_doc_ids_match_inventory() {
    let doc = include_str!("../../../docs/PRIVACY.md");
    let doc_ids = doc
        .lines()
        .filter_map(|line| line.trim().strip_prefix("id: "))
        .collect::<BTreeSet<_>>();
    let inventory_ids = PrivacyCallInventory::all()
        .calls
        .iter()
        .map(|call| call.id)
        .collect::<BTreeSet<_>>();

    assert_eq!(doc_ids, inventory_ids);
}

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}

struct EnvGuard {
    values: Vec<(&'static str, Option<String>)>,
}

fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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

use polaris_core::db::migrate;
use polaris_core::engine::Engine;
use polaris_core::mental_state::{forward_filter, HmmObservation, StatePosterior};
use polaris_core::report::MirrorReport;
use proptest::prelude::*;
use rusqlite::Connection;
use serde_json::json;

const GU_PATTERNS: [&str; 8] = [
    "overgeneralization",
    "boundary-blindness",
    "symbol-referent-confusion",
    "causal-inversion",
    "fluency-illusion",
    "procedural-conceptual-gap",
    "granularity-mismatch",
    "interference-confusion",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn p06c_gu_lifecycle_is_deterministic_under_attempt_insertion_order(
        pattern_idx in 0_usize..GU_PATTERNS.len(),
        stage in 0_u8..4,
    ) {
        let pattern = GU_PATTERNS[pattern_idx];

        let left = gu_lifecycle_snapshot(pattern, stage, false);
        let right = gu_lifecycle_snapshot(pattern, stage, true);

        prop_assert_eq!(&left, &right);
        assert_gu_stage(&left, stage)?;
    }

    #[test]
    fn p06c_mirror_report_stable_fields_are_deterministic_for_generated_phantoms(
        ownership_attempts in 3_usize..9,
        borrowing_attempts in 3_usize..9,
        seed_borrowing_first in any::<bool>(),
    ) {
        let engine = seeded_engine();
        if seed_borrowing_first {
            seed_phantom_concept(&engine, "borrowing", borrowing_attempts);
            seed_phantom_concept(&engine, "ownership", ownership_attempts);
        } else {
            seed_phantom_concept(&engine, "ownership", ownership_attempts);
            seed_phantom_concept(&engine, "borrowing", borrowing_attempts);
        }

        let first = engine.run_mirror_report().unwrap();
        let second = engine.run_mirror_report().unwrap();

        prop_assert_eq!(mirror_stable_fields(&first), mirror_stable_fields(&second));
        prop_assert_ne!(first.id, second.id);
    }

    #[test]
    fn p06c_hmm_filter_stays_finite_normalized_for_extreme_sequences(
        observations in proptest::collection::vec(arbitrary_hmm_observation(), 1..128),
    ) {
        let mut previous: Option<StatePosterior> = None;
        for observation in observations {
            let posterior = forward_filter(previous.as_ref(), observation);
            assert_valid_posterior(&posterior)?;

            let repeated = forward_filter(previous.as_ref(), observation);
            prop_assert_eq!(&posterior, &repeated);

            previous = Some(posterior);
        }
    }
}

fn assert_gu_stage(snapshots: &[GuRuleSnapshot], stage: u8) -> Result<(), TestCaseError> {
    prop_assert_eq!(snapshots.len(), 1, "expected one G_u rule: {:?}", snapshots);
    let snapshot = &snapshots[0];
    let expected_status = match stage {
        0 => "candidate",
        1 => "validated",
        2 => "active",
        _ => "resolved",
    };
    let expected_lifecycle = match stage {
        0 => vec!["candidate"],
        1 => vec!["candidate", "validated"],
        2 => vec!["candidate", "validated", "active"],
        _ => vec!["candidate", "validated", "active", "resolved"],
    }
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();

    prop_assert_eq!(snapshot.status.as_str(), expected_status, "{:?}", snapshot);
    prop_assert_eq!(
        &snapshot.lifecycle_statuses,
        &expected_lifecycle,
        "{:?}",
        snapshot
    );
    match stage {
        0 => {
            prop_assert_eq!(snapshot.alpha.as_str(), "1.000000", "{:?}", snapshot);
            prop_assert_eq!(snapshot.beta.as_str(), "1.000000", "{:?}", snapshot);
            prop_assert_eq!(snapshot.confusion_edges, 0, "{:?}", snapshot);
        }
        1 | 2 => {
            prop_assert_eq!(snapshot.alpha.as_str(), "4.000000", "{:?}", snapshot);
            prop_assert_eq!(snapshot.beta.as_str(), "1.000000", "{:?}", snapshot);
            prop_assert_eq!(snapshot.confusion_edges, 3, "{:?}", snapshot);
        }
        _ => {
            prop_assert_eq!(snapshot.alpha.as_str(), "1.000000", "{:?}", snapshot);
            prop_assert_eq!(snapshot.beta.as_str(), "1.000000", "{:?}", snapshot);
            prop_assert_eq!(snapshot.correct_streak, 3, "{:?}", snapshot);
            prop_assert_eq!(snapshot.confusion_edges, 3, "{:?}", snapshot);
        }
    }

    Ok(())
}

fn gu_lifecycle_snapshot(pattern: &str, stage: u8, reverse_insert: bool) -> Vec<GuRuleSnapshot> {
    let engine = seeded_engine();

    let initial = ["ownership", "borrowing", "lifetimes"]
        .iter()
        .enumerate()
        .map(|(idx, concept)| AttemptSeed {
            id: format!("{pattern}-fail-{idx}"),
            concept_id: (*concept).to_owned(),
            score: 0.20,
            pattern_tags: vec![pattern.to_owned()],
            created_at: format!("2026-06-01T00:0{idx}:00Z"),
        })
        .collect::<Vec<_>>();
    insert_attempts(&engine, initial, reverse_insert);
    engine.run_gu_induction().unwrap();

    if stage >= 1 {
        let mut holdout = ["ownership", "borrowing", "lifetimes"]
            .iter()
            .enumerate()
            .map(|(idx, concept)| AttemptSeed {
                id: format!("{pattern}-hit-{idx}"),
                concept_id: (*concept).to_owned(),
                score: 0.30,
                pattern_tags: vec![pattern.to_owned()],
                created_at: format!("2026-06-02T00:0{idx}:00Z"),
            })
            .collect::<Vec<_>>();
        holdout.extend(["traits", "modules", "closures"].iter().enumerate().map(
            |(idx, concept)| AttemptSeed {
                id: format!("{pattern}-baseline-{idx}"),
                concept_id: (*concept).to_owned(),
                score: 0.85,
                pattern_tags: Vec::new(),
                created_at: format!("2026-06-02T01:0{idx}:00Z"),
            },
        ));
        insert_attempts(&engine, holdout, reverse_insert);
        engine.run_gu_induction().unwrap();
    }

    if stage >= 2 {
        let active = engine.active_gu_rules_for_concept("ownership").unwrap();
        assert_eq!(active.len(), 1);
    }

    if stage >= 3 {
        let correct = (0..3)
            .map(|idx| AttemptSeed {
                id: format!("{pattern}-correct-{idx}"),
                concept_id: "ownership".to_owned(),
                score: 0.90,
                pattern_tags: Vec::new(),
                created_at: format!("2026-06-03T00:0{idx}:00Z"),
            })
            .collect::<Vec<_>>();
        insert_attempts(&engine, correct, reverse_insert);
        engine.run_gu_induction().unwrap();
    }

    gu_rule_snapshots(&engine)
}

#[derive(Debug, Clone)]
struct AttemptSeed {
    id: String,
    concept_id: String,
    score: f64,
    pattern_tags: Vec<String>,
    created_at: String,
}

fn insert_attempts(engine: &Engine, mut attempts: Vec<AttemptSeed>, reverse: bool) {
    if reverse {
        attempts.reverse();
    }
    for attempt in attempts {
        insert_graded_attempt(engine, attempt);
    }
}

fn insert_graded_attempt(engine: &Engine, attempt: AttemptSeed) {
    engine
        .conn()
        .execute(
            "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence,
                                  provisional_score, final_score, depth, grader_json, rating,
                                  created_at, graded_at)
             VALUES (?1, 's1', ?2, 'recall', 2, 0.30, ?3, 'recall', ?4, 'again', ?5, ?5)",
            (
                attempt.id,
                attempt.concept_id,
                attempt.score,
                json!({
                    "score": attempt.score,
                    "depth": "recall",
                    "pattern_tags": attempt.pattern_tags,
                    "citations": [],
                })
                .to_string(),
                attempt.created_at,
            ),
        )
        .unwrap();
}

#[derive(Debug, Clone, PartialEq)]
struct GuRuleSnapshot {
    id: String,
    pattern: String,
    status: String,
    concept_ids: Vec<String>,
    attempt_ids: Vec<String>,
    alpha: String,
    beta: String,
    correct_streak: i64,
    confusion_edges: i64,
    lifecycle_statuses: Vec<String>,
}

fn gu_rule_snapshots(engine: &Engine) -> Vec<GuRuleSnapshot> {
    let mut stmt = engine
        .conn()
        .prepare(
            "SELECT id, pattern, status, concept_ids_json, attempt_ids_json,
                    printf('%.6f', alpha), printf('%.6f', beta), COALESCE(correct_streak, 0)
             FROM gu_rules
             ORDER BY id ASC",
        )
        .unwrap();
    stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let concept_ids_json: String = row.get(3)?;
        let attempt_ids_json: String = row.get(4)?;
        let confusion_edges = engine.conn().query_row(
            "SELECT COUNT(*) FROM edges WHERE src=?1 AND type='confusion'",
            [format!("gu:{id}")],
            |edge_row| edge_row.get(0),
        )?;
        let lifecycle_statuses = lifecycle_statuses(engine, &id);
        Ok(GuRuleSnapshot {
            id,
            pattern: row.get(1)?,
            status: row.get(2)?,
            concept_ids: serde_json::from_str(&concept_ids_json).unwrap(),
            attempt_ids: serde_json::from_str(&attempt_ids_json).unwrap(),
            alpha: row.get(5)?,
            beta: row.get(6)?,
            correct_streak: row.get(7)?,
            confusion_edges,
            lifecycle_statuses,
        })
    })
    .unwrap()
    .collect::<rusqlite::Result<Vec<_>>>()
    .unwrap()
}

fn lifecycle_statuses(engine: &Engine, rule_id: &str) -> Vec<String> {
    let mut stmt = engine
        .conn()
        .prepare(
            "SELECT json_extract(payload_json, '$.status')
             FROM behavior_events
             WHERE type='gu_lifecycle'
               AND json_extract(payload_json, '$.rule_id')=?1
             ORDER BY rowid ASC",
        )
        .unwrap();
    stmt.query_map([rule_id], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

fn seed_phantom_concept(engine: &Engine, concept_id: &str, attempt_count: usize) {
    for idx in 0..attempt_count {
        engine
            .conn()
            .execute(
                "INSERT INTO attempts(id, session_id, concept_id, task_type, self_confidence,
                                      final_score, created_at, graded_at)
                 VALUES (?1, 's1', ?2, 'recall', 5, 0.2,
                         strftime('%Y-%m-%dT%H:%M:%SZ','now','-2 hours'),
                         strftime('%Y-%m-%dT%H:%M:%SZ','now','-2 hours'))",
                (format!("{concept_id}-p06c-phantom-{idx}"), concept_id),
            )
            .unwrap();
    }
    engine
        .conn()
        .execute(
            "INSERT OR REPLACE INTO mastery_states(concept_id, p_known, calib_gap, attempt_count, updated_at)
             VALUES (?1, 0.30, 0.40, ?2, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
            (concept_id, attempt_count as i64),
        )
        .unwrap();
}

#[derive(Debug, Clone, PartialEq)]
struct MirrorStableSnapshot {
    schema_version: i64,
    week: String,
    window_days: i64,
    assertions: Vec<ReportItemStable>,
    hypotheses: Vec<ReportItemStable>,
    suggestions: Vec<ReportItemStable>,
    skipped: Vec<(String, String, String)>,
    hazard_gate: (bool, String, Option<String>, String),
    reflection_prompts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct ReportItemStable {
    id: String,
    kind: String,
    subject: String,
    claim: String,
    confidence: String,
    evidence_ids: Vec<String>,
    stats_json: String,
}

fn mirror_stable_fields(report: &MirrorReport) -> MirrorStableSnapshot {
    MirrorStableSnapshot {
        schema_version: report.schema_version,
        week: report.week.clone(),
        window_days: report.window_days,
        assertions: stable_items(&report.assertions),
        hypotheses: stable_items(&report.hypotheses),
        suggestions: stable_items(&report.suggestions),
        skipped: report
            .skipped
            .iter()
            .map(|skip| (skip.id.clone(), skip.kind.clone(), skip.reason.clone()))
            .collect(),
        hazard_gate: (
            report.hazard_gate.participates,
            report.hazard_gate.reason.clone(),
            report
                .hazard_gate
                .validation_auc
                .map(|value| format!("{value:.9}")),
            format!("{:.9}", report.hazard_gate.auc_gate),
        ),
        reflection_prompts: report.reflection_prompts.clone(),
    }
}

fn stable_items(items: &[polaris_core::report::ReportItem]) -> Vec<ReportItemStable> {
    items
        .iter()
        .map(|item| ReportItemStable {
            id: item.id.clone(),
            kind: item.kind.clone(),
            subject: item.subject.clone(),
            claim: item.claim.clone(),
            confidence: format!("{:.9}", item.confidence),
            evidence_ids: item.evidence_ids.clone(),
            stats_json: serde_json::to_string(&item.stats).unwrap(),
        })
        .collect()
}

fn arbitrary_hmm_observation() -> impl Strategy<Value = HmmObservation> {
    (
        arbitrary_f64(),
        arbitrary_f64(),
        arbitrary_f64(),
        arbitrary_f64(),
        arbitrary_f64(),
        arbitrary_f64(),
        arbitrary_f64(),
    )
        .prop_map(
            |(
                z_latency,
                hints,
                residual,
                consec_fail,
                conf_delta,
                interval_bucket,
                session_min,
            )| HmmObservation {
                z_latency,
                hints,
                residual,
                consec_fail,
                conf_delta,
                interval_bucket,
                session_min,
            },
        )
}

fn arbitrary_f64() -> impl Strategy<Value = f64> {
    prop_oneof![
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(f64::NEG_INFINITY),
        Just(1.0e308),
        Just(-1.0e308),
        (-1_000_000_i64..=1_000_000).prop_map(|value| value as f64 / 10.0),
    ]
}

fn assert_valid_posterior(posterior: &StatePosterior) -> Result<(), TestCaseError> {
    let sum = posterior.probabilities.iter().sum::<f64>();
    prop_assert!(
        sum.is_finite(),
        "posterior sum is not finite: {posterior:?}"
    );
    prop_assert!(
        (sum - 1.0).abs() < 1e-9,
        "posterior sum {sum} is not normalized: {posterior:?}"
    );
    for probability in posterior.probabilities {
        prop_assert!(
            probability.is_finite(),
            "posterior contains non-finite value: {posterior:?}"
        );
        prop_assert!(
            (0.0..=1.0).contains(&probability),
            "posterior contains out-of-range value: {posterior:?}"
        );
    }
    Ok(())
}

fn seeded_engine() -> Engine {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    let mut engine = Engine::new(conn);
    engine.init_pack(workspace_pack_path("packs/rust")).unwrap();
    engine
}

fn workspace_pack_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}

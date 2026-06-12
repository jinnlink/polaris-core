use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension};

use crate::error::{PolarisError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterClass {
    A,
    B,
    C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuningRoute {
    Replay,
    Mrt,
    Manual,
    Fit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterSpec {
    pub key: &'static str,
    pub default_value: &'static str,
    pub class: ParameterClass,
    pub bounds: Option<&'static str>,
    pub tuning_route: TuningRoute,
}

pub fn default_registry() -> BTreeMap<&'static str, ParameterSpec> {
    [
        spec(
            "bkt.p_init",
            "0.20",
            ParameterClass::B,
            Some("[0.05,0.50]"),
            TuningRoute::Replay,
        ),
        spec(
            "bkt.slip",
            "0.10",
            ParameterClass::B,
            Some("[0.02,0.30]"),
            TuningRoute::Replay,
        ),
        spec(
            "bkt.guess",
            "0.20",
            ParameterClass::B,
            Some("[0.05,0.40]"),
            TuningRoute::Replay,
        ),
        spec(
            "bkt.guess_explain",
            "0.05",
            ParameterClass::B,
            Some("[0,0.20]"),
            TuningRoute::Replay,
        ),
        spec(
            "bkt.learn",
            "0.10",
            ParameterClass::B,
            Some("[0.02,0.40]"),
            TuningRoute::Replay,
        ),
        spec(
            "bkt.cut_hi",
            "0.75",
            ParameterClass::B,
            Some("[0.60,0.90]"),
            TuningRoute::Manual,
        ),
        spec(
            "bkt.cut_lo",
            "0.40",
            ParameterClass::B,
            Some("[0.20,0.50]"),
            TuningRoute::Manual,
        ),
        spec(
            "calib.ewma",
            "0.30",
            ParameterClass::B,
            Some("[0.10,0.50]"),
            TuningRoute::Replay,
        ),
        spec(
            "calib.phantom_gap",
            "0.25",
            ParameterClass::B,
            Some("[0.15,0.40]"),
            TuningRoute::Replay,
        ),
        spec(
            "calib.phantom_p",
            "0.60",
            ParameterClass::B,
            Some("[0.4,0.8]"),
            TuningRoute::Replay,
        ),
        spec(
            "calib.phantom_n",
            "2",
            ParameterClass::B,
            Some("[2,5]"),
            TuningRoute::Replay,
        ),
        spec(
            "sched.w_r",
            "0.40",
            ParameterClass::B,
            Some("simplex"),
            TuningRoute::Mrt,
        ),
        spec(
            "sched.w_cal",
            "0.30",
            ParameterClass::B,
            Some("simplex"),
            TuningRoute::Mrt,
        ),
        spec(
            "sched.w_mis",
            "0.20",
            ParameterClass::B,
            Some("simplex"),
            TuningRoute::Mrt,
        ),
        spec(
            "sched.w_new",
            "0.10",
            ParameterClass::B,
            Some("simplex"),
            TuningRoute::Mrt,
        ),
        spec(
            "sched.w_phase",
            "0.0",
            ParameterClass::B,
            Some("[0,1]"),
            TuningRoute::Mrt,
        ),
        spec(
            "sched.mis_window_days",
            "14",
            ParameterClass::B,
            Some("[7,30]"),
            TuningRoute::Replay,
        ),
        spec(
            "sched.prereq_p",
            "0.60",
            ParameterClass::B,
            Some("[0.4,0.8]"),
            TuningRoute::Mrt,
        ),
        spec(
            "grade.provisional_base",
            "0.10",
            ParameterClass::B,
            None,
            TuningRoute::Replay,
        ),
        spec(
            "grade.provisional_slope",
            "0.80",
            ParameterClass::B,
            None,
            TuningRoute::Replay,
        ),
        spec(
            "grade.quote_min",
            "8",
            ParameterClass::A,
            None,
            TuningRoute::Manual,
        ),
        spec(
            "grade.quote_max",
            "220",
            ParameterClass::A,
            None,
            TuningRoute::Manual,
        ),
        spec(
            "fsrs.r_again",
            "0.5",
            ParameterClass::B,
            Some("[0.4,0.6]"),
            TuningRoute::Mrt,
        ),
        spec(
            "fsrs.r_hard",
            "0.7",
            ParameterClass::B,
            Some("[0.6,0.8]"),
            TuningRoute::Mrt,
        ),
        spec(
            "fsrs.r_good",
            "0.9",
            ParameterClass::B,
            Some("[0.8,1.0]"),
            TuningRoute::Mrt,
        ),
        spec(
            "fsrs.w",
            "[0.4,0.6,2.4,5.8,4.93,0.94,0.86,0.01,1.49,0.14,0.94,2.18,0.05,0.34,1.26,0.29,2.61]",
            ParameterClass::C,
            None,
            TuningRoute::Fit,
        ),
        spec(
            "fsrs.request_retention",
            "0.9",
            ParameterClass::B,
            Some("[0.70,0.98]"),
            TuningRoute::Mrt,
        ),
        spec(
            "fsrs.maximum_interval",
            "365",
            ParameterClass::B,
            Some("[1,3650]"),
            TuningRoute::Manual,
        ),
        spec(
            "fsrs.min_interval",
            "1",
            ParameterClass::B,
            Some("[1,30]"),
            TuningRoute::Manual,
        ),
        spec(
            "fsrs.initial_stability",
            "1.0",
            ParameterClass::B,
            Some("[0.1,10.0]"),
            TuningRoute::Replay,
        ),
        spec(
            "fsrs.initial_difficulty",
            "5.0",
            ParameterClass::B,
            Some("[1.0,10.0]"),
            TuningRoute::Replay,
        ),
        spec(
            "latent.k",
            "32",
            ParameterClass::B,
            Some("[8,64]"),
            TuningRoute::Manual,
        ),
        spec(
            "latent.k_max",
            "64",
            ParameterClass::A,
            None,
            TuningRoute::Manual,
        ),
        spec(
            "graph.struct_threshold",
            "0.40",
            ParameterClass::A,
            None,
            TuningRoute::Manual,
        ),
        spec(
            "geometry.hnsw_m",
            "16",
            ParameterClass::B,
            Some("[4,64]"),
            TuningRoute::Manual,
        ),
        spec(
            "geometry.ef_search",
            "64",
            ParameterClass::B,
            Some("[8,512]"),
            TuningRoute::Manual,
        ),
        spec(
            "embedding.dim",
            "0",
            ParameterClass::C,
            Some("[0,8192]"),
            TuningRoute::Fit,
        ),
        spec(
            "mirt.eta",
            "0.05",
            ParameterClass::B,
            Some("[0.01,0.2]"),
            TuningRoute::Replay,
        ),
        spec(
            "mirt.step_cap",
            "0.05",
            ParameterClass::B,
            Some("[0.01,0.1]"),
            TuningRoute::Replay,
        ),
        spec(
            "mirt.shrink",
            "1e-4",
            ParameterClass::B,
            None,
            TuningRoute::Replay,
        ),
        spec(
            "mirt.fuse_n0",
            "5",
            ParameterClass::B,
            Some("[2,20]"),
            TuningRoute::Replay,
        ),
        spec(
            "mirt.d.recall",
            "-0.30",
            ParameterClass::B,
            Some("[-1,+1]"),
            TuningRoute::Replay,
        ),
        spec(
            "mirt.d.choose",
            "-0.15",
            ParameterClass::B,
            Some("[-1,+1]"),
            TuningRoute::Replay,
        ),
        spec(
            "mirt.d.cloze",
            "0.00",
            ParameterClass::B,
            Some("[-1,+1]"),
            TuningRoute::Replay,
        ),
        spec(
            "mirt.d.rewrite",
            "0.15",
            ParameterClass::B,
            Some("[-1,+1]"),
            TuningRoute::Replay,
        ),
        spec(
            "mirt.d.apply",
            "0.30",
            ParameterClass::B,
            Some("[-1,+1]"),
            TuningRoute::Replay,
        ),
        spec(
            "mirt.d.translate",
            "0.30",
            ParameterClass::B,
            Some("[-1,+1]"),
            TuningRoute::Replay,
        ),
        spec(
            "mirt.d.transfer",
            "0.50",
            ParameterClass::B,
            Some("[-1,+1]"),
            TuningRoute::Replay,
        ),
        spec(
            "mirt.d.free_produce",
            "0.50",
            ParameterClass::B,
            Some("[-1,+1]"),
            TuningRoute::Replay,
        ),
        spec(
            "mirt.a_c",
            "1.0",
            ParameterClass::A,
            None,
            TuningRoute::Manual,
        ),
        spec(
            "consol.accept_margin",
            "0.01",
            ParameterClass::A,
            None,
            TuningRoute::Manual,
        ),
        spec(
            "consol.holdout_frac",
            "0.20",
            ParameterClass::A,
            None,
            TuningRoute::Manual,
        ),
        spec(
            "hmm.em_min_n",
            "200",
            ParameterClass::B,
            Some("[100,500]"),
            TuningRoute::Manual,
        ),
        spec(
            "hmm.gate_auc_margin",
            "0.03",
            ParameterClass::A,
            None,
            TuningRoute::Manual,
        ),
        spec(
            "hazard.auc_gate",
            "0.70",
            ParameterClass::A,
            None,
            TuningRoute::Manual,
        ),
        spec(
            "friction.w1",
            "0.40",
            ParameterClass::A,
            None,
            TuningRoute::Manual,
        ),
        spec(
            "friction.w2",
            "0.20",
            ParameterClass::A,
            None,
            TuningRoute::Manual,
        ),
        spec(
            "friction.w3",
            "0.20",
            ParameterClass::A,
            None,
            TuningRoute::Manual,
        ),
        spec(
            "friction.w4",
            "0.20",
            ParameterClass::A,
            None,
            TuningRoute::Manual,
        ),
        spec(
            "friction.lambda",
            "1.0",
            ParameterClass::B,
            Some("[0.5,3.0]"),
            TuningRoute::Mrt,
        ),
        spec(
            "mrt.epsilon",
            "0.20",
            ParameterClass::B,
            Some("[0.05,0.30]"),
            TuningRoute::Manual,
        ),
        spec(
            "sig.shrink_n0",
            "10",
            ParameterClass::B,
            Some("[5,30]"),
            TuningRoute::Replay,
        ),
        spec(
            "thompson.prior_n",
            "10",
            ParameterClass::B,
            Some("[5,30]"),
            TuningRoute::Replay,
        ),
        spec(
            "gu.retire_p",
            "0.30",
            ParameterClass::B,
            None,
            TuningRoute::Replay,
        ),
        spec(
            "gu.retire_thresh",
            "0.80",
            ParameterClass::B,
            None,
            TuningRoute::Replay,
        ),
        spec(
            "gu.window_days",
            "30",
            ParameterClass::B,
            None,
            TuningRoute::Replay,
        ),
    ]
    .into_iter()
    .map(|entry| (entry.key, entry))
    .collect()
}

pub fn meta_value(conn: &Connection, key: &str) -> Result<String> {
    if let Some(value) = conn
        .query_row("SELECT value FROM meta WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .optional()?
    {
        return Ok(value);
    }

    let registry = default_registry();
    registry
        .get(key)
        .map(|spec| spec.default_value.to_owned())
        .ok_or_else(|| PolarisError::InvalidParameter {
            key: key.to_owned(),
            value: "<missing>".to_owned(),
        })
}

pub fn meta_f64(conn: &Connection, key: &str) -> Result<f64> {
    let value = meta_value(conn, key)?;
    value.parse().map_err(|_| PolarisError::InvalidParameter {
        key: key.to_owned(),
        value,
    })
}

pub fn meta_i64(conn: &Connection, key: &str) -> Result<i64> {
    let value = meta_value(conn, key)?;
    value.parse().map_err(|_| PolarisError::InvalidParameter {
        key: key.to_owned(),
        value,
    })
}

pub fn meta_usize(conn: &Connection, key: &str) -> Result<usize> {
    let value = meta_value(conn, key)?;
    value.parse().map_err(|_| PolarisError::InvalidParameter {
        key: key.to_owned(),
        value,
    })
}

pub fn meta_json<T: serde::de::DeserializeOwned>(conn: &Connection, key: &str) -> Result<T> {
    let value = meta_value(conn, key)?;
    serde_json::from_str(&value).map_err(Into::into)
}

fn spec(
    key: &'static str,
    default_value: &'static str,
    class: ParameterClass,
    bounds: Option<&'static str>,
    tuning_route: TuningRoute,
) -> ParameterSpec {
    ParameterSpec {
        key,
        default_value,
        class,
        bounds,
        tuning_route,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_registry_contains_p01_constants() {
        let registry = default_registry();

        let p_init = registry.get("bkt.p_init").expect("bkt.p_init");
        assert_eq!(p_init.default_value, "0.20");
        assert_eq!(p_init.class, ParameterClass::B);
        assert_eq!(p_init.tuning_route, TuningRoute::Replay);

        let quote_min = registry.get("grade.quote_min").expect("grade.quote_min");
        assert_eq!(quote_min.default_value, "8");
        assert_eq!(quote_min.class, ParameterClass::A);
        assert_eq!(quote_min.tuning_route, TuningRoute::Manual);
    }

    #[test]
    fn parameter_registry_contains_all_data_model_section_10_keys() {
        let registry = default_registry();

        for key in [
            "mirt.eta",
            "mirt.step_cap",
            "mirt.shrink",
            "mirt.fuse_n0",
            "mirt.d.recall",
            "mirt.d.choose",
            "mirt.d.cloze",
            "mirt.d.rewrite",
            "mirt.d.apply",
            "mirt.d.translate",
            "mirt.d.transfer",
            "mirt.d.free_produce",
            "mirt.a_c",
            "consol.accept_margin",
            "consol.holdout_frac",
            "hmm.em_min_n",
            "hmm.gate_auc_margin",
            "hazard.auc_gate",
            "friction.w1",
            "friction.w2",
            "friction.w3",
            "friction.w4",
            "friction.lambda",
            "mrt.epsilon",
            "sig.shrink_n0",
            "thompson.prior_n",
            "gu.retire_p",
            "gu.retire_thresh",
            "gu.window_days",
        ] {
            assert!(registry.contains_key(key), "missing {key}");
        }
    }

    #[test]
    fn parameter_registry_contains_p02a_graph_threshold() {
        let registry = default_registry();
        let threshold = registry
            .get("graph.struct_threshold")
            .expect("graph.struct_threshold");

        assert_eq!(threshold.default_value, "0.40");
        assert_eq!(threshold.class, ParameterClass::A);
        assert_eq!(threshold.tuning_route, TuningRoute::Manual);

        let hnsw_m = registry.get("geometry.hnsw_m").expect("geometry.hnsw_m");
        assert_eq!(hnsw_m.default_value, "16");

        let ef_search = registry
            .get("geometry.ef_search")
            .expect("geometry.ef_search");
        assert_eq!(ef_search.default_value, "64");

        let embedding_dim = registry.get("embedding.dim").expect("embedding.dim");
        assert_eq!(embedding_dim.default_value, "0");
        assert_eq!(embedding_dim.class, ParameterClass::C);
    }
}

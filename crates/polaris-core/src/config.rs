use std::collections::BTreeMap;

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
    ]
    .into_iter()
    .map(|entry| (entry.key, entry))
    .collect()
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
}

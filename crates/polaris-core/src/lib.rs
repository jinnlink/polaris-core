pub mod ai_profile;
pub mod breeding;
pub mod calibration;
pub mod capture_queue;
pub mod citation;
pub mod config;
pub mod consolidation;
pub mod db;
pub mod diagnosis;
pub mod engine;
pub mod error;
pub mod fsrs;
pub mod fsrs_fit;
pub mod geometry;
pub mod goals;
pub mod grader;
pub mod graph;
pub mod gu;
pub mod gu_prior;
pub mod inbox_practice;
pub mod knowledge_map;
pub mod learner_feedback;
pub mod learner_inbox;
pub mod learner_mirror;
pub mod mastery;
pub mod mental_fit;
pub mod mental_state;
pub mod mirt;
pub mod moves;
pub mod ops;
pub mod pack;
pub mod pack_state;
pub mod pedagogy;
pub mod phase;
pub mod phase_dynamics;
pub mod prediction_map;
pub mod privacy;
pub mod profile;
pub mod project_manifest;
pub mod report;
pub mod sandbox;
pub mod scheduler;
pub mod session;
pub mod simulation;
pub mod status;
pub mod teaching;
pub mod trust;
pub mod tuning;

pub const VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_exports_version() {
        assert_eq!(crate::VERSION, "0.1.0");
    }
}

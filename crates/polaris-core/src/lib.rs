pub mod breeding;
pub mod calibration;
pub mod citation;
pub mod config;
pub mod consolidation;
pub mod db;
pub mod diagnosis;
pub mod engine;
pub mod error;
pub mod fsrs;
pub mod geometry;
pub mod goals;
pub mod grader;
pub mod graph;
pub mod gu;
pub mod learner_mirror;
pub mod mastery;
pub mod mental_fit;
pub mod mental_state;
pub mod mirt;
pub mod moves;
pub mod ops;
pub mod pack;
pub mod pedagogy;
pub mod phase;
pub mod privacy;
pub mod report;
pub mod scheduler;
pub mod simulation;
pub mod status;
pub mod teaching;
pub mod tuning;

pub const VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_exports_version() {
        assert_eq!(crate::VERSION, "0.1.0");
    }
}

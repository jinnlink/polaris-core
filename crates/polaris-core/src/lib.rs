pub mod citation;
pub mod config;
pub mod consolidation;
pub mod db;
pub mod diagnosis;
pub mod engine;
pub mod error;
pub mod fsrs;
pub mod geometry;
pub mod grader;
pub mod graph;
pub mod mastery;
pub mod mental_state;
pub mod mirt;
pub mod moves;
pub mod pack;
pub mod phase;
pub mod scheduler;
pub mod status;
pub mod teaching;

pub const VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_exports_version() {
        assert_eq!(crate::VERSION, "0.1.0");
    }
}

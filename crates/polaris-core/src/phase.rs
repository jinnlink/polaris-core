use serde::{Deserialize, Serialize};

use crate::config::{meta_f64, meta_i64};
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Depth {
    Recall,
    Explain,
    Apply,
    Analyze,
    Evaluate,
    Create,
    Transfer,
}

impl Depth {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "recall" => Some(Self::Recall),
            "explain" => Some(Self::Explain),
            "apply" => Some(Self::Apply),
            "analyze" => Some(Self::Analyze),
            "evaluate" => Some(Self::Evaluate),
            "create" => Some(Self::Create),
            "transfer" => Some(Self::Transfer),
            _ => None,
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Recall => 0,
            Self::Explain => 1,
            Self::Apply => 2,
            Self::Analyze | Self::Evaluate => 3,
            Self::Create => 4,
            Self::Transfer => 5,
        }
    }

    fn at_most(self, other: Self) -> bool {
        self.rank() <= other.rank()
    }

    fn at_least(self, other: Self) -> bool {
        self.rank() >= other.rank()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Undetermined,
    Phantom,
    Fluctuation,
    Settling,
    Solidification,
    Transfer,
    Generation,
    Regression,
}

impl Phase {
    pub const ALL: [Phase; 8] = [
        Phase::Undetermined,
        Phase::Phantom,
        Phase::Fluctuation,
        Phase::Settling,
        Phase::Solidification,
        Phase::Transfer,
        Phase::Generation,
        Phase::Regression,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Undetermined => "undetermined",
            Self::Phantom => "phantom",
            Self::Fluctuation => "fluctuation",
            Self::Settling => "settling",
            Self::Solidification => "solidification",
            Self::Transfer => "transfer",
            Self::Generation => "generation",
            Self::Regression => "regression",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "undetermined" => Some(Self::Undetermined),
            "phantom" => Some(Self::Phantom),
            "fluctuation" => Some(Self::Fluctuation),
            "settling" => Some(Self::Settling),
            "solidification" => Some(Self::Solidification),
            "transfer" => Some(Self::Transfer),
            "generation" => Some(Self::Generation),
            "regression" => Some(Self::Regression),
            _ => None,
        }
    }

    pub fn progress_rank(self) -> u8 {
        match self {
            Self::Undetermined | Self::Phantom | Self::Regression => 0,
            Self::Fluctuation => 1,
            Self::Settling => 2,
            Self::Solidification => 3,
            Self::Transfer => 4,
            Self::Generation => 5,
        }
    }

    pub fn schedule_bonus(self) -> f64 {
        match self {
            Self::Regression => 0.20,
            Self::Phantom => 0.15,
            Self::Undetermined => 0.05,
            _ => 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PhaseInput {
    pub p_known: f64,
    pub retrievability: Option<f64>,
    pub theta_prediction: Option<f64>,
    pub calib_gap: f64,
    pub attempt_count: u32,
    pub lapses: u32,
    pub recent_lapses: u32,
    pub max_depth: Option<Depth>,
    pub has_transfer_success: bool,
    pub ever_reached_transfer_or_generation: bool,
    pub relevant_task_attempt_count: u32,
    pub original_context_success: u32,
    pub transfer_fail_count: u32,
    pub novel_context_success: u32,
    pub novel_context_fail: u32,
    pub median_latency_ratio: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseParams {
    pub phantom_gap: f64,
    pub phantom_p: f64,
    pub phantom_n: u32,
}

impl PhaseParams {
    pub fn from_conn(conn: &rusqlite::Connection) -> Result<Self> {
        Ok(Self {
            phantom_gap: meta_f64(conn, "calib.phantom_gap")?,
            phantom_p: meta_f64(conn, "calib.phantom_p")?,
            phantom_n: meta_i64(conn, "calib.phantom_n")?.max(0) as u32,
        })
    }
}

pub fn determine_phase(input: &PhaseInput, params: &PhaseParams) -> Phase {
    if input.attempt_count < 2 {
        return Phase::Undetermined;
    }

    if input.ever_reached_transfer_or_generation && input.recent_lapses >= 2 && input.p_known < 0.5
    {
        return Phase::Regression;
    }

    if input.attempt_count >= params.phantom_n
        && input.calib_gap >= params.phantom_gap
        && input.p_known < params.phantom_p
    {
        return Phase::Phantom;
    }

    if transfer_condition(input)
        && input.relevant_task_attempt_count >= 3
        && input
            .median_latency_ratio
            .is_some_and(|ratio| ratio.is_finite() && ratio < 1.0)
    {
        return Phase::Generation;
    }

    if transfer_condition(input) {
        return Phase::Transfer;
    }

    let Some(max_depth) = input.max_depth else {
        return Phase::Undetermined;
    };

    if !input.has_transfer_success
        && input.p_known >= 0.6
        && max_depth.at_least(Depth::Apply)
        && input.transfer_fail_count >= 2
    {
        return Phase::Solidification;
    }

    if !input.has_transfer_success
        && input.p_known >= 0.6
        && max_depth.at_least(Depth::Apply)
        && input.original_context_success >= 2
        && input.novel_context_fail >= 2
    {
        return Phase::Settling;
    }

    if input.p_known >= 0.6 && max_depth.at_most(Depth::Explain) {
        return Phase::Fluctuation;
    }

    Phase::Undetermined
}

fn transfer_condition(input: &PhaseInput) -> bool {
    input.p_known >= 0.7 && input.has_transfer_success
}

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

    pub fn label(self) -> &'static str {
        match self {
            Self::Undetermined => "还看不清",
            Self::Phantom => "看起来懂",
            Self::Fluctuation => "刚上路",
            Self::Settling => "刚扎根",
            Self::Solidification => "稳了但僵",
            Self::Transfer => "能迁移",
            Self::Generation => "能创造",
            Self::Regression => "退步了",
        }
    }

    pub fn summary(self) -> &'static str {
        match self {
            Self::Undetermined => "才试了几次，证据还不够，系统会先补探针任务。",
            Self::Phantom => "自信高但实际表现不稳，需要用更硬的题确认。",
            Self::Fluctuation => "表现起伏明显，结果还不结实。",
            Self::Settling => "原场景中渐稳，新场景还卡。",
            Self::Solidification => "熟练但迁移受限，需要用变式题松动。",
            Self::Transfer => "能在新情境使用。",
            Self::Generation => "能独立产出，且迁移表现更快更稳。",
            Self::Regression => "之前会但近期又脱档，需要回到证据补缺。",
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
    pub calibration_overestimates: usize,
    pub calibration_sample_count: usize,
    pub calibration_probability_over_half: f64,
    pub median_latency_ratio: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseParams {
    pub phantom_gap: f64,
    pub phantom_p: f64,
    pub phantom_n: u32,
    pub phantom_confidence: f64,
}

impl PhaseParams {
    pub fn from_conn(conn: &rusqlite::Connection) -> Result<Self> {
        Ok(Self {
            phantom_gap: meta_f64(conn, "calib.phantom_gap")?,
            phantom_p: meta_f64(conn, "calib.phantom_p")?,
            phantom_n: meta_i64(conn, "calib.phantom_n")?.max(0) as u32,
            phantom_confidence: meta_f64(conn, "calib.phantom_confidence")?,
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
        && input.calibration_sample_count >= params.phantom_n as usize
        && input.calibration_probability_over_half >= params.phantom_confidence
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

use rusqlite::{params, Connection};

use crate::error::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct CalibrationSample {
    pub attempt_id: String,
    pub gap: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CalibrationPosterior {
    pub overestimates: usize,
    pub total: usize,
    pub alpha: f64,
    pub beta: f64,
    pub probability_over_half: f64,
}

pub fn calibration_samples(
    conn: &Connection,
    concept_id: &str,
    limit: usize,
) -> Result<Vec<CalibrationSample>> {
    let mut stmt = conn.prepare(
        "SELECT id, (self_confidence - 1.0) / 4.0 - COALESCE(final_score, provisional_score)
         FROM attempts
         WHERE concept_id=?1 AND self_confidence IS NOT NULL
           AND COALESCE(final_score, provisional_score) IS NOT NULL
         ORDER BY julianday(COALESCE(created_at, '1970-01-01T00:00:00Z')) DESC, id DESC
         LIMIT ?2",
    )?;
    let samples = stmt
        .query_map(params![concept_id, limit as i64], |row| {
            Ok(CalibrationSample {
                attempt_id: row.get(0)?,
                gap: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(samples)
}

pub fn posterior_from_samples(samples: &[CalibrationSample]) -> CalibrationPosterior {
    posterior_from_counts(
        samples.iter().filter(|sample| sample.gap > 0.0).count(),
        samples.len(),
    )
}

pub fn posterior_from_counts(overestimates: usize, total: usize) -> CalibrationPosterior {
    let overestimates = overestimates.min(total);
    let alpha = (overestimates + 1) as f64;
    let beta = (total - overestimates + 1) as f64;
    CalibrationPosterior {
        overestimates,
        total,
        alpha,
        beta,
        probability_over_half: prob_beta_greater_half(alpha, beta),
    }
}

/// Lanczos 近似 ln Γ(x)，x > 0。
pub fn ln_gamma(x: f64) -> f64 {
    const G: f64 = 7.0;
    const COEFFS: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut sum = COEFFS[0];
    for (idx, coeff) in COEFFS.iter().enumerate().skip(1) {
        sum += coeff / (x + idx as f64);
    }
    let t = x + G + 0.5;
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + sum.ln()
}

fn ln_beta(a: f64, b: f64) -> f64 {
    ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)
}

/// 正则不完全 Beta 函数 I_x(a, b)（连分式实现）。
pub fn regularized_incomplete_beta(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let ln_front = a * x.ln() + b * (1.0 - x).ln() - ln_beta(a, b);
    if x < (a + 1.0) / (a + b + 2.0) {
        (ln_front.exp() * beta_continued_fraction(x, a, b) / a).clamp(0.0, 1.0)
    } else {
        (1.0 - ln_front.exp() * beta_continued_fraction(1.0 - x, b, a) / b).clamp(0.0, 1.0)
    }
}

fn beta_continued_fraction(x: f64, a: f64, b: f64) -> f64 {
    const MAX_ITERATIONS: usize = 200;
    const EPSILON: f64 = 1e-12;
    const TINY: f64 = 1e-300;

    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut result = d;
    for m in 1..=MAX_ITERATIONS {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        result *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let delta = d * c;
        result *= delta;
        if (delta - 1.0).abs() < EPSILON {
            break;
        }
    }
    result
}

/// P(X > 0.5)，X ~ Beta(a, b)。
pub fn prob_beta_greater_half(a: f64, b: f64) -> f64 {
    1.0 - regularized_incomplete_beta(0.5, a, b)
}

/// P(X > Y)，X ~ Beta(a1, b1)，Y ~ Beta(a2, b2)，要求 a1 为正整数（计数 + 1 满足）。
pub fn prob_beta_greater(a1: f64, b1: f64, a2: f64, b2: f64) -> f64 {
    let steps = a1.round().max(1.0) as usize;
    let mut total = 0.0;
    for i in 0..steps {
        let i = i as f64;
        let ln_term =
            ln_beta(a2 + i, b1 + b2) - (b1 + i).ln() - ln_beta(1.0 + i, b1) - ln_beta(a2, b2);
        total += ln_term.exp();
    }
    total.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posterior_from_counts_uses_uniform_prior() {
        let posterior = posterior_from_counts(4, 4);
        assert_eq!(posterior.overestimates, 4);
        assert_eq!(posterior.total, 4);
        assert_eq!(posterior.alpha, 5.0);
        assert_eq!(posterior.beta, 1.0);
        assert!((posterior.probability_over_half - 0.96875).abs() < 1e-12);
    }

    #[test]
    fn posterior_from_counts_clamps_overestimates_to_total() {
        let posterior = posterior_from_counts(9, 2);
        assert_eq!(posterior.overestimates, 2);
        assert_eq!(posterior.total, 2);
    }

    #[test]
    fn ln_gamma_matches_factorials() {
        assert!((ln_gamma(5.0) - 24.0_f64.ln()).abs() < 1e-10);
        assert!((ln_gamma(1.0)).abs() < 1e-10);
        assert!((ln_gamma(0.5) - std::f64::consts::PI.sqrt().ln()).abs() < 1e-10);
    }

    #[test]
    fn incomplete_beta_known_values() {
        assert!((regularized_incomplete_beta(0.5, 2.0, 2.0) - 0.5).abs() < 1e-9);
        assert!((regularized_incomplete_beta(0.5, 1.0, 1.0) - 0.5).abs() < 1e-9);
        let expected = 1.0 - 0.7_f64.powi(3);
        assert!((regularized_incomplete_beta(0.3, 1.0, 3.0) - expected).abs() < 1e-9);
    }

    #[test]
    fn prob_beta_greater_symmetric_is_half() {
        let p = prob_beta_greater(3.0, 5.0, 3.0, 5.0);
        assert!((p - 0.5).abs() < 1e-9);
    }

    #[test]
    fn prob_beta_greater_separated_approaches_one() {
        let p = prob_beta_greater(40.0, 2.0, 2.0, 40.0);
        assert!(p > 0.999_999);
    }

    #[test]
    fn prob_beta_greater_half_matches_cdf() {
        let direct = prob_beta_greater_half(8.0, 3.0);
        assert!((direct - (1.0 - regularized_incomplete_beta(0.5, 8.0, 3.0))).abs() < 1e-12);
    }
}

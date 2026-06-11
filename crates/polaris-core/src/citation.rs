use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use rusqlite::Connection;

use crate::config::{default_registry, meta_usize, ParameterSpec};
use crate::error::Result as PolarisResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation {
    pub evidence_id: String,
    pub quote: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceText {
    pub id: String,
    pub text: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CitationError {
    #[error("evidence {evidence_id} is not allowed for this attempt")]
    EvidenceNotAllowed { evidence_id: String },
    #[error("quote for evidence {evidence_id} is too short: {len}")]
    QuoteTooShort { evidence_id: String, len: usize },
    #[error("quote for evidence {evidence_id} is too long: {len}")]
    QuoteTooLong { evidence_id: String, len: usize },
    #[error("quote for evidence {evidence_id} is not a source substring")]
    QuoteNotFound { evidence_id: String },
}

#[derive(Debug, Clone, Copy)]
pub struct CitationPolicy {
    pub quote_min: usize,
    pub quote_max: usize,
}

impl CitationPolicy {
    pub fn defaults() -> Self {
        let registry = default_registry();
        Self {
            quote_min: parse_usize(&registry, "grade.quote_min"),
            quote_max: parse_usize(&registry, "grade.quote_max"),
        }
    }

    pub fn from_conn(conn: &Connection) -> PolarisResult<Self> {
        Ok(Self {
            quote_min: meta_usize(conn, "grade.quote_min")?,
            quote_max: meta_usize(conn, "grade.quote_max")?,
        })
    }
}

pub fn validate_citations(
    citations: &[Citation],
    evidence: &[EvidenceText],
) -> Result<(), CitationError> {
    validate_citations_with_policy(citations, evidence, CitationPolicy::defaults())
}

pub fn validate_citations_with_policy(
    citations: &[Citation],
    evidence: &[EvidenceText],
    policy: CitationPolicy,
) -> Result<(), CitationError> {
    let evidence_by_id: HashMap<&str, &str> = evidence
        .iter()
        .map(|item| (item.id.as_str(), item.text.as_str()))
        .collect();

    for citation in citations {
        let Some(source_text) = evidence_by_id.get(citation.evidence_id.as_str()) else {
            return Err(CitationError::EvidenceNotAllowed {
                evidence_id: citation.evidence_id.clone(),
            });
        };

        let quote = citation.quote.trim();
        let len = quote.chars().count();
        if len < policy.quote_min {
            return Err(CitationError::QuoteTooShort {
                evidence_id: citation.evidence_id.clone(),
                len,
            });
        }
        if len > policy.quote_max {
            return Err(CitationError::QuoteTooLong {
                evidence_id: citation.evidence_id.clone(),
                len,
            });
        }
        if !source_text.contains(quote) {
            return Err(CitationError::QuoteNotFound {
                evidence_id: citation.evidence_id.clone(),
            });
        }
    }

    Ok(())
}

fn parse_usize(
    registry: &std::collections::BTreeMap<&'static str, ParameterSpec>,
    key: &'static str,
) -> usize {
    registry[key]
        .default_value
        .parse()
        .expect("valid default usize parameter")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence() -> Vec<EvidenceText> {
        vec![EvidenceText {
            id: "ev1".to_owned(),
            text: "Ownership moves values unless borrowing is used explicitly.".to_owned(),
        }]
    }

    #[test]
    fn accepts_quote_that_is_allowed_evidence_substring() {
        let citations = [Citation {
            evidence_id: "ev1".to_owned(),
            quote: "moves values".to_owned(),
        }];

        validate_citations(&citations, &evidence()).unwrap();
    }

    #[test]
    fn rejects_short_quote() {
        let citations = [Citation {
            evidence_id: "ev1".to_owned(),
            quote: "moves".to_owned(),
        }];

        assert!(matches!(
            validate_citations(&citations, &evidence()),
            Err(CitationError::QuoteTooShort { .. })
        ));
    }

    #[test]
    fn rejects_long_quote() {
        let citations = [Citation {
            evidence_id: "ev1".to_owned(),
            quote: "x".repeat(221),
        }];

        assert!(matches!(
            validate_citations(&citations, &evidence()),
            Err(CitationError::QuoteTooLong { .. })
        ));
    }

    #[test]
    fn rejects_non_substring_quote() {
        let citations = [Citation {
            evidence_id: "ev1".to_owned(),
            quote: "borrow checker accepts every alias".to_owned(),
        }];

        assert!(matches!(
            validate_citations(&citations, &evidence()),
            Err(CitationError::QuoteNotFound { .. })
        ));
    }

    #[test]
    fn rejects_citation_to_unrelated_evidence() {
        let citations = [Citation {
            evidence_id: "ev2".to_owned(),
            quote: "moves values".to_owned(),
        }];

        assert!(matches!(
            validate_citations(&citations, &evidence()),
            Err(CitationError::EvidenceNotAllowed { .. })
        ));
    }
}

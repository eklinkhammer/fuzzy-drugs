//! Drug resolution models for the semantic resolver.

use serde::{Deserialize, Serialize};

/// Extracted drug mention from NER.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DrugMention {
    /// Raw text as spoken/transcribed
    pub raw_text: String,
    /// Extracted drug name
    pub drug_name: String,
    /// Extracted dose value
    pub dose: Option<f64>,
    /// Extracted dose unit (e.g., "mg", "mL", "cc")
    pub unit: Option<String>,
    /// Extracted route (e.g., "orally", "IV", "subcutaneously")
    pub route: Option<String>,
    /// Extracted species if mentioned
    pub species: Option<String>,
    /// Start position in transcript
    pub start_offset: usize,
    /// End position in transcript
    pub end_offset: usize,
}

/// Normalized drug mention after alias/unit conversion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NormalizedMention {
    /// Original mention
    pub original: DrugMention,
    /// Normalized drug name (after alias expansion)
    pub normalized_name: String,
    /// Normalized dose (after unit conversion)
    pub normalized_dose: Option<f64>,
    /// Normalized unit (canonical form: mg, mL, etc.)
    pub normalized_unit: Option<String>,
    /// Normalized route (canonical form: PO, IV, IM, SQ, etc.)
    pub normalized_route: Option<String>,
}

/// A candidate SKU match with scoring breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoredCandidate {
    /// The catalog SKU
    pub sku: String,
    /// The catalog item name
    pub name: String,
    /// Overall confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Breakdown of scoring factors
    pub score_breakdown: ScoreBreakdown,
}

/// Breakdown of how a candidate was scored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreBreakdown {
    /// Name/alias match quality (0.0 - 1.0) - weight: 40%
    pub name_score: f64,
    /// Species compatibility (0.0 - 1.0) - weight: 25%
    pub species_score: f64,
    /// Route compatibility (0.0 - 1.0) - weight: 20%
    pub route_score: f64,
    /// Dose plausibility (0.0 - 1.0) - weight: 15%
    pub dose_score: f64,
}

impl ScoreBreakdown {
    /// Calculate weighted confidence score.
    pub fn weighted_score(&self) -> f64 {
        self.name_score * 0.40
            + self.species_score * 0.25
            + self.route_score * 0.20
            + self.dose_score * 0.15
    }
}

/// A resolved drug item ready for vet review.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedItem {
    /// The normalized mention
    pub mention: NormalizedMention,
    /// Top candidate (best match)
    pub top_candidate: ScoredCandidate,
    /// Alternative candidates (for vet to choose from if needed)
    pub alternatives: Vec<ScoredCandidate>,
    /// Resolution status
    pub status: ResolutionStatus,
}

/// Status of a drug resolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResolutionStatus {
    /// Awaiting vet review
    PendingReview,
    /// Vet approved the top candidate
    Approved,
    /// Vet selected an alternative
    AlternativeSelected { selected_sku: String },
    /// Vet manually entered a different SKU
    ManualOverride { override_sku: String },
    /// Vet rejected - no match appropriate
    Rejected,
}

impl ResolvedItem {
    /// Get the final SKU based on resolution status.
    pub fn final_sku(&self) -> Option<&str> {
        match &self.status {
            ResolutionStatus::Approved => Some(&self.top_candidate.sku),
            ResolutionStatus::AlternativeSelected { selected_sku } => Some(selected_sku),
            ResolutionStatus::ManualOverride { override_sku } => Some(override_sku),
            ResolutionStatus::PendingReview | ResolutionStatus::Rejected => None,
        }
    }

    /// Check if this item needs vet attention.
    pub fn needs_review(&self) -> bool {
        matches!(self.status, ResolutionStatus::PendingReview)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_breakdown_weighted() {
        let breakdown = ScoreBreakdown {
            name_score: 1.0,
            species_score: 1.0,
            route_score: 1.0,
            dose_score: 1.0,
        };
        assert!((breakdown.weighted_score() - 1.0).abs() < 0.001);

        let breakdown2 = ScoreBreakdown {
            name_score: 0.5,
            species_score: 0.5,
            route_score: 0.5,
            dose_score: 0.5,
        };
        assert!((breakdown2.weighted_score() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_resolved_item_final_sku() {
        let mention = NormalizedMention {
            original: DrugMention {
                raw_text: "test".into(),
                drug_name: "test".into(),
                dose: None,
                unit: None,
                route: None,
                species: None,
                start_offset: 0,
                end_offset: 4,
            },
            normalized_name: "test".into(),
            normalized_dose: None,
            normalized_unit: None,
            normalized_route: None,
        };

        let candidate = ScoredCandidate {
            sku: "SKU001".into(),
            name: "Test Drug".into(),
            confidence: 0.9,
            score_breakdown: ScoreBreakdown {
                name_score: 0.9,
                species_score: 1.0,
                route_score: 1.0,
                dose_score: 1.0,
            },
        };

        let mut item = ResolvedItem {
            mention,
            top_candidate: candidate,
            alternatives: vec![],
            status: ResolutionStatus::PendingReview,
        };

        assert!(item.needs_review());
        assert!(item.final_sku().is_none());

        item.status = ResolutionStatus::Approved;
        assert!(!item.needs_review());
        assert_eq!(item.final_sku(), Some("SKU001"));

        item.status = ResolutionStatus::AlternativeSelected {
            selected_sku: "SKU002".into(),
        };
        assert_eq!(item.final_sku(), Some("SKU002"));
    }
}

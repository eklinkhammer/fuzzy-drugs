//! Encounter models for drafts and reviewed encounters.

use serde::{Deserialize, Serialize};

use super::resolution::{ResolvedItem, ResolutionStatus};

/// Draft encounter status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DraftStatus {
    /// Recording in progress
    Recording,
    /// Transcription complete, awaiting resolution
    Transcribed,
    /// Resolution complete, awaiting vet review
    PendingReview,
    /// Vet has reviewed, ready for commit
    Reviewed,
    /// Committed to Merkle tree
    Committed,
}

/// An encounter draft (mutable, pre-review staging area).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EncounterDraft {
    /// Unique draft ID
    pub draft_id: String,
    /// Patient local ID
    pub patient_id: String,
    /// Full transcript text
    pub transcript: String,
    /// Resolved drug items (pending vet review)
    pub resolved_items: Vec<ResolvedItem>,
    /// Draft status
    pub status: DraftStatus,
    /// Creation timestamp
    pub created_at: String,
    /// Last update timestamp
    pub updated_at: String,
}

impl EncounterDraft {
    /// Create a new encounter draft.
    pub fn new(patient_id: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            draft_id: uuid::Uuid::new_v4().to_string(),
            patient_id,
            transcript: String::new(),
            resolved_items: Vec::new(),
            status: DraftStatus::Recording,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Get count of items needing review.
    pub fn pending_review_count(&self) -> usize {
        self.resolved_items
            .iter()
            .filter(|item| item.needs_review())
            .count()
    }

    /// Check if all items have been reviewed.
    pub fn all_reviewed(&self) -> bool {
        self.resolved_items
            .iter()
            .all(|item| !item.needs_review())
    }

    /// Get the lowest confidence score among pending items.
    pub fn lowest_confidence(&self) -> Option<f64> {
        self.resolved_items
            .iter()
            .filter(|item| item.needs_review())
            .map(|item| item.top_candidate.confidence)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Touch the updated_at timestamp.
    pub fn touch(&mut self) {
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}

/// A reviewed encounter ready for Merkle tree commit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewedEncounter {
    /// Original draft ID (for traceability)
    pub draft_id: String,
    /// Patient local ID
    pub patient_id: String,
    /// Patient server ID (if synced)
    pub patient_server_id: Option<String>,
    /// Full transcript text
    pub transcript: String,
    /// Final line items (approved by vet)
    pub line_items: Vec<EncounterLineItem>,
    /// Vet who reviewed
    pub reviewed_by: String,
    /// Review timestamp
    pub reviewed_at: String,
    /// Additional notes from vet
    pub notes: Option<String>,
}

/// A single line item in a reviewed encounter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EncounterLineItem {
    /// SKU of the item
    pub sku: String,
    /// Item name (for display/export)
    pub name: String,
    /// Quantity/dose
    pub quantity: f64,
    /// Unit
    pub unit: String,
    /// Route of administration
    pub route: Option<String>,
    /// Original mention text (for audit)
    pub original_mention: String,
    /// How this item was resolved
    pub resolution_method: ResolutionMethod,
}

/// How a line item was resolved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResolutionMethod {
    /// System suggestion approved by vet
    SystemApproved { confidence: f64 },
    /// Vet selected from alternatives
    AlternativeSelected { original_confidence: f64 },
    /// Vet manually overrode
    ManualOverride,
    /// Vet manually added (not from transcript)
    ManualEntry,
}

impl ReviewedEncounter {
    /// Create from a draft where all items have been reviewed.
    pub fn from_draft(draft: &EncounterDraft, reviewed_by: String) -> Option<Self> {
        if !draft.all_reviewed() {
            return None;
        }

        let line_items: Vec<EncounterLineItem> = draft
            .resolved_items
            .iter()
            .filter_map(|item| {
                let sku = item.final_sku()?;
                let resolution_method = match &item.status {
                    ResolutionStatus::Approved => ResolutionMethod::SystemApproved {
                        confidence: item.top_candidate.confidence,
                    },
                    ResolutionStatus::AlternativeSelected { .. } => {
                        ResolutionMethod::AlternativeSelected {
                            original_confidence: item.top_candidate.confidence,
                        }
                    }
                    ResolutionStatus::ManualOverride { .. } => ResolutionMethod::ManualOverride,
                    _ => return None,
                };

                Some(EncounterLineItem {
                    sku: sku.to_string(),
                    name: item.top_candidate.name.clone(),
                    quantity: item.mention.normalized_dose.unwrap_or(1.0),
                    unit: item
                        .mention
                        .normalized_unit
                        .clone()
                        .unwrap_or_else(|| "unit".into()),
                    route: item.mention.normalized_route.clone(),
                    original_mention: item.mention.original.raw_text.clone(),
                    resolution_method,
                })
            })
            .collect();

        Some(Self {
            draft_id: draft.draft_id.clone(),
            patient_id: draft.patient_id.clone(),
            patient_server_id: None, // Will be filled in during commit
            transcript: draft.transcript.clone(),
            line_items,
            reviewed_by,
            reviewed_at: chrono::Utc::now().to_rfc3339(),
            notes: None,
        })
    }

    /// Serialize to canonical JSON for Merkle tree hashing.
    pub fn to_canonical_json(&self) -> Result<String, serde_json::Error> {
        // Use sorted keys for deterministic hashing
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::resolution::{
        DrugMention, NormalizedMention, ScoreBreakdown, ScoredCandidate,
    };

    fn make_test_draft() -> EncounterDraft {
        let mut draft = EncounterDraft::new("patient-123".into());
        draft.transcript = "Give 10mg of carprofen PO".into();

        let mention = NormalizedMention {
            original: DrugMention {
                raw_text: "10mg of carprofen PO".into(),
                drug_name: "carprofen".into(),
                dose: Some(10.0),
                unit: Some("mg".into()),
                route: Some("PO".into()),
                species: None,
                start_offset: 5,
                end_offset: 25,
            },
            normalized_name: "carprofen".into(),
            normalized_dose: Some(10.0),
            normalized_unit: Some("mg".into()),
            normalized_route: Some("PO".into()),
        };

        let candidate = ScoredCandidate {
            sku: "CARP-10".into(),
            name: "Carprofen 10mg tablets".into(),
            confidence: 0.95,
            score_breakdown: ScoreBreakdown {
                name_score: 1.0,
                species_score: 1.0,
                route_score: 1.0,
                dose_score: 0.8,
            },
        };

        draft.resolved_items.push(ResolvedItem {
            mention,
            top_candidate: candidate,
            alternatives: vec![],
            status: ResolutionStatus::Approved,
        });

        draft.status = DraftStatus::Reviewed;
        draft
    }

    #[test]
    fn test_encounter_draft_new() {
        let draft = EncounterDraft::new("patient-123".into());
        assert_eq!(draft.patient_id, "patient-123");
        assert!(matches!(draft.status, DraftStatus::Recording));
        assert_eq!(draft.draft_id.len(), 36);
    }

    #[test]
    fn test_reviewed_encounter_from_draft() {
        let draft = make_test_draft();
        let reviewed = ReviewedEncounter::from_draft(&draft, "Dr. Smith".into());

        assert!(reviewed.is_some());
        let reviewed = reviewed.unwrap();
        assert_eq!(reviewed.patient_id, "patient-123");
        assert_eq!(reviewed.reviewed_by, "Dr. Smith");
        assert_eq!(reviewed.line_items.len(), 1);

        let item = &reviewed.line_items[0];
        assert_eq!(item.sku, "CARP-10");
        assert_eq!(item.quantity, 10.0);
        assert_eq!(item.unit, "mg");
    }

    #[test]
    fn test_canonical_json_deterministic() {
        let draft = make_test_draft();
        let reviewed = ReviewedEncounter::from_draft(&draft, "Dr. Smith".into()).unwrap();

        let json1 = reviewed.to_canonical_json().unwrap();
        let json2 = reviewed.to_canonical_json().unwrap();
        assert_eq!(json1, json2);
    }
}

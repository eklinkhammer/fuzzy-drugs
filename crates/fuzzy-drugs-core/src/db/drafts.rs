//! Encounter draft database operations.

use rusqlite::{params, OptionalExtension};

use super::{Database, DbError, DbResult};
use crate::models::{DraftStatus, EncounterDraft, ResolvedItem};

impl Database {
    /// Insert a new encounter draft.
    pub fn insert_draft(&self, draft: &EncounterDraft) -> DbResult<()> {
        let resolved_items_json = serde_json::to_string(&draft.resolved_items)?;
        let status_str = status_to_string(&draft.status);

        self.conn.execute(
            r#"
            INSERT INTO encounter_drafts (
                draft_id, patient_id, transcript, resolved_items,
                status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                draft.draft_id,
                draft.patient_id,
                draft.transcript,
                resolved_items_json,
                status_str,
                draft.created_at,
                draft.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Update an existing draft.
    pub fn update_draft(&self, draft: &EncounterDraft) -> DbResult<bool> {
        let resolved_items_json = serde_json::to_string(&draft.resolved_items)?;
        let status_str = status_to_string(&draft.status);

        let rows_affected = self.conn.execute(
            r#"
            UPDATE encounter_drafts SET
                transcript = ?2,
                resolved_items = ?3,
                status = ?4,
                updated_at = datetime('now')
            WHERE draft_id = ?1
            "#,
            params![
                draft.draft_id,
                draft.transcript,
                resolved_items_json,
                status_str,
            ],
        )?;
        Ok(rows_affected > 0)
    }

    /// Get a draft by ID.
    pub fn get_draft(&self, draft_id: &str) -> DbResult<Option<EncounterDraft>> {
        self.conn
            .query_row(
                r#"
                SELECT draft_id, patient_id, transcript, resolved_items,
                       status, created_at, updated_at
                FROM encounter_drafts
                WHERE draft_id = ?
                "#,
                [draft_id],
                |row| {
                    Ok(DraftRow {
                        draft_id: row.get(0)?,
                        patient_id: row.get(1)?,
                        transcript: row.get(2)?,
                        resolved_items: row.get(3)?,
                        status: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                },
            )
            .optional()?
            .map(|row| row.try_into())
            .transpose()
    }

    /// List drafts pending review, ordered by lowest confidence first.
    pub fn list_pending_review_drafts(&self) -> DbResult<Vec<EncounterDraft>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT draft_id, patient_id, transcript, resolved_items,
                   status, created_at, updated_at
            FROM encounter_drafts
            WHERE status = 'pending_review'
            ORDER BY updated_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(DraftRow {
                draft_id: row.get(0)?,
                patient_id: row.get(1)?,
                transcript: row.get(2)?,
                resolved_items: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;

        let mut drafts = Vec::new();
        for row in rows {
            drafts.push(row?.try_into()?);
        }

        // Sort by lowest confidence (items needing most attention first)
        drafts.sort_by(|a: &EncounterDraft, b: &EncounterDraft| {
            let conf_a = a.lowest_confidence().unwrap_or(1.0);
            let conf_b = b.lowest_confidence().unwrap_or(1.0);
            conf_a.partial_cmp(&conf_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(drafts)
    }

    /// List drafts by status.
    pub fn list_drafts_by_status(&self, status: &DraftStatus) -> DbResult<Vec<EncounterDraft>> {
        let status_str = status_to_string(status);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT draft_id, patient_id, transcript, resolved_items,
                   status, created_at, updated_at
            FROM encounter_drafts
            WHERE status = ?
            ORDER BY updated_at DESC
            "#,
        )?;

        let rows = stmt.query_map([status_str], |row| {
            Ok(DraftRow {
                draft_id: row.get(0)?,
                patient_id: row.get(1)?,
                transcript: row.get(2)?,
                resolved_items: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;

        let mut drafts = Vec::new();
        for row in rows {
            drafts.push(row?.try_into()?);
        }
        Ok(drafts)
    }

    /// List all drafts for a patient.
    pub fn list_drafts_for_patient(&self, patient_id: &str) -> DbResult<Vec<EncounterDraft>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT draft_id, patient_id, transcript, resolved_items,
                   status, created_at, updated_at
            FROM encounter_drafts
            WHERE patient_id = ?
            ORDER BY created_at DESC
            "#,
        )?;

        let rows = stmt.query_map([patient_id], |row| {
            Ok(DraftRow {
                draft_id: row.get(0)?,
                patient_id: row.get(1)?,
                transcript: row.get(2)?,
                resolved_items: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;

        let mut drafts = Vec::new();
        for row in rows {
            drafts.push(row?.try_into()?);
        }
        Ok(drafts)
    }

    /// Delete a draft.
    pub fn delete_draft(&self, draft_id: &str) -> DbResult<bool> {
        let rows_affected = self
            .conn
            .execute("DELETE FROM encounter_drafts WHERE draft_id = ?", [draft_id])?;
        Ok(rows_affected > 0)
    }

    /// Mark draft as committed (after Merkle tree commit).
    pub fn mark_draft_committed(&self, draft_id: &str) -> DbResult<bool> {
        let rows_affected = self.conn.execute(
            "UPDATE encounter_drafts SET status = 'committed', updated_at = datetime('now') WHERE draft_id = ?",
            [draft_id],
        )?;
        Ok(rows_affected > 0)
    }
}

/// Intermediate row struct for database mapping.
struct DraftRow {
    draft_id: String,
    patient_id: String,
    transcript: String,
    resolved_items: String,
    status: String,
    created_at: String,
    updated_at: String,
}

impl TryFrom<DraftRow> for EncounterDraft {
    type Error = DbError;

    fn try_from(row: DraftRow) -> Result<Self, Self::Error> {
        let resolved_items: Vec<ResolvedItem> = serde_json::from_str(&row.resolved_items)?;
        let status = string_to_status(&row.status)?;

        Ok(EncounterDraft {
            draft_id: row.draft_id,
            patient_id: row.patient_id,
            transcript: row.transcript,
            resolved_items,
            status,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

fn status_to_string(status: &DraftStatus) -> &'static str {
    match status {
        DraftStatus::Recording => "recording",
        DraftStatus::Transcribed => "transcribed",
        DraftStatus::PendingReview => "pending_review",
        DraftStatus::Reviewed => "reviewed",
        DraftStatus::Committed => "committed",
    }
}

fn string_to_status(s: &str) -> Result<DraftStatus, DbError> {
    match s {
        "recording" => Ok(DraftStatus::Recording),
        "transcribed" => Ok(DraftStatus::Transcribed),
        "pending_review" => Ok(DraftStatus::PendingReview),
        "reviewed" => Ok(DraftStatus::Reviewed),
        "committed" => Ok(DraftStatus::Committed),
        _ => Err(DbError::Constraint(format!("Unknown draft status: {}", s))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        DrugMention, NormalizedMention, ResolutionStatus, ResolvedItem, ScoreBreakdown,
        ScoredCandidate,
    };
    use crate::models::Patient;

    fn setup_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        // Create a test patient
        let patient = Patient::new("Max".into(), "canine".into());
        db.insert_patient(&patient).unwrap();
        db
    }

    fn make_resolved_item(confidence: f64) -> ResolvedItem {
        ResolvedItem {
            mention: NormalizedMention {
                original: DrugMention {
                    raw_text: "test".into(),
                    drug_name: "test".into(),
                    dose: Some(10.0),
                    unit: Some("mg".into()),
                    route: Some("PO".into()),
                    species: None,
                    start_offset: 0,
                    end_offset: 4,
                },
                normalized_name: "test".into(),
                normalized_dose: Some(10.0),
                normalized_unit: Some("mg".into()),
                normalized_route: Some("PO".into()),
            },
            top_candidate: ScoredCandidate {
                sku: "SKU001".into(),
                name: "Test Drug".into(),
                confidence,
                score_breakdown: ScoreBreakdown {
                    name_score: confidence,
                    species_score: 1.0,
                    route_score: 1.0,
                    dose_score: 1.0,
                },
            },
            alternatives: vec![],
            status: ResolutionStatus::PendingReview,
        }
    }

    #[test]
    fn test_insert_and_get_draft() {
        let db = setup_db();
        let patients = db.list_patients().unwrap();
        let patient_id = patients[0].local_id.clone();

        let mut draft = EncounterDraft::new(patient_id);
        draft.transcript = "Give 10mg carprofen PO".into();
        draft.resolved_items.push(make_resolved_item(0.85));
        draft.status = DraftStatus::PendingReview;

        db.insert_draft(&draft).unwrap();

        let retrieved = db.get_draft(&draft.draft_id).unwrap().unwrap();
        assert_eq!(retrieved.transcript, "Give 10mg carprofen PO");
        assert_eq!(retrieved.resolved_items.len(), 1);
        assert!(matches!(retrieved.status, DraftStatus::PendingReview));
    }

    #[test]
    fn test_update_draft() {
        let db = setup_db();
        let patients = db.list_patients().unwrap();
        let patient_id = patients[0].local_id.clone();

        let mut draft = EncounterDraft::new(patient_id);
        db.insert_draft(&draft).unwrap();

        draft.transcript = "Updated transcript".into();
        draft.status = DraftStatus::Transcribed;
        db.update_draft(&draft).unwrap();

        let retrieved = db.get_draft(&draft.draft_id).unwrap().unwrap();
        assert_eq!(retrieved.transcript, "Updated transcript");
        assert!(matches!(retrieved.status, DraftStatus::Transcribed));
    }

    #[test]
    fn test_list_pending_review_sorted_by_confidence() {
        let db = setup_db();
        let patients = db.list_patients().unwrap();
        let patient_id = patients[0].local_id.clone();

        // Create drafts with different confidence levels
        let mut draft1 = EncounterDraft::new(patient_id.clone());
        draft1.resolved_items.push(make_resolved_item(0.95)); // High confidence
        draft1.status = DraftStatus::PendingReview;
        db.insert_draft(&draft1).unwrap();

        let mut draft2 = EncounterDraft::new(patient_id.clone());
        draft2.resolved_items.push(make_resolved_item(0.50)); // Low confidence
        draft2.status = DraftStatus::PendingReview;
        db.insert_draft(&draft2).unwrap();

        let mut draft3 = EncounterDraft::new(patient_id);
        draft3.resolved_items.push(make_resolved_item(0.75)); // Medium confidence
        draft3.status = DraftStatus::PendingReview;
        db.insert_draft(&draft3).unwrap();

        let pending = db.list_pending_review_drafts().unwrap();
        assert_eq!(pending.len(), 3);

        // Should be sorted by lowest confidence first
        assert_eq!(pending[0].draft_id, draft2.draft_id); // 0.50
        assert_eq!(pending[1].draft_id, draft3.draft_id); // 0.75
        assert_eq!(pending[2].draft_id, draft1.draft_id); // 0.95
    }

    #[test]
    fn test_mark_committed() {
        let db = setup_db();
        let patients = db.list_patients().unwrap();
        let patient_id = patients[0].local_id.clone();

        let mut draft = EncounterDraft::new(patient_id);
        draft.status = DraftStatus::Reviewed;
        db.insert_draft(&draft).unwrap();

        db.mark_draft_committed(&draft.draft_id).unwrap();

        let retrieved = db.get_draft(&draft.draft_id).unwrap().unwrap();
        assert!(matches!(retrieved.status, DraftStatus::Committed));
    }
}

//! Compliance export with full audit trail.

use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::merkle::{ComplianceProof, MerkleResult, MerkleTree};
use crate::models::ReviewedEncounter;

/// Full compliance export for a single encounter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncounterComplianceExport {
    /// Export metadata
    pub metadata: ComplianceMetadata,
    /// The full encounter data
    pub encounter: ReviewedEncounter,
    /// Merkle inclusion proof
    pub proof: ComplianceProof,
}

/// Compliance export metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceMetadata {
    /// Export format version
    pub format_version: String,
    /// Export timestamp
    pub exported_at: String,
    /// Hash algorithm used
    pub hash_algorithm: String,
    /// Exporting system identifier
    pub system_id: Option<String>,
}

impl EncounterComplianceExport {
    /// Export to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Batch compliance export for audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchComplianceExport {
    /// Export metadata
    pub metadata: BatchComplianceMetadata,
    /// Individual encounter exports
    pub encounters: Vec<EncounterComplianceExport>,
}

/// Batch compliance export metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchComplianceMetadata {
    /// Export format version
    pub format_version: String,
    /// Export timestamp
    pub exported_at: String,
    /// Hash algorithm used
    pub hash_algorithm: String,
    /// Root hash at export time
    pub root_hash: String,
    /// Tree height
    pub tree_height: u32,
    /// Total leaf count
    pub leaf_count: u32,
    /// Exporting system identifier
    pub system_id: Option<String>,
}

impl BatchComplianceExport {
    /// Export to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Verify all proofs in the export.
    pub fn verify_all_proofs(&self) -> Vec<ProofVerification> {
        self.encounters
            .iter()
            .map(|enc| {
                let is_valid = crate::merkle::verify_proof(&crate::merkle::MerkleProof {
                    leaf_hash: enc.proof.leaf_hash.clone(),
                    root_hash: enc.proof.root_hash.clone(),
                    proof_hashes: enc
                        .proof
                        .audit_path
                        .iter()
                        .map(|e| e.hash.clone())
                        .collect(),
                    proof_directions: enc
                        .proof
                        .audit_path
                        .iter()
                        .map(|e| e.position == "right")
                        .collect(),
                    leaf_index: enc.proof.leaf_index,
                });

                ProofVerification {
                    draft_id: enc.encounter.draft_id.clone(),
                    leaf_hash: enc.proof.leaf_hash.clone(),
                    is_valid,
                }
            })
            .collect()
    }
}

/// Result of proof verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofVerification {
    /// Draft ID
    pub draft_id: String,
    /// Leaf hash
    pub leaf_hash: String,
    /// Whether proof is valid
    pub is_valid: bool,
}

/// Compliance exporter.
pub struct ComplianceExporter<'a> {
    db: &'a Database,
    tree: MerkleTree<'a>,
    system_id: Option<String>,
}

impl<'a> ComplianceExporter<'a> {
    /// Create a new compliance exporter.
    pub fn new(db: &'a Database) -> Self {
        Self {
            db,
            tree: MerkleTree::new(db),
            system_id: None,
        }
    }

    /// Set the system identifier for exports.
    pub fn with_system_id(mut self, system_id: String) -> Self {
        self.system_id = Some(system_id);
        self
    }

    /// Export compliance data for a specific leaf hash.
    pub fn export_by_hash(&self, leaf_hash: &str) -> MerkleResult<EncounterComplianceExport> {
        let payload = self
            .tree
            .get_leaf_payload(leaf_hash)?
            .ok_or_else(|| crate::merkle::MerkleError::NodeNotFound(leaf_hash.to_string()))?;

        let encounter: ReviewedEncounter = serde_json::from_str(&payload)?;
        let proof = self.tree.generate_proof(leaf_hash)?;

        Ok(EncounterComplianceExport {
            metadata: ComplianceMetadata {
                format_version: "1.0".to_string(),
                exported_at: chrono::Utc::now().to_rfc3339(),
                hash_algorithm: "SHA-256".to_string(),
                system_id: self.system_id.clone(),
            },
            encounter,
            proof: proof.to_compliance_format(),
        })
    }

    /// Export full compliance data for all encounters.
    pub fn export_all(&self) -> MerkleResult<BatchComplianceExport> {
        let root_state = self.db.get_merkle_root()?;
        let leaf_hashes = self.db.get_all_leaf_hashes()?;

        let mut encounters = Vec::new();
        for hash in leaf_hashes {
            encounters.push(self.export_by_hash(&hash)?);
        }

        Ok(BatchComplianceExport {
            metadata: BatchComplianceMetadata {
                format_version: "1.0".to_string(),
                exported_at: chrono::Utc::now().to_rfc3339(),
                hash_algorithm: "SHA-256".to_string(),
                root_hash: root_state.root_hash.unwrap_or_default(),
                tree_height: root_state.tree_height,
                leaf_count: root_state.leaf_count,
                system_id: self.system_id.clone(),
            },
            encounters,
        })
    }

    /// Export compliance data for a date range.
    pub fn export_date_range(
        &self,
        start: &str,
        end: &str,
    ) -> MerkleResult<BatchComplianceExport> {
        let root_state = self.db.get_merkle_root()?;
        let nodes = self.db.get_nodes_since(start)?;

        let mut encounters = Vec::new();
        for node in nodes {
            if node.payload.is_some() && node.created_at <= end.to_string() {
                encounters.push(self.export_by_hash(&node.hash)?);
            }
        }

        Ok(BatchComplianceExport {
            metadata: BatchComplianceMetadata {
                format_version: "1.0".to_string(),
                exported_at: chrono::Utc::now().to_rfc3339(),
                hash_algorithm: "SHA-256".to_string(),
                root_hash: root_state.root_hash.unwrap_or_default(),
                tree_height: root_state.tree_height,
                leaf_count: root_state.leaf_count,
                system_id: self.system_id.clone(),
            },
            encounters,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EncounterLineItem, ResolutionMethod};

    fn make_encounter(id: &str) -> ReviewedEncounter {
        ReviewedEncounter {
            draft_id: id.to_string(),
            patient_id: "patient-1".to_string(),
            patient_server_id: None,
            transcript: "Test transcript".to_string(),
            line_items: vec![EncounterLineItem {
                sku: "SKU001".to_string(),
                name: "Test Drug".to_string(),
                quantity: 10.0,
                unit: "mg".to_string(),
                route: Some("PO".to_string()),
                original_mention: "10mg test drug".to_string(),
                resolution_method: ResolutionMethod::SystemApproved { confidence: 0.95 },
            }],
            reviewed_by: "Dr. Smith".to_string(),
            reviewed_at: "2024-01-15T10:00:00Z".to_string(),
            notes: None,
        }
    }

    #[test]
    fn test_single_compliance_export() {
        let db = Database::open_in_memory().unwrap();
        let tree = MerkleTree::new(&db);

        let enc = make_encounter("draft-1");
        let commit = tree.commit_encounter(&enc).unwrap();

        let exporter = ComplianceExporter::new(&db).with_system_id("test-system".into());
        let export = exporter.export_by_hash(&commit.leaf_hash).unwrap();

        assert_eq!(export.encounter.draft_id, "draft-1");
        assert_eq!(export.proof.leaf_hash, commit.leaf_hash);
        assert_eq!(export.proof.root_hash, commit.root_hash);
        assert_eq!(export.metadata.system_id, Some("test-system".into()));
    }

    #[test]
    fn test_batch_compliance_export() {
        let db = Database::open_in_memory().unwrap();
        let tree = MerkleTree::new(&db);

        for i in 1..=3 {
            let enc = make_encounter(&format!("draft-{}", i));
            tree.commit_encounter(&enc).unwrap();
        }

        let exporter = ComplianceExporter::new(&db);
        let batch = exporter.export_all().unwrap();

        assert_eq!(batch.encounters.len(), 3);
        assert_eq!(batch.metadata.leaf_count, 3);
    }

    #[test]
    fn test_proof_verification() {
        let db = Database::open_in_memory().unwrap();
        let tree = MerkleTree::new(&db);

        for i in 1..=3 {
            let enc = make_encounter(&format!("draft-{}", i));
            tree.commit_encounter(&enc).unwrap();
        }

        let exporter = ComplianceExporter::new(&db);
        let batch = exporter.export_all().unwrap();

        let verifications = batch.verify_all_proofs();
        assert_eq!(verifications.len(), 3);
        assert!(verifications.iter().all(|v| v.is_valid));
    }

    #[test]
    fn test_compliance_export_json() {
        let db = Database::open_in_memory().unwrap();
        let tree = MerkleTree::new(&db);

        let enc = make_encounter("draft-1");
        tree.commit_encounter(&enc).unwrap();

        let exporter = ComplianceExporter::new(&db);
        let batch = exporter.export_all().unwrap();

        let json = batch.to_json().unwrap();
        assert!(json.contains("draft-1"));
        assert!(json.contains("audit_path"));
        assert!(json.contains("SHA-256"));
    }
}

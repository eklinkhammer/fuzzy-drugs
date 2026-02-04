//! Merkle proof structures for compliance export.

use serde::{Deserialize, Serialize};

/// Merkle inclusion proof (RFC 6962 style).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Hash of the leaf being proven
    pub leaf_hash: String,
    /// Root hash at time of proof generation
    pub root_hash: String,
    /// Sibling hashes from leaf to root
    pub proof_hashes: Vec<String>,
    /// Direction of each sibling (true = right, false = left)
    pub proof_directions: Vec<bool>,
    /// Index of leaf in tree (for context)
    pub leaf_index: usize,
}

impl MerkleProof {
    /// Serialize proof to standard format for compliance export.
    pub fn to_compliance_format(&self) -> ComplianceProof {
        ComplianceProof {
            version: "1.0".to_string(),
            algorithm: "SHA-256".to_string(),
            leaf_hash: self.leaf_hash.clone(),
            root_hash: self.root_hash.clone(),
            audit_path: self
                .proof_hashes
                .iter()
                .zip(self.proof_directions.iter())
                .map(|(hash, dir)| AuditPathEntry {
                    hash: hash.clone(),
                    position: if *dir { "right" } else { "left" }.to_string(),
                })
                .collect(),
            leaf_index: self.leaf_index,
        }
    }
}

/// Compliance-friendly proof format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceProof {
    /// Format version
    pub version: String,
    /// Hash algorithm used
    pub algorithm: String,
    /// Hash of the data being proven
    pub leaf_hash: String,
    /// Root hash at time of proof
    pub root_hash: String,
    /// Audit path from leaf to root
    pub audit_path: Vec<AuditPathEntry>,
    /// Index of leaf in tree
    pub leaf_index: usize,
}

/// Single entry in audit path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditPathEntry {
    /// Sibling hash
    pub hash: String,
    /// Position of sibling ("left" or "right")
    pub position: String,
}

/// Complete compliance export for an encounter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceExport {
    /// Export metadata
    pub metadata: ExportMetadata,
    /// The encounter data
    pub encounter: serde_json::Value,
    /// Inclusion proof
    pub proof: ComplianceProof,
}

/// Export metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMetadata {
    /// Export timestamp
    pub exported_at: String,
    /// Export format version
    pub format_version: String,
    /// Exporting system identifier
    pub system_id: Option<String>,
}

impl ComplianceExport {
    /// Create a new compliance export.
    pub fn new(encounter_json: &str, proof: MerkleProof) -> Result<Self, serde_json::Error> {
        let encounter: serde_json::Value = serde_json::from_str(encounter_json)?;

        Ok(Self {
            metadata: ExportMetadata {
                exported_at: chrono::Utc::now().to_rfc3339(),
                format_version: "1.0".to_string(),
                system_id: None,
            },
            encounter,
            proof: proof.to_compliance_format(),
        })
    }

    /// Serialize to JSON for export.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_proof_format() {
        let proof = MerkleProof {
            leaf_hash: "abc123".to_string(),
            root_hash: "root456".to_string(),
            proof_hashes: vec!["sibling1".to_string(), "sibling2".to_string()],
            proof_directions: vec![true, false],
            leaf_index: 2,
        };

        let compliance = proof.to_compliance_format();

        assert_eq!(compliance.version, "1.0");
        assert_eq!(compliance.algorithm, "SHA-256");
        assert_eq!(compliance.audit_path.len(), 2);
        assert_eq!(compliance.audit_path[0].position, "right");
        assert_eq!(compliance.audit_path[1].position, "left");
    }

    #[test]
    fn test_compliance_export_serialization() {
        let encounter_json = r#"{"draft_id": "test", "patient_id": "p1"}"#;
        let proof = MerkleProof {
            leaf_hash: "abc123".to_string(),
            root_hash: "root456".to_string(),
            proof_hashes: vec![],
            proof_directions: vec![],
            leaf_index: 0,
        };

        let export = ComplianceExport::new(encounter_json, proof).unwrap();
        let json = export.to_json().unwrap();

        assert!(json.contains("draft_id"));
        assert!(json.contains("proof"));
        assert!(json.contains("metadata"));
    }
}

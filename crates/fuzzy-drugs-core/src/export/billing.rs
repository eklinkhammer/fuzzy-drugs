//! Billing export for PIMS integration.

use serde::{Deserialize, Serialize};

use crate::db::Database;
use crate::merkle::{MerkleResult, MerkleTree};
use crate::models::ReviewedEncounter;

/// Billing export for a single encounter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingExport {
    /// Export metadata
    pub metadata: BillingMetadata,
    /// Line items for billing
    pub line_items: Vec<BillingLineItem>,
}

/// Billing export metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingMetadata {
    /// Draft ID for traceability
    pub draft_id: String,
    /// Patient local ID
    pub patient_id: String,
    /// Patient server ID (if synced)
    pub patient_server_id: Option<String>,
    /// Vet who reviewed
    pub reviewed_by: String,
    /// Review timestamp
    pub reviewed_at: String,
    /// Export timestamp
    pub exported_at: String,
    /// Merkle leaf hash for audit trail
    pub merkle_leaf_hash: String,
}

/// Single line item for billing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingLineItem {
    /// SKU
    pub sku: String,
    /// Item description
    pub description: String,
    /// Quantity
    pub quantity: f64,
    /// Unit
    pub unit: String,
    /// Route of administration (optional)
    pub route: Option<String>,
}

impl BillingExport {
    /// Create billing export from a reviewed encounter and its Merkle hash.
    pub fn from_encounter(encounter: &ReviewedEncounter, merkle_hash: &str) -> Self {
        let line_items = encounter
            .line_items
            .iter()
            .map(|item| BillingLineItem {
                sku: item.sku.clone(),
                description: item.name.clone(),
                quantity: item.quantity,
                unit: item.unit.clone(),
                route: item.route.clone(),
            })
            .collect();

        Self {
            metadata: BillingMetadata {
                draft_id: encounter.draft_id.clone(),
                patient_id: encounter.patient_id.clone(),
                patient_server_id: encounter.patient_server_id.clone(),
                reviewed_by: encounter.reviewed_by.clone(),
                reviewed_at: encounter.reviewed_at.clone(),
                exported_at: chrono::Utc::now().to_rfc3339(),
                merkle_leaf_hash: merkle_hash.to_string(),
            },
            line_items,
        }
    }

    /// Export to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Export to CSV format.
    pub fn to_csv(&self) -> String {
        let mut csv = String::new();

        // Header
        csv.push_str("draft_id,patient_id,sku,description,quantity,unit,route,reviewed_by,reviewed_at,merkle_hash\n");

        // Lines
        for item in &self.line_items {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{}\n",
                escape_csv(&self.metadata.draft_id),
                escape_csv(&self.metadata.patient_id),
                escape_csv(&item.sku),
                escape_csv(&item.description),
                item.quantity,
                escape_csv(&item.unit),
                item.route.as_deref().unwrap_or(""),
                escape_csv(&self.metadata.reviewed_by),
                escape_csv(&self.metadata.reviewed_at),
                escape_csv(&self.metadata.merkle_leaf_hash),
            ));
        }

        csv
    }
}

/// Batch billing export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchBillingExport {
    /// Export timestamp
    pub exported_at: String,
    /// Individual encounter exports
    pub encounters: Vec<BillingExport>,
    /// Total line item count
    pub total_items: usize,
}

impl BatchBillingExport {
    /// Export to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Export to CSV format.
    pub fn to_csv(&self) -> String {
        let mut csv = String::new();

        // Header
        csv.push_str("draft_id,patient_id,sku,description,quantity,unit,route,reviewed_by,reviewed_at,merkle_hash\n");

        // Lines from all encounters
        for export in &self.encounters {
            for item in &export.line_items {
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{}\n",
                    escape_csv(&export.metadata.draft_id),
                    escape_csv(&export.metadata.patient_id),
                    escape_csv(&item.sku),
                    escape_csv(&item.description),
                    item.quantity,
                    escape_csv(&item.unit),
                    item.route.as_deref().unwrap_or(""),
                    escape_csv(&export.metadata.reviewed_by),
                    escape_csv(&export.metadata.reviewed_at),
                    escape_csv(&export.metadata.merkle_leaf_hash),
                ));
            }
        }

        csv
    }
}

/// Billing exporter.
pub struct BillingExporter<'a> {
    db: &'a Database,
    tree: MerkleTree<'a>,
}

impl<'a> BillingExporter<'a> {
    /// Create a new billing exporter.
    pub fn new(db: &'a Database) -> Self {
        Self {
            db,
            tree: MerkleTree::new(db),
        }
    }

    /// Export billing for a specific leaf hash.
    pub fn export_by_hash(&self, leaf_hash: &str) -> MerkleResult<BillingExport> {
        let payload = self
            .tree
            .get_leaf_payload(leaf_hash)?
            .ok_or_else(|| crate::merkle::MerkleError::NodeNotFound(leaf_hash.to_string()))?;

        let encounter: ReviewedEncounter = serde_json::from_str(&payload)?;
        Ok(BillingExport::from_encounter(&encounter, leaf_hash))
    }

    /// Export billing for all leaves.
    pub fn export_all(&self) -> MerkleResult<BatchBillingExport> {
        let leaf_hashes = self.db.get_all_leaf_hashes()?;
        let mut encounters = Vec::new();
        let mut total_items = 0;

        for hash in leaf_hashes {
            let export = self.export_by_hash(&hash)?;
            total_items += export.line_items.len();
            encounters.push(export);
        }

        Ok(BatchBillingExport {
            exported_at: chrono::Utc::now().to_rfc3339(),
            encounters,
            total_items,
        })
    }

    /// Export billing for leaves since a given timestamp.
    pub fn export_since(&self, since: &str) -> MerkleResult<BatchBillingExport> {
        let nodes = self.db.get_nodes_since(since)?;
        let mut encounters = Vec::new();
        let mut total_items = 0;

        for node in nodes {
            if node.payload.is_some() {
                // It's a leaf
                let export = self.export_by_hash(&node.hash)?;
                total_items += export.line_items.len();
                encounters.push(export);
            }
        }

        Ok(BatchBillingExport {
            exported_at: chrono::Utc::now().to_rfc3339(),
            encounters,
            total_items,
        })
    }
}

/// Escape a string for CSV output.
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EncounterLineItem, ResolutionMethod};

    fn make_encounter() -> ReviewedEncounter {
        ReviewedEncounter {
            draft_id: "draft-1".to_string(),
            patient_id: "patient-1".to_string(),
            patient_server_id: Some("server-patient-1".to_string()),
            transcript: "Test transcript".to_string(),
            line_items: vec![
                EncounterLineItem {
                    sku: "SKU001".to_string(),
                    name: "Carprofen 100mg".to_string(),
                    quantity: 2.0,
                    unit: "tablets".to_string(),
                    route: Some("PO".to_string()),
                    original_mention: "2 carprofen tablets".to_string(),
                    resolution_method: ResolutionMethod::SystemApproved { confidence: 0.95 },
                },
                EncounterLineItem {
                    sku: "SKU002".to_string(),
                    name: "Meloxicam 1.5mg/mL".to_string(),
                    quantity: 0.5,
                    unit: "mL".to_string(),
                    route: Some("PO".to_string()),
                    original_mention: "half mL meloxicam".to_string(),
                    resolution_method: ResolutionMethod::SystemApproved { confidence: 0.88 },
                },
            ],
            reviewed_by: "Dr. Smith".to_string(),
            reviewed_at: "2024-01-15T10:00:00Z".to_string(),
            notes: None,
        }
    }

    #[test]
    fn test_billing_export_from_encounter() {
        let encounter = make_encounter();
        let export = BillingExport::from_encounter(&encounter, "hash123");

        assert_eq!(export.metadata.draft_id, "draft-1");
        assert_eq!(export.metadata.merkle_leaf_hash, "hash123");
        assert_eq!(export.line_items.len(), 2);
        assert_eq!(export.line_items[0].sku, "SKU001");
        assert_eq!(export.line_items[0].quantity, 2.0);
    }

    #[test]
    fn test_billing_export_json() {
        let encounter = make_encounter();
        let export = BillingExport::from_encounter(&encounter, "hash123");

        let json = export.to_json().unwrap();
        assert!(json.contains("SKU001"));
        assert!(json.contains("Carprofen 100mg"));
    }

    #[test]
    fn test_billing_export_csv() {
        let encounter = make_encounter();
        let export = BillingExport::from_encounter(&encounter, "hash123");

        let csv = export.to_csv();
        let lines: Vec<&str> = csv.lines().collect();

        assert_eq!(lines.len(), 3); // Header + 2 items
        assert!(lines[0].contains("draft_id"));
        assert!(lines[1].contains("SKU001"));
        assert!(lines[2].contains("SKU002"));
    }

    #[test]
    fn test_csv_escaping() {
        assert_eq!(escape_csv("simple"), "simple");
        assert_eq!(escape_csv("with,comma"), "\"with,comma\"");
        assert_eq!(escape_csv("with\"quote"), "\"with\"\"quote\"");
    }

    #[test]
    fn test_batch_billing_export() {
        let db = Database::open_in_memory().unwrap();
        let tree = MerkleTree::new(&db);

        // Commit encounters
        let enc1 = make_encounter();
        tree.commit_encounter(&enc1).unwrap();

        let mut enc2 = make_encounter();
        enc2.draft_id = "draft-2".to_string();
        tree.commit_encounter(&enc2).unwrap();

        let exporter = BillingExporter::new(&db);
        let batch = exporter.export_all().unwrap();

        assert_eq!(batch.encounters.len(), 2);
        assert_eq!(batch.total_items, 4); // 2 items per encounter
    }
}

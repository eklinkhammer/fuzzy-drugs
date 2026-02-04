//! Merkle tree sync protocol for PIMS integration.
//!
//! Protocol:
//! 1. Local sends root hash to PIMS
//! 2. PIMS responds with list of missing node hashes
//! 3. Local sends missing nodes
//! 4. PIMS verifies and acknowledges new root

use serde::{Deserialize, Serialize};

use crate::db::{Database, MerkleNode, MerkleNodeType};

use super::{MerkleError, MerkleResult, MerkleTree};

/// Request to initiate sync (sent to PIMS).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    /// Local root hash
    pub root_hash: String,
    /// Local tree height
    pub tree_height: u32,
    /// Local leaf count
    pub leaf_count: u32,
}

/// Response from PIMS indicating missing nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    /// Hashes of nodes PIMS needs
    pub missing_hashes: Vec<String>,
    /// PIMS's current root hash (for conflict detection)
    pub server_root_hash: Option<String>,
}

/// Nodes being sent to PIMS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPayload {
    /// Nodes to sync
    pub nodes: Vec<SyncNode>,
    /// Expected new root after sync
    pub expected_root: String,
}

/// A single node in the sync payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncNode {
    /// Node hash
    pub hash: String,
    /// Node type
    pub node_type: String,
    /// Left child hash (for internal nodes)
    pub left_child: Option<String>,
    /// Right child hash (for internal nodes)
    pub right_child: Option<String>,
    /// Payload JSON (for leaf nodes)
    pub payload: Option<String>,
}

impl From<MerkleNode> for SyncNode {
    fn from(node: MerkleNode) -> Self {
        Self {
            hash: node.hash,
            node_type: match node.node_type {
                MerkleNodeType::Leaf => "leaf".to_string(),
                MerkleNodeType::Internal => "internal".to_string(),
            },
            left_child: node.left_child,
            right_child: node.right_child,
            payload: node.payload,
        }
    }
}

/// Acknowledgment from PIMS after successful sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAck {
    /// Success status
    pub success: bool,
    /// New server root hash
    pub new_root: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Full tree export for audit or cold storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeExport {
    /// Export timestamp
    pub exported_at: String,
    /// Root hash at export time
    pub root_hash: String,
    /// Tree height
    pub tree_height: u32,
    /// Total leaf count
    pub leaf_count: u32,
    /// All nodes in the tree
    pub nodes: Vec<SyncNode>,
}

/// Sync manager for handling PIMS communication.
pub struct SyncManager<'a> {
    db: &'a Database,
    #[allow(dead_code)]
    tree: MerkleTree<'a>,
}

impl<'a> SyncManager<'a> {
    /// Create a new sync manager.
    pub fn new(db: &'a Database) -> Self {
        Self {
            db,
            tree: MerkleTree::new(db),
        }
    }

    /// Create a sync request to send to PIMS.
    pub fn create_sync_request(&self) -> MerkleResult<Option<SyncRequest>> {
        let root_state = self.db.get_merkle_root()?;

        match root_state.root_hash {
            Some(root_hash) => Ok(Some(SyncRequest {
                root_hash,
                tree_height: root_state.tree_height,
                leaf_count: root_state.leaf_count,
            })),
            None => Ok(None), // Empty tree, nothing to sync
        }
    }

    /// Process a sync response from PIMS and create payload.
    pub fn process_sync_response(&self, response: &SyncResponse) -> MerkleResult<SyncPayload> {
        let nodes = self.db.get_nodes_by_hashes(&response.missing_hashes)?;

        let root_state = self.db.get_merkle_root()?;
        let expected_root = root_state
            .root_hash
            .ok_or_else(|| MerkleError::InvalidState("No root hash".into()))?;

        Ok(SyncPayload {
            nodes: nodes.into_iter().map(SyncNode::from).collect(),
            expected_root,
        })
    }

    /// Handle sync acknowledgment from PIMS.
    pub fn handle_sync_ack(&self, ack: &SyncAck) -> MerkleResult<()> {
        if ack.success {
            if let Some(root) = &ack.new_root {
                self.db.set_sync_state("last_synced_root", root)?;
                self.db.set_sync_state(
                    "encounters_last_sync",
                    &chrono::Utc::now().to_rfc3339(),
                )?;
            }
        }
        Ok(())
    }

    /// Get the last synced root hash.
    pub fn get_last_synced_root(&self) -> MerkleResult<Option<String>> {
        let value = self.db.get_sync_state("last_synced_root")?;
        Ok(value.filter(|s| !s.is_empty()))
    }

    /// Check if there are unsynced changes.
    pub fn has_unsynced_changes(&self) -> MerkleResult<bool> {
        let current_root = self.db.get_merkle_root()?.root_hash;
        let last_synced = self.get_last_synced_root()?;

        match (current_root, last_synced) {
            (None, _) => Ok(false), // Empty tree
            (Some(_), None) => Ok(true), // Never synced
            (Some(current), Some(last)) => Ok(current != last),
        }
    }

    /// Export full tree for compliance audit.
    pub fn export_full_tree(&self) -> MerkleResult<TreeExport> {
        let root_state = self.db.get_merkle_root()?;
        let root_hash = root_state
            .root_hash
            .ok_or_else(|| MerkleError::InvalidState("No root hash".into()))?;

        // Get all nodes
        let nodes = self.collect_all_nodes(&root_hash)?;

        Ok(TreeExport {
            exported_at: chrono::Utc::now().to_rfc3339(),
            root_hash,
            tree_height: root_state.tree_height,
            leaf_count: root_state.leaf_count,
            nodes: nodes.into_iter().map(SyncNode::from).collect(),
        })
    }

    /// Export nodes added since a given root hash.
    pub fn export_since(&self, since_root: Option<&str>) -> MerkleResult<TreeExport> {
        let root_state = self.db.get_merkle_root()?;
        let current_root = root_state
            .root_hash
            .ok_or_else(|| MerkleError::InvalidState("No root hash".into()))?;

        let nodes = match since_root {
            Some(old_root) => {
                // Get timestamp of old root node
                let old_node = self.db.get_merkle_node(old_root)?;
                match old_node {
                    Some(node) => self.db.get_nodes_since(&node.created_at)?,
                    None => self.collect_all_nodes(&current_root)?,
                }
            }
            None => self.collect_all_nodes(&current_root)?,
        };

        Ok(TreeExport {
            exported_at: chrono::Utc::now().to_rfc3339(),
            root_hash: current_root,
            tree_height: root_state.tree_height,
            leaf_count: root_state.leaf_count,
            nodes: nodes.into_iter().map(SyncNode::from).collect(),
        })
    }

    /// Collect all nodes in tree by traversing from root.
    fn collect_all_nodes(&self, root_hash: &str) -> MerkleResult<Vec<MerkleNode>> {
        let mut nodes = Vec::new();
        let mut stack = vec![root_hash.to_string()];

        while let Some(hash) = stack.pop() {
            if let Some(node) = self.db.get_merkle_node(&hash)? {
                if let Some(ref left) = node.left_child {
                    stack.push(left.clone());
                }
                if let Some(ref right) = node.right_child {
                    stack.push(right.clone());
                }
                nodes.push(node);
            }
        }

        Ok(nodes)
    }
}

/// Catalog sync for downloading inventory updates from PIMS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSyncRequest {
    /// Last sync timestamp (ISO 8601)
    pub since: Option<String>,
}

/// Catalog delta from PIMS.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogDelta {
    /// Items to upsert
    pub items: Vec<CatalogSyncItem>,
    /// SKUs to deactivate
    pub deactivated_skus: Vec<String>,
    /// Timestamp of this delta
    pub timestamp: String,
}

/// Catalog item for sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSyncItem {
    pub sku: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub concentration: Option<String>,
    pub package_size: Option<String>,
    pub species: Vec<String>,
    pub routes: Vec<String>,
    pub active: bool,
    pub server_id: String,
}

impl SyncManager<'_> {
    /// Create catalog sync request.
    pub fn create_catalog_sync_request(&self) -> MerkleResult<CatalogSyncRequest> {
        let since = self.db.get_sync_state("catalog_last_sync")?;
        Ok(CatalogSyncRequest {
            since: since.filter(|s| !s.is_empty()),
        })
    }

    /// Apply catalog delta from PIMS.
    pub fn apply_catalog_delta(&self, delta: &CatalogDelta) -> MerkleResult<()> {
        use crate::models::CatalogItem;

        // Upsert items
        for item in &delta.items {
            let catalog_item = CatalogItem {
                sku: item.sku.clone(),
                name: item.name.clone(),
                aliases: item.aliases.clone(),
                concentration: item.concentration.clone(),
                package_size: item.package_size.clone(),
                species: item.species.clone(),
                routes: item.routes.clone(),
                dose_range: None, // Dose range managed locally
                active: item.active,
                server_id: Some(item.server_id.clone()),
                last_synced: Some(delta.timestamp.clone()),
            };
            self.db.upsert_catalog_item(&catalog_item)?;
        }

        // Deactivate removed items
        for sku in &delta.deactivated_skus {
            self.db.deactivate_catalog_item(sku)?;
        }

        // Update sync timestamp
        self.db.set_sync_state("catalog_last_sync", &delta.timestamp)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EncounterLineItem, ResolutionMethod, ReviewedEncounter};

    fn setup_db() -> Database {
        Database::open_in_memory().unwrap()
    }

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
                original_mention: "10mg test drug PO".to_string(),
                resolution_method: ResolutionMethod::SystemApproved { confidence: 0.95 },
            }],
            reviewed_by: "Dr. Smith".to_string(),
            reviewed_at: "2024-01-15T10:00:00Z".to_string(),
            notes: None,
        }
    }

    #[test]
    fn test_sync_request_empty_tree() {
        let db = setup_db();
        let manager = SyncManager::new(&db);

        let request = manager.create_sync_request().unwrap();
        assert!(request.is_none());
    }

    #[test]
    fn test_sync_request_with_data() {
        let db = setup_db();
        let tree = MerkleTree::new(&db);
        let manager = SyncManager::new(&db);

        // Commit an encounter
        let enc = make_encounter("draft-1");
        tree.commit_encounter(&enc).unwrap();

        let request = manager.create_sync_request().unwrap().unwrap();
        assert!(!request.root_hash.is_empty());
        assert_eq!(request.leaf_count, 1);
    }

    #[test]
    fn test_has_unsynced_changes() {
        let db = setup_db();
        let tree = MerkleTree::new(&db);
        let manager = SyncManager::new(&db);

        // Empty tree - no changes
        assert!(!manager.has_unsynced_changes().unwrap());

        // Add encounter - now has changes
        let enc = make_encounter("draft-1");
        tree.commit_encounter(&enc).unwrap();
        assert!(manager.has_unsynced_changes().unwrap());

        // Simulate sync ack
        let root = db.get_merkle_root().unwrap().root_hash.unwrap();
        manager
            .handle_sync_ack(&SyncAck {
                success: true,
                new_root: Some(root),
                error: None,
            })
            .unwrap();

        // Now synced
        assert!(!manager.has_unsynced_changes().unwrap());

        // Add another encounter
        let enc2 = make_encounter("draft-2");
        tree.commit_encounter(&enc2).unwrap();
        assert!(manager.has_unsynced_changes().unwrap());
    }

    #[test]
    fn test_export_full_tree() {
        let db = setup_db();
        let tree = MerkleTree::new(&db);
        let manager = SyncManager::new(&db);

        // Commit multiple encounters
        for i in 1..=3 {
            let enc = make_encounter(&format!("draft-{}", i));
            tree.commit_encounter(&enc).unwrap();
        }

        let export = manager.export_full_tree().unwrap();
        assert_eq!(export.leaf_count, 3);
        assert!(!export.nodes.is_empty());
        assert!(export.nodes.iter().filter(|n| n.node_type == "leaf").count() == 3);
    }

    #[test]
    fn test_catalog_sync() {
        let db = setup_db();
        let manager = SyncManager::new(&db);

        // Initial sync request should have no timestamp
        let request = manager.create_catalog_sync_request().unwrap();
        assert!(request.since.is_none());

        // Apply delta
        let delta = CatalogDelta {
            items: vec![CatalogSyncItem {
                sku: "NEW-SKU".into(),
                name: "New Drug 100mg".into(),
                aliases: vec!["newdrug".into()],
                concentration: Some("100mg".into()),
                package_size: None,
                species: vec!["canine".into()],
                routes: vec!["PO".into()],
                active: true,
                server_id: "server-123".into(),
            }],
            deactivated_skus: vec![],
            timestamp: "2024-01-15T12:00:00Z".into(),
        };

        manager.apply_catalog_delta(&delta).unwrap();

        // Verify item was added
        let item = db.get_catalog_item("NEW-SKU").unwrap().unwrap();
        assert_eq!(item.name, "New Drug 100mg");
        assert_eq!(item.server_id, Some("server-123".into()));

        // Next sync request should have timestamp
        let request = manager.create_catalog_sync_request().unwrap();
        assert_eq!(request.since, Some("2024-01-15T12:00:00Z".into()));
    }
}

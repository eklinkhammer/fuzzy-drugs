//! Merkle tree core implementation.

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::db::Database;
use crate::models::ReviewedEncounter;

use super::proof::MerkleProof;

/// Merkle tree errors.
#[derive(Error, Debug)]
pub enum MerkleError {
    #[error("Database error: {0}")]
    Database(#[from] crate::db::DbError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Invalid tree state: {0}")]
    InvalidState(String),
}

pub type MerkleResult<T> = Result<T, MerkleError>;

/// Result of committing an encounter to the Merkle tree.
#[derive(Debug, Clone)]
pub struct LeafCommit {
    /// Hash of the committed leaf
    pub leaf_hash: String,
    /// New root hash after commit
    pub root_hash: String,
    /// Inclusion proof for the leaf
    pub proof: MerkleProof,
    /// New tree height
    pub tree_height: u32,
    /// Total leaf count
    pub leaf_count: u32,
}

/// Merkle tree manager.
pub struct MerkleTree<'a> {
    db: &'a Database,
}

impl<'a> MerkleTree<'a> {
    /// Create a new Merkle tree manager.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Commit a reviewed encounter to the tree (append-only).
    pub fn commit_encounter(&self, encounter: &ReviewedEncounter) -> MerkleResult<LeafCommit> {
        // 1. Serialize encounter to canonical JSON
        let payload = encounter.to_canonical_json()?;

        // 2. Create leaf hash
        let leaf_hash = hash_data(payload.as_bytes());

        // 3. Check if leaf already exists (idempotency)
        if self.db.merkle_node_exists(&leaf_hash)? {
            // Leaf already committed, return existing state
            let root_state = self.db.get_merkle_root()?;
            let proof = self.generate_proof(&leaf_hash)?;
            return Ok(LeafCommit {
                leaf_hash,
                root_hash: root_state.root_hash.unwrap_or_default(),
                proof,
                tree_height: root_state.tree_height,
                leaf_count: root_state.leaf_count,
            });
        }

        // 4. Insert leaf node
        self.db.insert_merkle_leaf(&leaf_hash, &payload)?;

        // 5. Get all existing leaves and rebuild tree
        let all_leaves = self.db.get_all_leaf_hashes()?;

        // 6. Build tree from leaves
        let (new_root, height) = self.build_tree(&all_leaves)?;

        // 7. Update root atomically
        self.db
            .update_merkle_root(&new_root, height, all_leaves.len() as u32)?;

        // 8. Generate proof for the new leaf
        let proof = self.generate_proof(&leaf_hash)?;

        Ok(LeafCommit {
            leaf_hash,
            root_hash: new_root,
            proof,
            tree_height: height,
            leaf_count: all_leaves.len() as u32,
        })
    }

    /// Build/rebuild the tree from a list of leaf hashes.
    /// Returns (root_hash, height).
    fn build_tree(&self, leaves: &[String]) -> MerkleResult<(String, u32)> {
        if leaves.is_empty() {
            return Err(MerkleError::InvalidState("Cannot build tree with no leaves".into()));
        }

        if leaves.len() == 1 {
            return Ok((leaves[0].clone(), 1));
        }

        // Build tree bottom-up
        let mut current_level: Vec<String> = leaves.to_vec();
        let mut height = 1u32;

        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in current_level.chunks(2) {
                let left = &chunk[0];
                let right = chunk.get(1);

                let parent_hash = if let Some(r) = right {
                    // Hash both children
                    let combined = format!("{}{}", left, r);
                    hash_data(combined.as_bytes())
                } else {
                    // Odd node - promote with self-hash
                    let combined = format!("{}{}", left, left);
                    hash_data(combined.as_bytes())
                };

                // Insert internal node if it doesn't exist
                if !self.db.merkle_node_exists(&parent_hash)? {
                    self.db
                        .insert_merkle_internal(&parent_hash, left, right.map(|s| s.as_str()))?;
                }

                next_level.push(parent_hash);
            }

            current_level = next_level;
            height += 1;
        }

        Ok((current_level[0].clone(), height))
    }

    /// Generate an inclusion proof for a leaf.
    pub fn generate_proof(&self, leaf_hash: &str) -> MerkleResult<MerkleProof> {
        let root_state = self.db.get_merkle_root()?;
        let root_hash = root_state
            .root_hash
            .ok_or_else(|| MerkleError::InvalidState("Tree has no root".into()))?;

        // Get all leaves to find position
        let leaves = self.db.get_all_leaf_hashes()?;
        let leaf_index = leaves
            .iter()
            .position(|h| h == leaf_hash)
            .ok_or_else(|| MerkleError::NodeNotFound(leaf_hash.to_string()))?;

        // Build proof by walking up the tree
        let mut proof_hashes = Vec::new();
        let mut proof_directions = Vec::new();
        let mut current_level = leaves;
        let mut current_index = leaf_index;

        while current_level.len() > 1 {
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            // Get sibling hash (or self if odd node at end)
            let sibling_hash = if sibling_index < current_level.len() {
                current_level[sibling_index].clone()
            } else {
                current_level[current_index].clone()
            };

            // Direction: true = sibling is on right, false = sibling is on left
            let sibling_on_right = current_index % 2 == 0;

            proof_hashes.push(sibling_hash);
            proof_directions.push(sibling_on_right);

            // Build next level
            let mut next_level = Vec::new();
            for chunk in current_level.chunks(2) {
                let left = &chunk[0];
                let right = chunk.get(1).unwrap_or(left);
                let combined = format!("{}{}", left, right);
                next_level.push(hash_data(combined.as_bytes()));
            }

            current_level = next_level;
            current_index /= 2;
        }

        Ok(MerkleProof {
            leaf_hash: leaf_hash.to_string(),
            root_hash,
            proof_hashes,
            proof_directions,
            leaf_index,
        })
    }

    /// Verify that a proof is valid.
    pub fn verify_proof(&self, proof: &MerkleProof) -> bool {
        verify_proof(proof)
    }

    /// Get the current root hash.
    pub fn get_root_hash(&self) -> MerkleResult<Option<String>> {
        Ok(self.db.get_merkle_root()?.root_hash)
    }

    /// Get the current tree statistics.
    pub fn get_stats(&self) -> MerkleResult<TreeStats> {
        let state = self.db.get_merkle_root()?;
        Ok(TreeStats {
            root_hash: state.root_hash,
            height: state.tree_height,
            leaf_count: state.leaf_count,
        })
    }

    /// Get a leaf's payload by hash.
    pub fn get_leaf_payload(&self, hash: &str) -> MerkleResult<Option<String>> {
        let node = self.db.get_merkle_node(hash)?;
        Ok(node.and_then(|n| n.payload))
    }
}

/// Tree statistics.
#[derive(Debug, Clone)]
pub struct TreeStats {
    pub root_hash: Option<String>,
    pub height: u32,
    pub leaf_count: u32,
}

/// Compute SHA-256 hash of data.
pub fn hash_data(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Verify a Merkle proof (standalone function for external use).
pub fn verify_proof(proof: &MerkleProof) -> bool {
    let mut current_hash = proof.leaf_hash.clone();

    for (sibling_hash, sibling_on_right) in proof
        .proof_hashes
        .iter()
        .zip(proof.proof_directions.iter())
    {
        let combined = if *sibling_on_right {
            format!("{}{}", current_hash, sibling_hash)
        } else {
            format!("{}{}", sibling_hash, current_hash)
        };
        current_hash = hash_data(combined.as_bytes());
    }

    current_hash == proof.root_hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EncounterLineItem, ResolutionMethod};

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
    fn test_commit_single_encounter() {
        let db = setup_db();
        let tree = MerkleTree::new(&db);

        let encounter = make_encounter("draft-1");
        let commit = tree.commit_encounter(&encounter).unwrap();

        assert!(!commit.leaf_hash.is_empty());
        assert!(!commit.root_hash.is_empty());
        assert_eq!(commit.leaf_count, 1);
        assert_eq!(commit.tree_height, 1);

        // Single leaf tree: root = leaf
        assert_eq!(commit.leaf_hash, commit.root_hash);
    }

    #[test]
    fn test_commit_multiple_encounters() {
        let db = setup_db();
        let tree = MerkleTree::new(&db);

        let enc1 = make_encounter("draft-1");
        let enc2 = make_encounter("draft-2");
        let enc3 = make_encounter("draft-3");

        let commit1 = tree.commit_encounter(&enc1).unwrap();
        let commit2 = tree.commit_encounter(&enc2).unwrap();
        let commit3 = tree.commit_encounter(&enc3).unwrap();

        assert_eq!(commit1.leaf_count, 1);
        assert_eq!(commit2.leaf_count, 2);
        assert_eq!(commit3.leaf_count, 3);

        // Root should change with each commit
        assert_ne!(commit1.root_hash, commit2.root_hash);
        assert_ne!(commit2.root_hash, commit3.root_hash);

        // Height should increase appropriately
        assert_eq!(commit1.tree_height, 1);
        assert_eq!(commit2.tree_height, 2);
        assert_eq!(commit3.tree_height, 3); // 3 leaves need height 3
    }

    #[test]
    fn test_idempotent_commit() {
        let db = setup_db();
        let tree = MerkleTree::new(&db);

        let encounter = make_encounter("draft-1");

        let commit1 = tree.commit_encounter(&encounter).unwrap();
        let commit2 = tree.commit_encounter(&encounter).unwrap();

        // Should return same result, not add duplicate
        assert_eq!(commit1.leaf_hash, commit2.leaf_hash);
        assert_eq!(commit1.root_hash, commit2.root_hash);
        assert_eq!(commit1.leaf_count, commit2.leaf_count);
    }

    #[test]
    fn test_proof_generation_and_verification() {
        let db = setup_db();
        let tree = MerkleTree::new(&db);

        // Commit multiple encounters
        for i in 1..=5 {
            let enc = make_encounter(&format!("draft-{}", i));
            tree.commit_encounter(&enc).unwrap();
        }

        // Generate proof for first leaf
        let leaves = db.get_all_leaf_hashes().unwrap();
        let proof = tree.generate_proof(&leaves[0]).unwrap();

        // Verify proof
        assert!(tree.verify_proof(&proof));
        assert!(verify_proof(&proof));

        // Verify proof for middle leaf
        let proof_mid = tree.generate_proof(&leaves[2]).unwrap();
        assert!(verify_proof(&proof_mid));

        // Verify proof for last leaf
        let proof_last = tree.generate_proof(&leaves[4]).unwrap();
        assert!(verify_proof(&proof_last));
    }

    #[test]
    fn test_invalid_proof() {
        let db = setup_db();
        let tree = MerkleTree::new(&db);

        let enc = make_encounter("draft-1");
        tree.commit_encounter(&enc).unwrap();

        let leaves = db.get_all_leaf_hashes().unwrap();
        let mut proof = tree.generate_proof(&leaves[0]).unwrap();

        // Tamper with proof
        proof.leaf_hash = "tampered_hash".to_string();
        assert!(!verify_proof(&proof));
    }

    #[test]
    fn test_get_leaf_payload() {
        let db = setup_db();
        let tree = MerkleTree::new(&db);

        let encounter = make_encounter("draft-1");
        let commit = tree.commit_encounter(&encounter).unwrap();

        let payload = tree.get_leaf_payload(&commit.leaf_hash).unwrap().unwrap();
        let recovered: ReviewedEncounter = serde_json::from_str(&payload).unwrap();

        assert_eq!(recovered.draft_id, "draft-1");
        assert_eq!(recovered.reviewed_by, "Dr. Smith");
    }

    #[test]
    fn test_hash_deterministic() {
        let data = b"test data";
        let hash1 = hash_data(data);
        let hash2 = hash_data(data);
        assert_eq!(hash1, hash2);

        // SHA-256 produces 64 hex characters
        assert_eq!(hash1.len(), 64);
    }
}

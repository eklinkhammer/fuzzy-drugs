//! Merkle tree integration tests.

use fuzzy_drugs_core::db::Database;
use fuzzy_drugs_core::merkle::{verify_proof, MerkleTree};
use fuzzy_drugs_core::models::{EncounterLineItem, ResolutionMethod, ReviewedEncounter};

fn make_encounter(id: &str, patient: &str) -> ReviewedEncounter {
    ReviewedEncounter {
        draft_id: id.to_string(),
        patient_id: patient.to_string(),
        patient_server_id: None,
        transcript: format!("Transcript for encounter {}", id),
        line_items: vec![EncounterLineItem {
            sku: "SKU001".to_string(),
            name: "Test Drug 100mg".to_string(),
            quantity: 10.0,
            unit: "mg".to_string(),
            route: Some("PO".to_string()),
            original_mention: "10mg test drug PO".to_string(),
            resolution_method: ResolutionMethod::SystemApproved { confidence: 0.95 },
        }],
        reviewed_by: "Dr. Smith".to_string(),
        reviewed_at: chrono::Utc::now().to_rfc3339(),
        notes: None,
    }
}

#[test]
fn test_commit_single_encounter() {
    let db = Database::open_in_memory().unwrap();
    let tree = MerkleTree::new(&db);

    let encounter = make_encounter("draft-1", "patient-1");
    let commit = tree.commit_encounter(&encounter).unwrap();

    assert!(!commit.leaf_hash.is_empty());
    assert!(!commit.root_hash.is_empty());
    assert_eq!(commit.leaf_count, 1);
    assert_eq!(commit.tree_height, 1);

    // Single leaf: root == leaf
    assert_eq!(commit.leaf_hash, commit.root_hash);
}

#[test]
fn test_commit_multiple_encounters() {
    let db = Database::open_in_memory().unwrap();
    let tree = MerkleTree::new(&db);

    let mut commits = Vec::new();
    for i in 1..=5 {
        let encounter = make_encounter(&format!("draft-{}", i), "patient-1");
        let commit = tree.commit_encounter(&encounter).unwrap();
        commits.push(commit);
    }

    // Verify leaf counts
    assert_eq!(commits[0].leaf_count, 1);
    assert_eq!(commits[1].leaf_count, 2);
    assert_eq!(commits[4].leaf_count, 5);

    // Each commit should have a different root
    let roots: Vec<_> = commits.iter().map(|c| c.root_hash.clone()).collect();
    let unique_roots: std::collections::HashSet<_> = roots.iter().collect();
    assert_eq!(unique_roots.len(), 5);
}

#[test]
fn test_idempotent_commit() {
    let db = Database::open_in_memory().unwrap();
    let tree = MerkleTree::new(&db);

    let encounter = make_encounter("draft-1", "patient-1");

    let commit1 = tree.commit_encounter(&encounter).unwrap();
    let commit2 = tree.commit_encounter(&encounter).unwrap();

    // Same encounter should produce same hash
    assert_eq!(commit1.leaf_hash, commit2.leaf_hash);
    assert_eq!(commit1.root_hash, commit2.root_hash);
    assert_eq!(commit1.leaf_count, commit2.leaf_count);
}

#[test]
fn test_proof_generation() {
    let db = Database::open_in_memory().unwrap();
    let tree = MerkleTree::new(&db);

    // Commit several encounters
    for i in 1..=5 {
        let encounter = make_encounter(&format!("draft-{}", i), "patient-1");
        tree.commit_encounter(&encounter).unwrap();
    }

    // Generate proof for each leaf
    let leaves = db.get_all_leaf_hashes().unwrap();
    for leaf_hash in &leaves {
        let proof = tree.generate_proof(leaf_hash).unwrap();

        // Proof should be valid
        assert!(verify_proof(&proof));
        assert_eq!(proof.leaf_hash, *leaf_hash);
    }
}

#[test]
fn test_proof_verification_fails_on_tampering() {
    let db = Database::open_in_memory().unwrap();
    let tree = MerkleTree::new(&db);

    let encounter = make_encounter("draft-1", "patient-1");
    let commit = tree.commit_encounter(&encounter).unwrap();

    let mut proof = commit.proof;

    // Tamper with leaf hash
    let original_leaf = proof.leaf_hash.clone();
    proof.leaf_hash = "tampered_hash".to_string();
    assert!(!verify_proof(&proof));

    // Restore and tamper with root
    proof.leaf_hash = original_leaf;
    proof.root_hash = "tampered_root".to_string();
    assert!(!verify_proof(&proof));
}

#[test]
fn test_tree_stats() {
    let db = Database::open_in_memory().unwrap();
    let tree = MerkleTree::new(&db);

    // Empty tree
    let stats = tree.get_stats().unwrap();
    assert!(stats.root_hash.is_none());
    assert_eq!(stats.leaf_count, 0);

    // After commits
    for i in 1..=3 {
        let encounter = make_encounter(&format!("draft-{}", i), "patient-1");
        tree.commit_encounter(&encounter).unwrap();
    }

    let stats = tree.get_stats().unwrap();
    assert!(stats.root_hash.is_some());
    assert_eq!(stats.leaf_count, 3);
}

#[test]
fn test_get_leaf_payload() {
    let db = Database::open_in_memory().unwrap();
    let tree = MerkleTree::new(&db);

    let encounter = make_encounter("draft-1", "patient-1");
    let commit = tree.commit_encounter(&encounter).unwrap();

    let payload = tree.get_leaf_payload(&commit.leaf_hash).unwrap().unwrap();
    let recovered: ReviewedEncounter = serde_json::from_str(&payload).unwrap();

    assert_eq!(recovered.draft_id, "draft-1");
    assert_eq!(recovered.patient_id, "patient-1");
    assert_eq!(recovered.line_items.len(), 1);
}

#[test]
fn test_proof_for_multiple_patients() {
    let db = Database::open_in_memory().unwrap();
    let tree = MerkleTree::new(&db);

    // Commit encounters for multiple patients
    for patient in 1..=3 {
        for encounter in 1..=3 {
            let enc = make_encounter(
                &format!("draft-p{}-e{}", patient, encounter),
                &format!("patient-{}", patient),
            );
            tree.commit_encounter(&enc).unwrap();
        }
    }

    // Verify all proofs
    let leaves = db.get_all_leaf_hashes().unwrap();
    assert_eq!(leaves.len(), 9);

    for leaf_hash in &leaves {
        let proof = tree.generate_proof(leaf_hash).unwrap();
        assert!(verify_proof(&proof));
    }
}

#[test]
fn test_deterministic_hashing() {
    let db1 = Database::open_in_memory().unwrap();
    let db2 = Database::open_in_memory().unwrap();

    let tree1 = MerkleTree::new(&db1);
    let tree2 = MerkleTree::new(&db2);

    // Same encounter should produce same hash in different databases
    let encounter = ReviewedEncounter {
        draft_id: "draft-1".to_string(),
        patient_id: "patient-1".to_string(),
        patient_server_id: None,
        transcript: "Test transcript".to_string(),
        line_items: vec![],
        reviewed_by: "Dr. Smith".to_string(),
        reviewed_at: "2024-01-15T10:00:00Z".to_string(), // Fixed timestamp
        notes: None,
    };

    let commit1 = tree1.commit_encounter(&encounter).unwrap();
    let commit2 = tree2.commit_encounter(&encounter).unwrap();

    assert_eq!(commit1.leaf_hash, commit2.leaf_hash);
}

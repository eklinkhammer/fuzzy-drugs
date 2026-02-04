//! Merkle tree database operations.

use rusqlite::{params, OptionalExtension};

use super::{Database, DbResult};

/// Merkle tree node types.
#[derive(Debug, Clone, PartialEq)]
pub enum MerkleNodeType {
    Leaf,
    Internal,
}

impl MerkleNodeType {
    #[allow(dead_code)]
    fn as_str(&self) -> &'static str {
        match self {
            MerkleNodeType::Leaf => "leaf",
            MerkleNodeType::Internal => "internal",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "leaf" => Some(MerkleNodeType::Leaf),
            "internal" => Some(MerkleNodeType::Internal),
            _ => None,
        }
    }
}

/// A Merkle tree node.
#[derive(Debug, Clone)]
pub struct MerkleNode {
    pub hash: String,
    pub node_type: MerkleNodeType,
    pub left_child: Option<String>,
    pub right_child: Option<String>,
    pub payload: Option<String>,
    pub created_at: String,
}

/// Current Merkle tree root state.
#[derive(Debug, Clone)]
pub struct MerkleRootState {
    pub root_hash: Option<String>,
    pub tree_height: u32,
    pub leaf_count: u32,
    pub updated_at: String,
}

impl Database {
    /// Insert a leaf node.
    pub fn insert_merkle_leaf(&self, hash: &str, payload: &str) -> DbResult<()> {
        self.conn.execute(
            "INSERT INTO merkle_nodes (hash, node_type, payload) VALUES (?, 'leaf', ?)",
            params![hash, payload],
        )?;
        Ok(())
    }

    /// Insert an internal node.
    pub fn insert_merkle_internal(
        &self,
        hash: &str,
        left_child: &str,
        right_child: Option<&str>,
    ) -> DbResult<()> {
        self.conn.execute(
            "INSERT INTO merkle_nodes (hash, node_type, left_child, right_child) VALUES (?, 'internal', ?, ?)",
            params![hash, left_child, right_child],
        )?;
        Ok(())
    }

    /// Get a Merkle node by hash.
    pub fn get_merkle_node(&self, hash: &str) -> DbResult<Option<MerkleNode>> {
        self.conn
            .query_row(
                r#"
                SELECT hash, node_type, left_child, right_child, payload, created_at
                FROM merkle_nodes
                WHERE hash = ?
                "#,
                [hash],
                |row| {
                    let node_type_str: String = row.get(1)?;
                    Ok(MerkleNode {
                        hash: row.get(0)?,
                        node_type: MerkleNodeType::from_str(&node_type_str)
                            .unwrap_or(MerkleNodeType::Leaf),
                        left_child: row.get(2)?,
                        right_child: row.get(3)?,
                        payload: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Check if a node exists.
    pub fn merkle_node_exists(&self, hash: &str) -> DbResult<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM merkle_nodes WHERE hash = ?",
            [hash],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get current root state.
    pub fn get_merkle_root(&self) -> DbResult<MerkleRootState> {
        self.conn
            .query_row(
                "SELECT root_hash, tree_height, leaf_count, updated_at FROM merkle_root WHERE id = 1",
                [],
                |row| {
                    Ok(MerkleRootState {
                        root_hash: row.get(0)?,
                        tree_height: row.get(1)?,
                        leaf_count: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    /// Update root state atomically.
    pub fn update_merkle_root(
        &self,
        root_hash: &str,
        tree_height: u32,
        leaf_count: u32,
    ) -> DbResult<()> {
        self.conn.execute(
            r#"
            UPDATE merkle_root
            SET root_hash = ?, tree_height = ?, leaf_count = ?, updated_at = datetime('now')
            WHERE id = 1
            "#,
            params![root_hash, tree_height, leaf_count],
        )?;
        Ok(())
    }

    /// Get all leaf hashes in insertion order.
    pub fn get_all_leaf_hashes(&self) -> DbResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT hash FROM merkle_nodes WHERE node_type = 'leaf' ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get nodes created after a given timestamp.
    pub fn get_nodes_since(&self, since: &str) -> DbResult<Vec<MerkleNode>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT hash, node_type, left_child, right_child, payload, created_at
            FROM merkle_nodes
            WHERE created_at > ?
            ORDER BY created_at
            "#,
        )?;

        let rows = stmt.query_map([since], |row| {
            let node_type_str: String = row.get(1)?;
            Ok(MerkleNode {
                hash: row.get(0)?,
                node_type: MerkleNodeType::from_str(&node_type_str)
                    .unwrap_or(MerkleNodeType::Leaf),
                left_child: row.get(2)?,
                right_child: row.get(3)?,
                payload: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get nodes by list of hashes (for sync).
    pub fn get_nodes_by_hashes(&self, hashes: &[String]) -> DbResult<Vec<MerkleNode>> {
        if hashes.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders: Vec<&str> = hashes.iter().map(|_| "?").collect();
        let sql = format!(
            r#"
            SELECT hash, node_type, left_child, right_child, payload, created_at
            FROM merkle_nodes
            WHERE hash IN ({})
            "#,
            placeholders.join(", ")
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(hashes.iter()),
            |row| {
                let node_type_str: String = row.get(1)?;
                Ok(MerkleNode {
                    hash: row.get(0)?,
                    node_type: MerkleNodeType::from_str(&node_type_str)
                        .unwrap_or(MerkleNodeType::Leaf),
                    left_child: row.get(2)?,
                    right_child: row.get(3)?,
                    payload: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get sync state value.
    pub fn get_sync_state(&self, key: &str) -> DbResult<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM sync_state WHERE key = ?",
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    /// Set sync state value.
    pub fn set_sync_state(&self, key: &str, value: &str) -> DbResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sync_state (key, value, updated_at) VALUES (?, ?, datetime('now'))",
            params![key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn test_insert_and_get_leaf() {
        let db = setup_db();

        db.insert_merkle_leaf("hash123", r#"{"test": "payload"}"#)
            .unwrap();

        let node = db.get_merkle_node("hash123").unwrap().unwrap();
        assert_eq!(node.hash, "hash123");
        assert_eq!(node.node_type, MerkleNodeType::Leaf);
        assert_eq!(node.payload, Some(r#"{"test": "payload"}"#.to_string()));
        assert!(node.left_child.is_none());
        assert!(node.right_child.is_none());
    }

    #[test]
    fn test_insert_and_get_internal() {
        let db = setup_db();

        // First insert leaf nodes
        db.insert_merkle_leaf("leaf1", "payload1").unwrap();
        db.insert_merkle_leaf("leaf2", "payload2").unwrap();

        // Then insert internal node
        db.insert_merkle_internal("internal1", "leaf1", Some("leaf2"))
            .unwrap();

        let node = db.get_merkle_node("internal1").unwrap().unwrap();
        assert_eq!(node.node_type, MerkleNodeType::Internal);
        assert_eq!(node.left_child, Some("leaf1".to_string()));
        assert_eq!(node.right_child, Some("leaf2".to_string()));
        assert!(node.payload.is_none());
    }

    #[test]
    fn test_root_state() {
        let db = setup_db();

        // Initial state
        let state = db.get_merkle_root().unwrap();
        assert!(state.root_hash.is_none());
        assert_eq!(state.leaf_count, 0);

        // Insert a leaf and update root
        db.insert_merkle_leaf("leaf1", "payload").unwrap();
        db.update_merkle_root("leaf1", 1, 1).unwrap();

        let state = db.get_merkle_root().unwrap();
        assert_eq!(state.root_hash, Some("leaf1".to_string()));
        assert_eq!(state.tree_height, 1);
        assert_eq!(state.leaf_count, 1);
    }

    #[test]
    fn test_node_exists() {
        let db = setup_db();

        assert!(!db.merkle_node_exists("nonexistent").unwrap());

        db.insert_merkle_leaf("exists", "payload").unwrap();
        assert!(db.merkle_node_exists("exists").unwrap());
    }

    #[test]
    fn test_get_all_leaf_hashes() {
        let db = setup_db();

        db.insert_merkle_leaf("leaf1", "p1").unwrap();
        db.insert_merkle_leaf("leaf2", "p2").unwrap();
        db.insert_merkle_leaf("leaf3", "p3").unwrap();

        let hashes = db.get_all_leaf_hashes().unwrap();
        assert_eq!(hashes.len(), 3);
        // Should be in insertion order
        assert_eq!(hashes[0], "leaf1");
        assert_eq!(hashes[1], "leaf2");
        assert_eq!(hashes[2], "leaf3");
    }

    #[test]
    fn test_sync_state() {
        let db = setup_db();

        // Default values from schema
        let catalog_sync = db.get_sync_state("catalog_last_sync").unwrap();
        assert_eq!(catalog_sync, Some("".to_string()));

        // Update
        db.set_sync_state("catalog_last_sync", "2024-01-15T10:00:00Z")
            .unwrap();
        let catalog_sync = db.get_sync_state("catalog_last_sync").unwrap();
        assert_eq!(catalog_sync, Some("2024-01-15T10:00:00Z".to_string()));
    }
}

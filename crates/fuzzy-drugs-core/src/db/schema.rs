//! SQLite schema definition.

/// Complete database schema for fuzzy-drugs.
pub const SCHEMA: &str = r#"
-- Enable foreign keys
PRAGMA foreign_keys = ON;

-- ============================================================================
-- Inventory Catalog
-- ============================================================================

CREATE TABLE IF NOT EXISTS inventory_catalog (
    sku TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    aliases TEXT NOT NULL DEFAULT '[]',           -- JSON array of strings
    concentration TEXT,
    package_size TEXT,
    species TEXT NOT NULL DEFAULT '[]',           -- JSON array of strings
    routes TEXT NOT NULL DEFAULT '[]',            -- JSON array of strings
    dose_range TEXT,                              -- JSON object {min, max, unit}
    active INTEGER NOT NULL DEFAULT 1,
    server_id TEXT,
    last_synced TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- FTS5 virtual table for full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS inventory_catalog_fts USING fts5(
    sku,
    name,
    aliases,
    content='inventory_catalog',
    content_rowid='rowid'
);

-- Triggers to keep FTS5 in sync with main table
CREATE TRIGGER IF NOT EXISTS inventory_catalog_ai AFTER INSERT ON inventory_catalog BEGIN
    INSERT INTO inventory_catalog_fts(rowid, sku, name, aliases)
    VALUES (new.rowid, new.sku, new.name, new.aliases);
END;

CREATE TRIGGER IF NOT EXISTS inventory_catalog_ad AFTER DELETE ON inventory_catalog BEGIN
    INSERT INTO inventory_catalog_fts(inventory_catalog_fts, rowid, sku, name, aliases)
    VALUES ('delete', old.rowid, old.sku, old.name, old.aliases);
END;

CREATE TRIGGER IF NOT EXISTS inventory_catalog_au AFTER UPDATE ON inventory_catalog BEGIN
    INSERT INTO inventory_catalog_fts(inventory_catalog_fts, rowid, sku, name, aliases)
    VALUES ('delete', old.rowid, old.sku, old.name, old.aliases);
    INSERT INTO inventory_catalog_fts(rowid, sku, name, aliases)
    VALUES (new.rowid, new.sku, new.name, new.aliases);
END;

-- Index for server sync
CREATE INDEX IF NOT EXISTS idx_catalog_server_id ON inventory_catalog(server_id);
CREATE INDEX IF NOT EXISTS idx_catalog_last_synced ON inventory_catalog(last_synced);

-- ============================================================================
-- Patients
-- ============================================================================

CREATE TABLE IF NOT EXISTS patients (
    local_id TEXT PRIMARY KEY,
    server_id TEXT,                              -- NULL until first sync
    name TEXT NOT NULL,
    species TEXT NOT NULL,
    breed TEXT,
    weight_kg REAL,
    date_of_birth TEXT,
    owner_name TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_patients_server_id ON patients(server_id);
CREATE INDEX IF NOT EXISTS idx_patients_name ON patients(name);

-- ============================================================================
-- Encounter Drafts (Staging Area - Mutable)
-- ============================================================================

CREATE TABLE IF NOT EXISTS encounter_drafts (
    draft_id TEXT PRIMARY KEY,
    patient_id TEXT NOT NULL REFERENCES patients(local_id),
    transcript TEXT NOT NULL DEFAULT '',
    resolved_items TEXT NOT NULL DEFAULT '[]',   -- JSON array of ResolvedItem
    status TEXT NOT NULL DEFAULT 'recording',    -- recording, transcribed, pending_review, reviewed, committed
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_drafts_patient ON encounter_drafts(patient_id);
CREATE INDEX IF NOT EXISTS idx_drafts_status ON encounter_drafts(status);

-- ============================================================================
-- Merkle Tree (Append-Only - Immutable after creation)
-- ============================================================================

-- Merkle tree nodes
CREATE TABLE IF NOT EXISTS merkle_nodes (
    hash TEXT PRIMARY KEY,                       -- SHA-256 of content or children
    node_type TEXT NOT NULL CHECK (node_type IN ('leaf', 'internal')),
    left_child TEXT REFERENCES merkle_nodes(hash),
    right_child TEXT REFERENCES merkle_nodes(hash),
    payload TEXT,                                -- JSON content (leaf only)
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Ensure leaves have payload, internals have children
CREATE TRIGGER IF NOT EXISTS merkle_nodes_check_leaf BEFORE INSERT ON merkle_nodes
WHEN new.node_type = 'leaf'
BEGIN
    SELECT CASE
        WHEN new.payload IS NULL THEN
            RAISE(ABORT, 'Leaf nodes must have payload')
        WHEN new.left_child IS NOT NULL OR new.right_child IS NOT NULL THEN
            RAISE(ABORT, 'Leaf nodes cannot have children')
    END;
END;

CREATE TRIGGER IF NOT EXISTS merkle_nodes_check_internal BEFORE INSERT ON merkle_nodes
WHEN new.node_type = 'internal'
BEGIN
    SELECT CASE
        WHEN new.left_child IS NULL THEN
            RAISE(ABORT, 'Internal nodes must have left child')
        WHEN new.payload IS NOT NULL THEN
            RAISE(ABORT, 'Internal nodes cannot have payload')
    END;
END;

-- Index for efficient tree traversal
CREATE INDEX IF NOT EXISTS idx_merkle_children ON merkle_nodes(left_child, right_child);
CREATE INDEX IF NOT EXISTS idx_merkle_type ON merkle_nodes(node_type);

-- Current root (single row, updated atomically)
CREATE TABLE IF NOT EXISTS merkle_root (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    root_hash TEXT REFERENCES merkle_nodes(hash),
    tree_height INTEGER NOT NULL DEFAULT 0,
    leaf_count INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Initialize with empty tree state
INSERT OR IGNORE INTO merkle_root (id, root_hash, tree_height, leaf_count)
VALUES (1, NULL, 0, 0);

-- ============================================================================
-- Sync State
-- ============================================================================

CREATE TABLE IF NOT EXISTS sync_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Track last successful sync timestamps
INSERT OR IGNORE INTO sync_state (key, value) VALUES ('catalog_last_sync', '');
INSERT OR IGNORE INTO sync_state (key, value) VALUES ('encounters_last_sync', '');
INSERT OR IGNORE INTO sync_state (key, value) VALUES ('last_synced_root', '');
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_schema_valid() {
        let conn = Connection::open_in_memory().unwrap();
        let result = conn.execute_batch(SCHEMA);
        assert!(result.is_ok(), "Schema should be valid SQL: {:?}", result);
    }

    #[test]
    fn test_fts_trigger() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        // Insert into catalog
        conn.execute(
            "INSERT INTO inventory_catalog (sku, name, aliases) VALUES (?, ?, ?)",
            ["SKU001", "Carprofen 100mg", r#"["rimadyl"]"#],
        )
        .unwrap();

        // Search via FTS
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM inventory_catalog_fts WHERE inventory_catalog_fts MATCH 'carprofen'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Search aliases
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM inventory_catalog_fts WHERE inventory_catalog_fts MATCH 'rimadyl'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_merkle_leaf_constraint() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        // Leaf without payload should fail
        let result = conn.execute(
            "INSERT INTO merkle_nodes (hash, node_type) VALUES ('abc', 'leaf')",
            [],
        );
        assert!(result.is_err());

        // Leaf with children should fail
        let result = conn.execute(
            "INSERT INTO merkle_nodes (hash, node_type, left_child, payload) VALUES ('abc', 'leaf', 'def', 'test')",
            [],
        );
        assert!(result.is_err());

        // Valid leaf should succeed
        let result = conn.execute(
            "INSERT INTO merkle_nodes (hash, node_type, payload) VALUES ('abc', 'leaf', 'test payload')",
            [],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_merkle_internal_constraint() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        // First create leaf nodes
        conn.execute(
            "INSERT INTO merkle_nodes (hash, node_type, payload) VALUES ('leaf1', 'leaf', 'payload1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO merkle_nodes (hash, node_type, payload) VALUES ('leaf2', 'leaf', 'payload2')",
            [],
        )
        .unwrap();

        // Internal without left_child should fail
        let result = conn.execute(
            "INSERT INTO merkle_nodes (hash, node_type) VALUES ('int1', 'internal')",
            [],
        );
        assert!(result.is_err());

        // Internal with payload should fail
        let result = conn.execute(
            "INSERT INTO merkle_nodes (hash, node_type, left_child, payload) VALUES ('int1', 'internal', 'leaf1', 'test')",
            [],
        );
        assert!(result.is_err());

        // Valid internal should succeed
        let result = conn.execute(
            "INSERT INTO merkle_nodes (hash, node_type, left_child, right_child) VALUES ('int1', 'internal', 'leaf1', 'leaf2')",
            [],
        );
        assert!(result.is_ok());
    }
}

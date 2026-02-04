//! Database layer for fuzzy-drugs.

mod schema;
mod catalog;
mod patients;
mod drafts;
mod merkle;

pub use schema::*;
#[allow(unused_imports)]
pub use catalog::*;
#[allow(unused_imports)]
pub use patients::*;
#[allow(unused_imports)]
pub use drafts::*;
pub use merkle::*;

use rusqlite::Connection;
use std::path::Path;
use thiserror::Error;

/// Database errors.
#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Record not found: {0}")]
    NotFound(String),

    #[error("Constraint violation: {0}")]
    Constraint(String),
}

pub type DbResult<T> = Result<T, DbError>;

/// Database connection wrapper.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open database at path, creating if needed.
    pub fn open<P: AsRef<Path>>(path: P) -> DbResult<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    /// Create in-memory database (for testing).
    pub fn open_in_memory() -> DbResult<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    /// Initialize schema.
    fn initialize(&self) -> DbResult<()> {
        self.conn.execute_batch(SCHEMA)?;
        Ok(())
    }

    /// Get raw connection (for advanced queries).
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Begin a transaction.
    pub fn transaction(&mut self) -> DbResult<rusqlite::Transaction<'_>> {
        Ok(self.conn.transaction()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_in_memory() {
        let db = Database::open_in_memory();
        assert!(db.is_ok());
    }

    #[test]
    fn test_schema_initialized() {
        let db = Database::open_in_memory().unwrap();

        // Check that tables exist
        let tables: Vec<String> = db
            .conn()
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"inventory_catalog".to_string()));
        assert!(tables.contains(&"patients".to_string()));
        assert!(tables.contains(&"encounter_drafts".to_string()));
        assert!(tables.contains(&"merkle_nodes".to_string()));
        assert!(tables.contains(&"merkle_root".to_string()));
    }
}

//! Patient database operations.

use rusqlite::{params, OptionalExtension};

use super::{Database, DbResult};
use crate::models::Patient;

impl Database {
    /// Insert a new patient.
    pub fn insert_patient(&self, patient: &Patient) -> DbResult<()> {
        self.conn.execute(
            r#"
            INSERT INTO patients (
                local_id, server_id, name, species, breed, weight_kg,
                date_of_birth, owner_name, notes, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                patient.local_id,
                patient.server_id,
                patient.name,
                patient.species,
                patient.breed,
                patient.weight_kg,
                patient.date_of_birth,
                patient.owner_name,
                patient.notes,
                patient.created_at,
                patient.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Update an existing patient.
    pub fn update_patient(&self, patient: &Patient) -> DbResult<bool> {
        let rows_affected = self.conn.execute(
            r#"
            UPDATE patients SET
                server_id = ?2,
                name = ?3,
                species = ?4,
                breed = ?5,
                weight_kg = ?6,
                date_of_birth = ?7,
                owner_name = ?8,
                notes = ?9,
                updated_at = datetime('now')
            WHERE local_id = ?1
            "#,
            params![
                patient.local_id,
                patient.server_id,
                patient.name,
                patient.species,
                patient.breed,
                patient.weight_kg,
                patient.date_of_birth,
                patient.owner_name,
                patient.notes,
            ],
        )?;
        Ok(rows_affected > 0)
    }

    /// Get a patient by local ID.
    pub fn get_patient(&self, local_id: &str) -> DbResult<Option<Patient>> {
        self.conn
            .query_row(
                r#"
                SELECT local_id, server_id, name, species, breed, weight_kg,
                       date_of_birth, owner_name, notes, created_at, updated_at
                FROM patients
                WHERE local_id = ?
                "#,
                [local_id],
                |row| {
                    Ok(Patient {
                        local_id: row.get(0)?,
                        server_id: row.get(1)?,
                        name: row.get(2)?,
                        species: row.get(3)?,
                        breed: row.get(4)?,
                        weight_kg: row.get(5)?,
                        date_of_birth: row.get(6)?,
                        owner_name: row.get(7)?,
                        notes: row.get(8)?,
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Get a patient by server ID.
    pub fn get_patient_by_server_id(&self, server_id: &str) -> DbResult<Option<Patient>> {
        self.conn
            .query_row(
                r#"
                SELECT local_id, server_id, name, species, breed, weight_kg,
                       date_of_birth, owner_name, notes, created_at, updated_at
                FROM patients
                WHERE server_id = ?
                "#,
                [server_id],
                |row| {
                    Ok(Patient {
                        local_id: row.get(0)?,
                        server_id: row.get(1)?,
                        name: row.get(2)?,
                        species: row.get(3)?,
                        breed: row.get(4)?,
                        weight_kg: row.get(5)?,
                        date_of_birth: row.get(6)?,
                        owner_name: row.get(7)?,
                        notes: row.get(8)?,
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Search patients by name (prefix match).
    pub fn search_patients(&self, query: &str, limit: usize) -> DbResult<Vec<Patient>> {
        let pattern = format!("{}%", query);
        let mut stmt = self.conn.prepare(
            r#"
            SELECT local_id, server_id, name, species, breed, weight_kg,
                   date_of_birth, owner_name, notes, created_at, updated_at
            FROM patients
            WHERE name LIKE ?
            ORDER BY name
            LIMIT ?
            "#,
        )?;

        let rows = stmt.query_map(params![pattern, limit as i64], |row| {
            Ok(Patient {
                local_id: row.get(0)?,
                server_id: row.get(1)?,
                name: row.get(2)?,
                species: row.get(3)?,
                breed: row.get(4)?,
                weight_kg: row.get(5)?,
                date_of_birth: row.get(6)?,
                owner_name: row.get(7)?,
                notes: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// List all patients.
    pub fn list_patients(&self) -> DbResult<Vec<Patient>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT local_id, server_id, name, species, breed, weight_kg,
                   date_of_birth, owner_name, notes, created_at, updated_at
            FROM patients
            ORDER BY name
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Patient {
                local_id: row.get(0)?,
                server_id: row.get(1)?,
                name: row.get(2)?,
                species: row.get(3)?,
                breed: row.get(4)?,
                weight_kg: row.get(5)?,
                date_of_birth: row.get(6)?,
                owner_name: row.get(7)?,
                notes: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Delete a patient.
    pub fn delete_patient(&self, local_id: &str) -> DbResult<bool> {
        let rows_affected = self
            .conn
            .execute("DELETE FROM patients WHERE local_id = ?", [local_id])?;
        Ok(rows_affected > 0)
    }

    /// Link local patient to server ID after first sync.
    pub fn link_patient_server_id(&self, local_id: &str, server_id: &str) -> DbResult<bool> {
        let rows_affected = self.conn.execute(
            "UPDATE patients SET server_id = ?, updated_at = datetime('now') WHERE local_id = ?",
            [server_id, local_id],
        )?;
        Ok(rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn test_insert_and_get() {
        let db = setup_db();

        let mut patient = Patient::new("Max".into(), "canine".into());
        patient.breed = Some("Golden Retriever".into());
        patient.weight_kg = Some(30.0);

        db.insert_patient(&patient).unwrap();

        let retrieved = db.get_patient(&patient.local_id).unwrap().unwrap();
        assert_eq!(retrieved.name, "Max");
        assert_eq!(retrieved.species, "canine");
        assert_eq!(retrieved.breed, Some("Golden Retriever".into()));
        assert_eq!(retrieved.weight_kg, Some(30.0));
    }

    #[test]
    fn test_update_patient() {
        let db = setup_db();

        let mut patient = Patient::new("Max".into(), "canine".into());
        db.insert_patient(&patient).unwrap();

        patient.weight_kg = Some(32.0);
        patient.notes = Some("Good boy".into());
        db.update_patient(&patient).unwrap();

        let retrieved = db.get_patient(&patient.local_id).unwrap().unwrap();
        assert_eq!(retrieved.weight_kg, Some(32.0));
        assert_eq!(retrieved.notes, Some("Good boy".into()));
    }

    #[test]
    fn test_search_patients() {
        let db = setup_db();

        let patient1 = Patient::new("Max".into(), "canine".into());
        let patient2 = Patient::new("Maxine".into(), "feline".into());
        let patient3 = Patient::new("Luna".into(), "canine".into());

        db.insert_patient(&patient1).unwrap();
        db.insert_patient(&patient2).unwrap();
        db.insert_patient(&patient3).unwrap();

        let results = db.search_patients("Max", 10).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|p| p.name == "Max"));
        assert!(results.iter().any(|p| p.name == "Maxine"));
    }

    #[test]
    fn test_link_server_id() {
        let db = setup_db();

        let patient = Patient::new("Max".into(), "canine".into());
        db.insert_patient(&patient).unwrap();

        assert!(!patient.is_synced());

        db.link_patient_server_id(&patient.local_id, "server-123")
            .unwrap();

        let retrieved = db.get_patient(&patient.local_id).unwrap().unwrap();
        assert_eq!(retrieved.server_id, Some("server-123".into()));

        // Should also be findable by server ID
        let by_server = db
            .get_patient_by_server_id("server-123")
            .unwrap()
            .unwrap();
        assert_eq!(by_server.local_id, patient.local_id);
    }
}

//! Patient models.

use serde::{Deserialize, Serialize};

/// A patient record with dual-ID support for offline-first sync.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Patient {
    /// Local UUID - always present, generated locally
    pub local_id: String,
    /// PIMS server ID - null until first sync
    pub server_id: Option<String>,
    /// Patient name
    pub name: String,
    /// Species (e.g., "canine", "feline", "equine")
    pub species: String,
    /// Breed
    pub breed: Option<String>,
    /// Weight in kg (important for dose validation)
    pub weight_kg: Option<f64>,
    /// Date of birth
    pub date_of_birth: Option<String>,
    /// Owner/client name
    pub owner_name: Option<String>,
    /// Additional notes
    pub notes: Option<String>,
    /// Creation timestamp
    pub created_at: String,
    /// Last update timestamp
    pub updated_at: String,
}

impl Patient {
    /// Create a new patient with required fields.
    pub fn new(name: String, species: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            local_id: uuid::Uuid::new_v4().to_string(),
            server_id: None,
            name,
            species,
            breed: None,
            weight_kg: None,
            date_of_birth: None,
            owner_name: None,
            notes: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Check if this patient has been synced to server.
    pub fn is_synced(&self) -> bool {
        self.server_id.is_some()
    }

    /// Get the canonical species name (lowercase).
    pub fn canonical_species(&self) -> String {
        self.species.to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_patient() {
        let patient = Patient::new("Max".into(), "canine".into());
        assert_eq!(patient.name, "Max");
        assert_eq!(patient.species, "canine");
        assert!(!patient.is_synced());
        assert_eq!(patient.local_id.len(), 36); // UUID format
    }

    #[test]
    fn test_canonical_species() {
        let patient = Patient::new("Max".into(), "Canine".into());
        assert_eq!(patient.canonical_species(), "canine");
    }
}

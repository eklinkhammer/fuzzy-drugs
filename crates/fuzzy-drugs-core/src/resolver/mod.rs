//! Semantic resolver for drug mentions.
//!
//! Pipeline: NER Extraction → Normalization → Disambiguation → Review Queue

mod normalizer;
mod disambiguator;

pub use normalizer::*;
pub use disambiguator::*;

use crate::db::Database;
use crate::models::{DrugMention, ResolvedItem, ResolutionStatus};
use thiserror::Error;

/// Resolver errors.
#[derive(Error, Debug)]
pub enum ResolverError {
    #[error("Database error: {0}")]
    Database(#[from] crate::db::DbError),

    #[error("No candidates found for: {0}")]
    NoCandidates(String),
}

pub type ResolverResult<T> = Result<T, ResolverError>;

/// Main resolver that coordinates the full pipeline.
pub struct Resolver<'a> {
    #[allow(dead_code)]
    db: &'a Database,
    normalizer: Normalizer,
    disambiguator: Disambiguator<'a>,
}

impl<'a> Resolver<'a> {
    /// Create a new resolver.
    pub fn new(db: &'a Database) -> Self {
        Self {
            db,
            normalizer: Normalizer::new(),
            disambiguator: Disambiguator::new(db),
        }
    }

    /// Resolve a drug mention to SKU candidates.
    pub fn resolve(&self, mention: &DrugMention, patient_species: Option<&str>, patient_weight_kg: Option<f64>) -> ResolverResult<ResolvedItem> {
        // Step 1: Normalize the mention
        let normalized = self.normalizer.normalize(mention);

        // Step 2: Disambiguate to find best SKU matches
        let (top_candidate, alternatives) = self.disambiguator.disambiguate(
            &normalized,
            patient_species,
            patient_weight_kg,
        )?;

        // Step 3: Create resolved item (always pending review)
        Ok(ResolvedItem {
            mention: normalized,
            top_candidate,
            alternatives,
            status: ResolutionStatus::PendingReview,
        })
    }

    /// Resolve multiple mentions from a transcript.
    pub fn resolve_all(
        &self,
        mentions: &[DrugMention],
        patient_species: Option<&str>,
        patient_weight_kg: Option<f64>,
    ) -> Vec<ResolverResult<ResolvedItem>> {
        mentions
            .iter()
            .map(|m| self.resolve(m, patient_species, patient_weight_kg))
            .collect()
    }

    /// Get the normalizer for direct access.
    pub fn normalizer(&self) -> &Normalizer {
        &self.normalizer
    }

    /// Get the disambiguator for direct access.
    pub fn disambiguator(&self) -> &Disambiguator<'a> {
        &self.disambiguator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CatalogItem;

    fn setup_db_with_catalog() -> Database {
        let db = Database::open_in_memory().unwrap();

        // Add some test catalog items
        let mut item1 = CatalogItem::new("CARP-100".into(), "Carprofen 100mg tablets".into());
        item1.aliases = vec!["rimadyl".into(), "novox".into()];
        item1.species = vec!["canine".into()];
        item1.routes = vec!["PO".into()];
        db.upsert_catalog_item(&item1).unwrap();

        let mut item2 = CatalogItem::new("MELOX-15".into(), "Meloxicam 1.5mg/mL oral suspension".into());
        item2.aliases = vec!["metacam".into()];
        item2.species = vec!["canine".into(), "feline".into()];
        item2.routes = vec!["PO".into()];
        db.upsert_catalog_item(&item2).unwrap();

        let mut item3 = CatalogItem::new("ACE-10".into(), "Acepromazine 10mg/mL injection".into());
        item3.aliases = vec!["ace".into(), "promace".into()];
        item3.species = vec!["canine".into(), "feline".into(), "equine".into()];
        item3.routes = vec!["IV".into(), "IM".into(), "SQ".into()];
        db.upsert_catalog_item(&item3).unwrap();

        db
    }

    #[test]
    fn test_resolve_by_alias() {
        let db = setup_db_with_catalog();
        let resolver = Resolver::new(&db);

        let mention = DrugMention {
            raw_text: "Give rimadyl 100mg PO".into(),
            drug_name: "rimadyl".into(),
            dose: Some(100.0),
            unit: Some("mg".into()),
            route: Some("PO".into()),
            species: None,
            start_offset: 5,
            end_offset: 21,
        };

        let result = resolver.resolve(&mention, Some("canine"), Some(30.0)).unwrap();

        assert_eq!(result.top_candidate.sku, "CARP-100");
        assert!(result.top_candidate.confidence > 0.5);
        assert!(matches!(result.status, ResolutionStatus::PendingReview));
    }

    #[test]
    fn test_resolve_normalized_alias() {
        let db = setup_db_with_catalog();
        let resolver = Resolver::new(&db);

        // Use "ace" which should expand to "acepromazine"
        let mention = DrugMention {
            raw_text: "Give ace 0.5cc IM".into(),
            drug_name: "ace".into(),
            dose: Some(0.5),
            unit: Some("cc".into()),  // Should normalize to mL
            route: Some("IM".into()),
            species: None,
            start_offset: 5,
            end_offset: 17,
        };

        let result = resolver.resolve(&mention, Some("canine"), Some(20.0)).unwrap();

        // Should find acepromazine
        assert_eq!(result.top_candidate.sku, "ACE-10");

        // Unit should be normalized
        assert_eq!(result.mention.normalized_unit, Some("mL".into()));
    }
}

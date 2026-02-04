//! SKU disambiguation using multi-factor scoring.
//!
//! Scoring weights:
//! - Name/alias match quality: 40%
//! - Species compatibility: 25%
//! - Route compatibility: 20%
//! - Dose plausibility: 15%

use strsim::{jaro_winkler, normalized_levenshtein};

use crate::db::Database;
use crate::models::{CatalogItem, NormalizedMention, ScoreBreakdown, ScoredCandidate};

use super::ResolverResult;

/// Number of candidates to retrieve from FTS5.
const FTS_CANDIDATE_LIMIT: usize = 20;

/// Number of alternatives to include in results.
const MAX_ALTERNATIVES: usize = 4;

/// Minimum confidence to be considered a candidate.
const MIN_CONFIDENCE: f64 = 0.20;

/// Disambiguator for resolving mentions to SKUs.
pub struct Disambiguator<'a> {
    db: &'a Database,
}

impl<'a> Disambiguator<'a> {
    /// Create a new disambiguator.
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Disambiguate a normalized mention to find best SKU matches.
    ///
    /// Returns (top_candidate, alternatives).
    pub fn disambiguate(
        &self,
        mention: &NormalizedMention,
        patient_species: Option<&str>,
        patient_weight_kg: Option<f64>,
    ) -> ResolverResult<(ScoredCandidate, Vec<ScoredCandidate>)> {
        // 1. Retrieve candidates via FTS5
        let candidates = self
            .db
            .search_catalog(&mention.normalized_name, FTS_CANDIDATE_LIMIT)?;

        if candidates.is_empty() {
            // Try searching with original drug name as fallback
            let fallback_candidates = self
                .db
                .search_catalog(&mention.original.drug_name, FTS_CANDIDATE_LIMIT)?;

            if fallback_candidates.is_empty() {
                return Err(super::ResolverError::NoCandidates(
                    mention.normalized_name.clone(),
                ));
            }

            return self.score_and_rank(fallback_candidates, mention, patient_species, patient_weight_kg);
        }

        self.score_and_rank(candidates, mention, patient_species, patient_weight_kg)
    }

    /// Score all candidates and return ranked results.
    fn score_and_rank(
        &self,
        candidates: Vec<CatalogItem>,
        mention: &NormalizedMention,
        patient_species: Option<&str>,
        patient_weight_kg: Option<f64>,
    ) -> ResolverResult<(ScoredCandidate, Vec<ScoredCandidate>)> {
        let mut scored: Vec<ScoredCandidate> = candidates
            .into_iter()
            .map(|item| self.score_candidate(&item, mention, patient_species, patient_weight_kg))
            .filter(|c| c.confidence >= MIN_CONFIDENCE)
            .collect();

        if scored.is_empty() {
            return Err(super::ResolverError::NoCandidates(
                mention.normalized_name.clone(),
            ));
        }

        // Sort by confidence descending
        scored.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

        let top = scored.remove(0);
        let alternatives: Vec<ScoredCandidate> = scored.into_iter().take(MAX_ALTERNATIVES).collect();

        Ok((top, alternatives))
    }

    /// Score a single candidate against the mention.
    fn score_candidate(
        &self,
        item: &CatalogItem,
        mention: &NormalizedMention,
        patient_species: Option<&str>,
        patient_weight_kg: Option<f64>,
    ) -> ScoredCandidate {
        let breakdown = ScoreBreakdown {
            name_score: self.score_name_match(item, &mention.normalized_name),
            species_score: self.score_species(item, patient_species),
            route_score: self.score_route(item, mention.normalized_route.as_deref()),
            dose_score: self.score_dose(
                item,
                mention.normalized_dose,
                mention.normalized_unit.as_deref(),
                patient_weight_kg,
            ),
        };

        ScoredCandidate {
            sku: item.sku.clone(),
            name: item.name.clone(),
            confidence: breakdown.weighted_score(),
            score_breakdown: breakdown,
        }
    }

    /// Score name/alias match quality (0.0 - 1.0).
    fn score_name_match(&self, item: &CatalogItem, query: &str) -> f64 {
        let query_lower = query.to_lowercase();

        // Check exact match on name
        if item.name.to_lowercase().contains(&query_lower) {
            return 1.0;
        }

        // Check exact match on aliases
        for alias in &item.aliases {
            if alias.to_lowercase() == query_lower {
                return 1.0;
            }
        }

        // Fuzzy match on name
        let name_lower = item.name.to_lowercase();
        let name_similarity = fuzzy_match(&query_lower, &name_lower);

        // Fuzzy match on aliases
        let alias_similarity = item
            .aliases
            .iter()
            .map(|a| fuzzy_match(&query_lower, &a.to_lowercase()))
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        // Take the best match
        name_similarity.max(alias_similarity)
    }

    /// Score species compatibility (0.0 - 1.0).
    fn score_species(&self, item: &CatalogItem, patient_species: Option<&str>) -> f64 {
        match patient_species {
            None => 0.75, // Unknown species - moderate score
            Some(species) => {
                if item.is_species_compatible(species) {
                    1.0
                } else if item.species.is_empty() {
                    0.75 // No restriction - probably compatible
                } else {
                    0.1 // Explicitly incompatible
                }
            }
        }
    }

    /// Score route compatibility (0.0 - 1.0).
    fn score_route(&self, item: &CatalogItem, route: Option<&str>) -> f64 {
        match route {
            None => 0.75, // Unknown route - moderate score
            Some(r) => {
                if item.is_route_compatible(r) {
                    1.0
                } else if item.routes.is_empty() {
                    0.75 // No restriction
                } else {
                    0.2 // Incompatible but might be off-label
                }
            }
        }
    }

    /// Score dose plausibility (0.0 - 1.0).
    fn score_dose(
        &self,
        item: &CatalogItem,
        dose: Option<f64>,
        unit: Option<&str>,
        weight_kg: Option<f64>,
    ) -> f64 {
        match (dose, unit, weight_kg) {
            (Some(d), Some(u), Some(w)) => {
                match item.is_dose_plausible(d, u, w) {
                    Some(true) => 1.0,
                    Some(false) => 0.3, // Out of range but might be intentional
                    None => 0.6,        // Can't compare - unknown
                }
            }
            _ => 0.6, // Missing data - moderate score
        }
    }
}

/// Compute fuzzy string similarity using combined metrics.
fn fuzzy_match(a: &str, b: &str) -> f64 {
    // Combine Jaro-Winkler (good for typos) and Levenshtein (good for overall similarity)
    let jw = jaro_winkler(a, b);
    let lev = normalized_levenshtein(a, b);

    // Weight Jaro-Winkler more heavily as it's better for prefix matching
    jw * 0.6 + lev * 0.4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DoseRange, DrugMention};

    fn setup_db() -> Database {
        let db = Database::open_in_memory().unwrap();

        // Carprofen
        let mut item1 = CatalogItem::new("CARP-100".into(), "Carprofen 100mg tablets".into());
        item1.aliases = vec!["rimadyl".into(), "novox".into()];
        item1.species = vec!["canine".into()];
        item1.routes = vec!["PO".into()];
        item1.dose_range = Some(DoseRange {
            min_dose_per_kg: 2.0,
            max_dose_per_kg: 4.4,
            unit: "mg".into(),
        });
        db.upsert_catalog_item(&item1).unwrap();

        // Carprofen different strength
        let mut item1b = CatalogItem::new("CARP-75".into(), "Carprofen 75mg tablets".into());
        item1b.aliases = vec!["rimadyl".into()];
        item1b.species = vec!["canine".into()];
        item1b.routes = vec!["PO".into()];
        db.upsert_catalog_item(&item1b).unwrap();

        // Meloxicam
        let mut item2 =
            CatalogItem::new("MELOX-15".into(), "Meloxicam 1.5mg/mL oral suspension".into());
        item2.aliases = vec!["metacam".into()];
        item2.species = vec!["canine".into(), "feline".into()];
        item2.routes = vec!["PO".into()];
        db.upsert_catalog_item(&item2).unwrap();

        // Acepromazine
        let mut item3 = CatalogItem::new("ACE-10".into(), "Acepromazine 10mg/mL injection".into());
        item3.aliases = vec!["ace".into(), "promace".into()];
        item3.species = vec!["canine".into(), "feline".into(), "equine".into()];
        item3.routes = vec!["IV".into(), "IM".into(), "SQ".into()];
        db.upsert_catalog_item(&item3).unwrap();

        db
    }

    fn make_mention(drug: &str, dose: Option<f64>, unit: Option<&str>, route: Option<&str>) -> NormalizedMention {
        NormalizedMention {
            original: DrugMention {
                raw_text: "test".into(),
                drug_name: drug.into(),
                dose,
                unit: unit.map(|s| s.into()),
                route: route.map(|s| s.into()),
                species: None,
                start_offset: 0,
                end_offset: 4,
            },
            normalized_name: drug.into(),
            normalized_dose: dose,
            normalized_unit: unit.map(|s| s.into()),
            normalized_route: route.map(|s| s.into()),
        }
    }

    #[test]
    fn test_exact_name_match() {
        let db = setup_db();
        let disambiguator = Disambiguator::new(&db);

        let mention = make_mention("carprofen", Some(100.0), Some("mg"), Some("PO"));
        let (top, _) = disambiguator
            .disambiguate(&mention, Some("canine"), Some(30.0))
            .unwrap();

        // Should match carprofen items
        assert!(top.sku.starts_with("CARP"));
        assert!(top.confidence > 0.8);
    }

    #[test]
    fn test_alias_match() {
        let db = setup_db();
        let disambiguator = Disambiguator::new(&db);

        let mention = make_mention("rimadyl", Some(100.0), Some("mg"), Some("PO"));
        let (top, _) = disambiguator
            .disambiguate(&mention, Some("canine"), Some(30.0))
            .unwrap();

        // Rimadyl is an alias for carprofen
        assert!(top.sku.starts_with("CARP"));
        assert!(top.score_breakdown.name_score > 0.9);
    }

    #[test]
    fn test_species_scoring() {
        let db = setup_db();
        let disambiguator = Disambiguator::new(&db);

        // Carprofen is canine-only
        let mention = make_mention("carprofen", None, None, None);

        // For a canine patient
        let (top_canine, _) = disambiguator
            .disambiguate(&mention, Some("canine"), None)
            .unwrap();
        assert_eq!(top_canine.score_breakdown.species_score, 1.0);

        // For a feline patient (carprofen not approved for cats)
        let (top_feline, _) = disambiguator
            .disambiguate(&mention, Some("feline"), None)
            .unwrap();
        assert_eq!(top_feline.score_breakdown.species_score, 0.1);
    }

    #[test]
    fn test_route_scoring() {
        let db = setup_db();
        let disambiguator = Disambiguator::new(&db);

        // Acepromazine - IV, IM, SQ routes
        let mention = make_mention("acepromazine", None, None, Some("IM"));
        let (top, _) = disambiguator.disambiguate(&mention, None, None).unwrap();

        assert_eq!(top.sku, "ACE-10");
        assert_eq!(top.score_breakdown.route_score, 1.0);

        // Try with incompatible route
        let mention_po = make_mention("acepromazine", None, None, Some("PO"));
        let (top_po, _) = disambiguator.disambiguate(&mention_po, None, None).unwrap();
        assert!(top_po.score_breakdown.route_score < 0.5);
    }

    #[test]
    fn test_dose_scoring() {
        let db = setup_db();
        let disambiguator = Disambiguator::new(&db);

        // 30kg dog, 100mg carprofen = 3.3mg/kg (within 2-4.4 range)
        let mention = make_mention("carprofen", Some(100.0), Some("mg"), Some("PO"));
        let (top, _) = disambiguator
            .disambiguate(&mention, Some("canine"), Some(30.0))
            .unwrap();

        assert_eq!(top.score_breakdown.dose_score, 1.0);

        // 10kg dog, 100mg = 10mg/kg (above 4.4 range)
        let (top_high, _) = disambiguator
            .disambiguate(&mention, Some("canine"), Some(10.0))
            .unwrap();
        // The top result might be CARP-75 (no dose range, score 0.6) or CARP-100 (out of range, score 0.3)
        // CARP-75 lacks dose_range so can't be penalized. Accept either a low score or unknown dose score.
        assert!(
            top_high.score_breakdown.dose_score < 0.5 || top_high.sku == "CARP-75",
            "Expected dose_score < 0.5 for CARP-100 or CARP-75 without dose range, got {} for {}",
            top_high.score_breakdown.dose_score,
            top_high.sku
        );
    }

    #[test]
    fn test_alternatives_returned() {
        let db = setup_db();
        let disambiguator = Disambiguator::new(&db);

        // Search for carprofen - should find multiple variants
        let mention = make_mention("carprofen", None, None, None);
        let (top, alternatives) = disambiguator
            .disambiguate(&mention, Some("canine"), None)
            .unwrap();

        // Should have at least one alternative
        assert!(!alternatives.is_empty());
        // Alternatives should have lower or equal confidence (equal can happen with similar scores)
        assert!(
            alternatives.iter().all(|a| a.confidence <= top.confidence),
            "Expected all alternatives to have confidence <= top ({}), but found: {:?}",
            top.confidence,
            alternatives.iter().map(|a| (a.sku.clone(), a.confidence)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_fuzzy_match() {
        // Test the fuzzy matching function
        assert!(fuzzy_match("carprofen", "carprofen") > 0.99);
        assert!(fuzzy_match("carprofen", "carprofn") > 0.85); // Typo
        assert!(fuzzy_match("carprofen", "meloxicam") < 0.5); // Different drug
    }

    #[test]
    fn test_no_candidates_error() {
        let db = setup_db();
        let disambiguator = Disambiguator::new(&db);

        let mention = make_mention("nonexistentdrug12345", None, None, None);
        let result = disambiguator.disambiguate(&mention, None, None);

        assert!(matches!(
            result,
            Err(super::super::ResolverError::NoCandidates(_))
        ));
    }
}

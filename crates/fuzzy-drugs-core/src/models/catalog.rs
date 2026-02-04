//! Inventory catalog models.

use serde::{Deserialize, Serialize};

/// A single item in the veterinary inventory catalog.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogItem {
    /// Stock Keeping Unit - unique identifier
    pub sku: String,
    /// Primary drug/product name
    pub name: String,
    /// Alternative names/spellings for fuzzy matching
    pub aliases: Vec<String>,
    /// Drug concentration (e.g., "10mg/mL")
    pub concentration: Option<String>,
    /// Package size (e.g., "100mL", "50 tablets")
    pub package_size: Option<String>,
    /// Compatible species (e.g., ["canine", "feline"])
    pub species: Vec<String>,
    /// Compatible routes (e.g., ["PO", "IV", "IM", "SQ"])
    pub routes: Vec<String>,
    /// Typical dose range for validation
    pub dose_range: Option<DoseRange>,
    /// Whether this item is currently active in inventory
    pub active: bool,
    /// PIMS server ID for sync
    pub server_id: Option<String>,
    /// Last sync timestamp
    pub last_synced: Option<String>,
}

/// Dose range for plausibility checking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DoseRange {
    /// Minimum dose per kg body weight
    pub min_dose_per_kg: f64,
    /// Maximum dose per kg body weight
    pub max_dose_per_kg: f64,
    /// Unit for dose (e.g., "mg", "mL")
    pub unit: String,
}

impl CatalogItem {
    /// Create a new catalog item with required fields.
    pub fn new(sku: String, name: String) -> Self {
        Self {
            sku,
            name,
            aliases: Vec::new(),
            concentration: None,
            package_size: None,
            species: Vec::new(),
            routes: Vec::new(),
            dose_range: None,
            active: true,
            server_id: None,
            last_synced: None,
        }
    }

    /// Check if this item is compatible with a given species.
    pub fn is_species_compatible(&self, species: &str) -> bool {
        if self.species.is_empty() {
            return true; // No restriction means all species
        }
        let species_lower = species.to_lowercase();
        self.species
            .iter()
            .any(|s| s.to_lowercase() == species_lower)
    }

    /// Check if this item is compatible with a given route.
    pub fn is_route_compatible(&self, route: &str) -> bool {
        if self.routes.is_empty() {
            return true; // No restriction means all routes
        }
        let route_upper = route.to_uppercase();
        self.routes.iter().any(|r| r.to_uppercase() == route_upper)
    }

    /// Check if a dose is within plausible range for given weight.
    pub fn is_dose_plausible(&self, dose: f64, unit: &str, weight_kg: f64) -> Option<bool> {
        let range = self.dose_range.as_ref()?;
        if range.unit.to_lowercase() != unit.to_lowercase() {
            return None; // Can't compare different units
        }
        let dose_per_kg = dose / weight_kg;
        Some(dose_per_kg >= range.min_dose_per_kg && dose_per_kg <= range.max_dose_per_kg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_species_compatibility() {
        let mut item = CatalogItem::new("SKU001".into(), "Test Drug".into());
        item.species = vec!["canine".into(), "feline".into()];

        assert!(item.is_species_compatible("canine"));
        assert!(item.is_species_compatible("Canine"));
        assert!(item.is_species_compatible("feline"));
        assert!(!item.is_species_compatible("equine"));
    }

    #[test]
    fn test_empty_species_means_all() {
        let item = CatalogItem::new("SKU001".into(), "Test Drug".into());
        assert!(item.is_species_compatible("anything"));
    }

    #[test]
    fn test_route_compatibility() {
        let mut item = CatalogItem::new("SKU001".into(), "Test Drug".into());
        item.routes = vec!["PO".into(), "IV".into()];

        assert!(item.is_route_compatible("PO"));
        assert!(item.is_route_compatible("po"));
        assert!(item.is_route_compatible("IV"));
        assert!(!item.is_route_compatible("IM"));
    }

    #[test]
    fn test_dose_plausibility() {
        let mut item = CatalogItem::new("SKU001".into(), "Test Drug".into());
        item.dose_range = Some(DoseRange {
            min_dose_per_kg: 1.0,
            max_dose_per_kg: 5.0,
            unit: "mg".into(),
        });

        // 10kg dog, 30mg dose = 3mg/kg (within range)
        assert_eq!(item.is_dose_plausible(30.0, "mg", 10.0), Some(true));

        // 10kg dog, 100mg dose = 10mg/kg (above range)
        assert_eq!(item.is_dose_plausible(100.0, "mg", 10.0), Some(false));

        // Wrong unit
        assert_eq!(item.is_dose_plausible(30.0, "mL", 10.0), None);
    }
}

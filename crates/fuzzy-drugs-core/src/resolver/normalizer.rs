//! Drug mention normalizer.
//!
//! Handles:
//! - Unit conversion (cc→mL, mcg→mg, etc.)
//! - Alias expansion (ace→acepromazine, metacam→meloxicam)
//! - Route canonicalization (orally→PO, subcutaneously→SQ)

use std::collections::HashMap;

use crate::models::{DrugMention, NormalizedMention};

/// Normalizer for drug mentions.
pub struct Normalizer {
    /// Alias map: spoken name → canonical name
    aliases: HashMap<String, String>,
    /// Unit conversions: non-standard → standard
    unit_conversions: HashMap<String, (String, f64)>, // (canonical_unit, multiplier)
    /// Route canonicalization: spoken → standard abbreviation
    route_map: HashMap<String, String>,
}

impl Default for Normalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Normalizer {
    /// Create a new normalizer with default mappings.
    pub fn new() -> Self {
        Self {
            aliases: Self::default_aliases(),
            unit_conversions: Self::default_unit_conversions(),
            route_map: Self::default_routes(),
        }
    }

    /// Normalize a drug mention.
    pub fn normalize(&self, mention: &DrugMention) -> NormalizedMention {
        // Normalize drug name via alias expansion
        let normalized_name = self.expand_alias(&mention.drug_name);

        // Normalize unit and convert dose
        let (normalized_unit, normalized_dose) = if let (Some(unit), Some(dose)) =
            (&mention.unit, mention.dose)
        {
            let (canonical_unit, multiplier) = self.convert_unit(unit);
            (Some(canonical_unit), Some(dose * multiplier))
        } else {
            (mention.unit.clone(), mention.dose)
        };

        // Normalize route
        let normalized_route = mention.route.as_ref().map(|r| self.canonicalize_route(r));

        NormalizedMention {
            original: mention.clone(),
            normalized_name,
            normalized_dose,
            normalized_unit,
            normalized_route,
        }
    }

    /// Expand a drug alias to its canonical name.
    pub fn expand_alias(&self, name: &str) -> String {
        let lower = name.to_lowercase();
        self.aliases
            .get(&lower)
            .cloned()
            .unwrap_or_else(|| lower)
    }

    /// Convert a unit to canonical form with multiplier.
    pub fn convert_unit(&self, unit: &str) -> (String, f64) {
        let lower = unit.to_lowercase();
        self.unit_conversions
            .get(&lower)
            .cloned()
            .unwrap_or_else(|| (lower, 1.0))
    }

    /// Canonicalize a route of administration.
    pub fn canonicalize_route(&self, route: &str) -> String {
        let lower = route.to_lowercase();
        self.route_map
            .get(&lower)
            .cloned()
            .unwrap_or_else(|| route.to_uppercase())
    }

    /// Add a custom alias mapping.
    pub fn add_alias(&mut self, alias: &str, canonical: &str) {
        self.aliases
            .insert(alias.to_lowercase(), canonical.to_lowercase());
    }

    /// Add a custom unit conversion.
    pub fn add_unit_conversion(&mut self, from: &str, to: &str, multiplier: f64) {
        self.unit_conversions
            .insert(from.to_lowercase(), (to.to_lowercase(), multiplier));
    }

    /// Add a custom route mapping.
    pub fn add_route(&mut self, spoken: &str, canonical: &str) {
        self.route_map
            .insert(spoken.to_lowercase(), canonical.to_uppercase());
    }

    /// Default drug alias mappings.
    fn default_aliases() -> HashMap<String, String> {
        let mut map = HashMap::new();

        // NSAIDs
        map.insert("rimadyl".into(), "carprofen".into());
        map.insert("novox".into(), "carprofen".into());
        map.insert("quellin".into(), "carprofen".into());
        map.insert("metacam".into(), "meloxicam".into());
        map.insert("loxicom".into(), "meloxicam".into());
        map.insert("previcox".into(), "firocoxib".into());
        map.insert("deramaxx".into(), "deracoxib".into());
        map.insert("galliprant".into(), "grapiprant".into());
        map.insert("onsior".into(), "robenacoxib".into());

        // Sedatives/Tranquilizers
        map.insert("ace".into(), "acepromazine".into());
        map.insert("promace".into(), "acepromazine".into());
        map.insert("atravet".into(), "acepromazine".into());

        // Anesthetics
        map.insert("propoflo".into(), "propofol".into());
        map.insert("telazol".into(), "tiletamine-zolazepam".into());
        map.insert("domitor".into(), "medetomidine".into());
        map.insert("dexdomitor".into(), "dexmedetomidine".into());
        map.insert("antisedan".into(), "atipamezole".into());
        map.insert("torb".into(), "butorphanol".into());
        map.insert("torbugesic".into(), "butorphanol".into());

        // Antibiotics
        map.insert("clavamox".into(), "amoxicillin-clavulanate".into());
        map.insert("augmentin".into(), "amoxicillin-clavulanate".into());
        map.insert("baytril".into(), "enrofloxacin".into());
        map.insert("zeniquin".into(), "marbofloxacin".into());
        map.insert("convenia".into(), "cefovecin".into());
        map.insert("simplicef".into(), "cefpodoxime".into());
        map.insert("orbax".into(), "orbifloxacin".into());

        // Steroids
        map.insert("dex".into(), "dexamethasone".into());
        map.insert("depo".into(), "methylprednisolone".into());
        map.insert("depo-medrol".into(), "methylprednisolone".into());
        map.insert("pred".into(), "prednisone".into());
        map.insert("prednisolone".into(), "prednisolone".into());
        map.insert("vetalog".into(), "triamcinolone".into());

        // Antiparasitics
        map.insert("heartgard".into(), "ivermectin".into());
        map.insert("ivomec".into(), "ivermectin".into());
        map.insert("interceptor".into(), "milbemycin".into());
        map.insert("sentinel".into(), "milbemycin-lufenuron".into());
        map.insert("revolution".into(), "selamectin".into());
        map.insert("strongid".into(), "pyrantel".into());
        map.insert("panacur".into(), "fenbendazole".into());
        map.insert("drontal".into(), "praziquantel-pyrantel".into());

        // Cardiac
        map.insert("vetmedin".into(), "pimobendan".into());
        map.insert("enacard".into(), "enalapril".into());
        map.insert("vasotec".into(), "enalapril".into());
        map.insert("salix".into(), "furosemide".into());
        map.insert("lasix".into(), "furosemide".into());
        map.insert("digoxin".into(), "digoxin".into());

        // GI
        map.insert("cerenia".into(), "maropitant".into());
        map.insert("reglan".into(), "metoclopramide".into());
        map.insert("pepcid".into(), "famotidine".into());
        map.insert("zantac".into(), "ranitidine".into());
        map.insert("prilosec".into(), "omeprazole".into());
        map.insert("gastrogard".into(), "omeprazole".into());
        map.insert("sucralfate".into(), "sucralfate".into());
        map.insert("carafate".into(), "sucralfate".into());

        // Anticonvulsants
        map.insert("keppra".into(), "levetiracetam".into());
        map.insert("zonegran".into(), "zonisamide".into());
        map.insert("phenobarb".into(), "phenobarbital".into());
        map.insert("potassium bromide".into(), "potassium-bromide".into());
        map.insert("kbr".into(), "potassium-bromide".into());

        // Thyroid
        map.insert("soloxine".into(), "levothyroxine".into());
        map.insert("thyro-tabs".into(), "levothyroxine".into());
        map.insert("methimazole".into(), "methimazole".into());
        map.insert("tapazole".into(), "methimazole".into());
        map.insert("felimazole".into(), "methimazole".into());

        // Behavioral
        map.insert("clomicalm".into(), "clomipramine".into());
        map.insert("reconcile".into(), "fluoxetine".into());
        map.insert("prozac".into(), "fluoxetine".into());
        map.insert("sileo".into(), "dexmedetomidine".into());
        map.insert("trazadone".into(), "trazodone".into());

        map
    }

    /// Default unit conversions.
    fn default_unit_conversions() -> HashMap<String, (String, f64)> {
        let mut map = HashMap::new();

        // Volume
        map.insert("cc".into(), ("mL".into(), 1.0));
        map.insert("ml".into(), ("mL".into(), 1.0));
        map.insert("l".into(), ("mL".into(), 1000.0));
        map.insert("liter".into(), ("mL".into(), 1000.0));
        map.insert("liters".into(), ("mL".into(), 1000.0));

        // Mass
        map.insert("mcg".into(), ("mg".into(), 0.001));
        map.insert("microgram".into(), ("mg".into(), 0.001));
        map.insert("micrograms".into(), ("mg".into(), 0.001));
        map.insert("µg".into(), ("mg".into(), 0.001));
        map.insert("g".into(), ("mg".into(), 1000.0));
        map.insert("gram".into(), ("mg".into(), 1000.0));
        map.insert("grams".into(), ("mg".into(), 1000.0));
        map.insert("kg".into(), ("mg".into(), 1_000_000.0));

        // Units (keep as-is but standardize)
        map.insert("unit".into(), ("units".into(), 1.0));
        map.insert("iu".into(), ("IU".into(), 1.0));

        // Tablets/capsules
        map.insert("tab".into(), ("tablets".into(), 1.0));
        map.insert("tabs".into(), ("tablets".into(), 1.0));
        map.insert("tablet".into(), ("tablets".into(), 1.0));
        map.insert("cap".into(), ("capsules".into(), 1.0));
        map.insert("caps".into(), ("capsules".into(), 1.0));
        map.insert("capsule".into(), ("capsules".into(), 1.0));

        map
    }

    /// Default route mappings.
    fn default_routes() -> HashMap<String, String> {
        let mut map = HashMap::new();

        // Oral
        map.insert("oral".into(), "PO".into());
        map.insert("orally".into(), "PO".into());
        map.insert("by mouth".into(), "PO".into());
        map.insert("per os".into(), "PO".into());
        map.insert("po".into(), "PO".into());

        // Intravenous
        map.insert("intravenous".into(), "IV".into());
        map.insert("intravenously".into(), "IV".into());
        map.insert("iv".into(), "IV".into());
        map.insert("i.v.".into(), "IV".into());

        // Intramuscular
        map.insert("intramuscular".into(), "IM".into());
        map.insert("intramuscularly".into(), "IM".into());
        map.insert("im".into(), "IM".into());
        map.insert("i.m.".into(), "IM".into());

        // Subcutaneous
        map.insert("subcutaneous".into(), "SQ".into());
        map.insert("subcutaneously".into(), "SQ".into());
        map.insert("subq".into(), "SQ".into());
        map.insert("sub-q".into(), "SQ".into());
        map.insert("sq".into(), "SQ".into());
        map.insert("sc".into(), "SQ".into());

        // Topical
        map.insert("topical".into(), "TOP".into());
        map.insert("topically".into(), "TOP".into());
        map.insert("top".into(), "TOP".into());

        // Ophthalmic
        map.insert("ophthalmic".into(), "OPH".into());
        map.insert("ophthalmically".into(), "OPH".into());
        map.insert("eye".into(), "OPH".into());
        map.insert("in the eye".into(), "OPH".into());
        map.insert("in the eyes".into(), "OPH".into());
        map.insert("ou".into(), "OPH".into());
        map.insert("od".into(), "OPH".into());
        map.insert("os".into(), "OPH".into());

        // Otic
        map.insert("otic".into(), "OT".into());
        map.insert("ear".into(), "OT".into());
        map.insert("in the ear".into(), "OT".into());
        map.insert("in the ears".into(), "OT".into());

        // Rectal
        map.insert("rectal".into(), "PR".into());
        map.insert("rectally".into(), "PR".into());
        map.insert("per rectum".into(), "PR".into());
        map.insert("pr".into(), "PR".into());

        // Intranasal
        map.insert("intranasal".into(), "IN".into());
        map.insert("intranasally".into(), "IN".into());
        map.insert("in the nose".into(), "IN".into());

        // Transdermal
        map.insert("transdermal".into(), "TD".into());
        map.insert("transdermally".into(), "TD".into());

        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_alias() {
        let normalizer = Normalizer::new();

        assert_eq!(normalizer.expand_alias("rimadyl"), "carprofen");
        assert_eq!(normalizer.expand_alias("Rimadyl"), "carprofen");
        assert_eq!(normalizer.expand_alias("RIMADYL"), "carprofen");
        assert_eq!(normalizer.expand_alias("ace"), "acepromazine");
        assert_eq!(normalizer.expand_alias("metacam"), "meloxicam");

        // Unknown names pass through lowercase
        assert_eq!(normalizer.expand_alias("SomeNewDrug"), "somenewdrug");
    }

    #[test]
    fn test_unit_conversion() {
        let normalizer = Normalizer::new();

        let (unit, mult) = normalizer.convert_unit("cc");
        assert_eq!(unit, "mL");
        assert_eq!(mult, 1.0);

        let (unit, mult) = normalizer.convert_unit("mcg");
        assert_eq!(unit, "mg");
        assert_eq!(mult, 0.001);

        let (unit, mult) = normalizer.convert_unit("g");
        assert_eq!(unit, "mg");
        assert_eq!(mult, 1000.0);

        // Unknown units pass through
        let (unit, mult) = normalizer.convert_unit("widgets");
        assert_eq!(unit, "widgets");
        assert_eq!(mult, 1.0);
    }

    #[test]
    fn test_route_canonicalization() {
        let normalizer = Normalizer::new();

        assert_eq!(normalizer.canonicalize_route("orally"), "PO");
        assert_eq!(normalizer.canonicalize_route("by mouth"), "PO");
        assert_eq!(normalizer.canonicalize_route("intravenously"), "IV");
        assert_eq!(normalizer.canonicalize_route("subcutaneously"), "SQ");
        assert_eq!(normalizer.canonicalize_route("sub-q"), "SQ");

        // Unknown routes get uppercased
        assert_eq!(normalizer.canonicalize_route("weird_route"), "WEIRD_ROUTE");
    }

    #[test]
    fn test_normalize_mention() {
        let normalizer = Normalizer::new();

        let mention = DrugMention {
            raw_text: "Give rimadyl 100mg orally".into(),
            drug_name: "rimadyl".into(),
            dose: Some(100.0),
            unit: Some("mg".into()),
            route: Some("orally".into()),
            species: None,
            start_offset: 0,
            end_offset: 25,
        };

        let normalized = normalizer.normalize(&mention);

        assert_eq!(normalized.normalized_name, "carprofen");
        assert_eq!(normalized.normalized_dose, Some(100.0));
        assert_eq!(normalized.normalized_unit, Some("mg".into()));
        assert_eq!(normalized.normalized_route, Some("PO".into()));
    }

    #[test]
    fn test_normalize_with_conversion() {
        let normalizer = Normalizer::new();

        let mention = DrugMention {
            raw_text: "Give ace 0.5cc IM".into(),
            drug_name: "ace".into(),
            dose: Some(0.5),
            unit: Some("cc".into()),
            route: Some("IM".into()),
            species: None,
            start_offset: 0,
            end_offset: 17,
        };

        let normalized = normalizer.normalize(&mention);

        assert_eq!(normalized.normalized_name, "acepromazine");
        assert_eq!(normalized.normalized_dose, Some(0.5)); // cc = mL, multiplier 1.0
        assert_eq!(normalized.normalized_unit, Some("mL".into()));
        assert_eq!(normalized.normalized_route, Some("IM".into()));
    }

    #[test]
    fn test_microgram_conversion() {
        let normalizer = Normalizer::new();

        let mention = DrugMention {
            raw_text: "Give 500mcg".into(),
            drug_name: "test".into(),
            dose: Some(500.0),
            unit: Some("mcg".into()),
            route: None,
            species: None,
            start_offset: 0,
            end_offset: 11,
        };

        let normalized = normalizer.normalize(&mention);

        assert_eq!(normalized.normalized_dose, Some(0.5)); // 500mcg = 0.5mg
        assert_eq!(normalized.normalized_unit, Some("mg".into()));
    }

    #[test]
    fn test_custom_alias() {
        let mut normalizer = Normalizer::new();
        normalizer.add_alias("customdrug", "realdrugname");

        assert_eq!(normalizer.expand_alias("customdrug"), "realdrugname");
    }
}

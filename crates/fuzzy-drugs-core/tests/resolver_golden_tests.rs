//! Golden tests for the semantic resolver.
//!
//! These tests verify normalization against known test cases.

use fuzzy_drugs_core::models::DrugMention;
use fuzzy_drugs_core::resolver::Normalizer;

/// Test case from golden file.
struct GoldenCase {
    id: &'static str,
    input_drug_name: &'static str,
    input_dose: Option<f64>,
    input_unit: Option<&'static str>,
    input_route: Option<&'static str>,
    expected_name: &'static str,
    expected_dose: Option<f64>,
    expected_unit: Option<&'static str>,
    expected_route: Option<&'static str>,
}

fn get_golden_cases() -> Vec<GoldenCase> {
    vec![
        GoldenCase {
            id: "rimadyl-basic",
            input_drug_name: "rimadyl",
            input_dose: Some(100.0),
            input_unit: Some("mg"),
            input_route: Some("PO"),
            expected_name: "carprofen",
            expected_dose: Some(100.0),
            expected_unit: Some("mg"),
            expected_route: Some("PO"),
        },
        GoldenCase {
            id: "ace-injection",
            input_drug_name: "ace",
            input_dose: Some(0.5),
            input_unit: Some("cc"),
            input_route: Some("IM"),
            expected_name: "acepromazine",
            expected_dose: Some(0.5),
            expected_unit: Some("mL"),
            expected_route: Some("IM"),
        },
        GoldenCase {
            id: "metacam-oral",
            input_drug_name: "metacam",
            input_dose: Some(1.5),
            input_unit: Some("mL"),
            input_route: Some("orally"),
            expected_name: "meloxicam",
            expected_dose: Some(1.5),
            expected_unit: Some("mL"),
            expected_route: Some("PO"),
        },
        GoldenCase {
            id: "microgram-conversion",
            input_drug_name: "test",
            input_dose: Some(500.0),
            input_unit: Some("mcg"),
            input_route: Some("PO"),
            expected_name: "test",
            expected_dose: Some(0.5),
            expected_unit: Some("mg"),
            expected_route: Some("PO"),
        },
        GoldenCase {
            id: "gram-conversion",
            input_drug_name: "test",
            input_dose: Some(2.0),
            input_unit: Some("g"),
            input_route: Some("PO"),
            expected_name: "test",
            expected_dose: Some(2000.0),
            expected_unit: Some("mg"),
            expected_route: Some("PO"),
        },
        GoldenCase {
            id: "keppra-anticonvulsant",
            input_drug_name: "keppra",
            input_dose: Some(500.0),
            input_unit: Some("mg"),
            input_route: Some("orally"),
            expected_name: "levetiracetam",
            expected_dose: Some(500.0),
            expected_unit: Some("mg"),
            expected_route: Some("PO"),
        },
        GoldenCase {
            id: "vetmedin-cardiac",
            input_drug_name: "vetmedin",
            input_dose: Some(2.5),
            input_unit: Some("mg"),
            input_route: Some("PO"),
            expected_name: "pimobendan",
            expected_dose: Some(2.5),
            expected_unit: Some("mg"),
            expected_route: Some("PO"),
        },
        GoldenCase {
            id: "lasix-diuretic",
            input_drug_name: "lasix",
            input_dose: Some(40.0),
            input_unit: Some("mg"),
            input_route: Some("IV"),
            expected_name: "furosemide",
            expected_dose: Some(40.0),
            expected_unit: Some("mg"),
            expected_route: Some("IV"),
        },
        GoldenCase {
            id: "dexdomitor-sedation",
            input_drug_name: "dexdomitor",
            input_dose: Some(0.1),
            input_unit: Some("ml"),
            input_route: Some("IM"),
            expected_name: "dexmedetomidine",
            expected_dose: Some(0.1),
            expected_unit: Some("mL"),
            expected_route: Some("IM"),
        },
        GoldenCase {
            id: "clavamox-antibiotic",
            input_drug_name: "clavamox",
            input_dose: Some(250.0),
            input_unit: Some("mg"),
            input_route: Some("by mouth"),
            expected_name: "amoxicillin-clavulanate",
            expected_dose: Some(250.0),
            expected_unit: Some("mg"),
            expected_route: Some("PO"),
        },
        GoldenCase {
            id: "subcutaneous-routes",
            input_drug_name: "test",
            input_dose: None,
            input_unit: None,
            input_route: Some("subcutaneously"),
            expected_name: "test",
            expected_dose: None,
            expected_unit: None,
            expected_route: Some("SQ"),
        },
        GoldenCase {
            id: "subq-route",
            input_drug_name: "test",
            input_dose: None,
            input_unit: None,
            input_route: Some("sub-q"),
            expected_name: "test",
            expected_dose: None,
            expected_unit: None,
            expected_route: Some("SQ"),
        },
    ]
}

#[test]
fn test_golden_cases() {
    let normalizer = Normalizer::new();

    for case in get_golden_cases() {
        let mention = DrugMention {
            raw_text: format!("{} {} {}",
                case.input_dose.map(|d| d.to_string()).unwrap_or_default(),
                case.input_drug_name,
                case.input_route.unwrap_or("")
            ),
            drug_name: case.input_drug_name.to_string(),
            dose: case.input_dose,
            unit: case.input_unit.map(|s| s.to_string()),
            route: case.input_route.map(|s| s.to_string()),
            species: None,
            start_offset: 0,
            end_offset: 0,
        };

        let normalized = normalizer.normalize(&mention);

        assert_eq!(
            normalized.normalized_name, case.expected_name,
            "Case {}: name mismatch", case.id
        );

        if let Some(expected_dose) = case.expected_dose {
            let actual_dose = normalized.normalized_dose.unwrap_or(0.0);
            assert!(
                (actual_dose - expected_dose).abs() < 0.001,
                "Case {}: dose mismatch - expected {}, got {}",
                case.id, expected_dose, actual_dose
            );
        }

        assert_eq!(
            normalized.normalized_unit.as_deref(), case.expected_unit,
            "Case {}: unit mismatch", case.id
        );

        assert_eq!(
            normalized.normalized_route.as_deref(), case.expected_route,
            "Case {}: route mismatch", case.id
        );
    }
}

#[test]
fn test_all_common_aliases() {
    let normalizer = Normalizer::new();

    let alias_tests = vec![
        ("rimadyl", "carprofen"),
        ("novox", "carprofen"),
        ("metacam", "meloxicam"),
        ("ace", "acepromazine"),
        ("promace", "acepromazine"),
        ("cerenia", "maropitant"),
        ("convenia", "cefovecin"),
        ("baytril", "enrofloxacin"),
        ("dex", "dexamethasone"),
        ("torb", "butorphanol"),
        ("keppra", "levetiracetam"),
        ("vetmedin", "pimobendan"),
        ("lasix", "furosemide"),
        ("salix", "furosemide"),
        ("dexdomitor", "dexmedetomidine"),
        ("domitor", "medetomidine"),
        ("clavamox", "amoxicillin-clavulanate"),
        ("phenobarb", "phenobarbital"),
        ("pred", "prednisone"),
    ];

    for (alias, expected) in alias_tests {
        let result = normalizer.expand_alias(alias);
        assert_eq!(
            result, expected,
            "Alias {} should expand to {}, got {}",
            alias, expected, result
        );
    }
}

#[test]
fn test_all_route_canonicalizations() {
    let normalizer = Normalizer::new();

    let route_tests = vec![
        ("orally", "PO"),
        ("by mouth", "PO"),
        ("per os", "PO"),
        ("intravenously", "IV"),
        ("intramuscularly", "IM"),
        ("subcutaneously", "SQ"),
        ("subq", "SQ"),
        ("sub-q", "SQ"),
        ("topically", "TOP"),
        ("ophthalmic", "OPH"),
        ("otic", "OT"),
        ("rectally", "PR"),
    ];

    for (spoken, expected) in route_tests {
        let result = normalizer.canonicalize_route(spoken);
        assert_eq!(
            result, expected,
            "Route {} should canonicalize to {}, got {}",
            spoken, expected, result
        );
    }
}

#[test]
fn test_all_unit_conversions() {
    let normalizer = Normalizer::new();

    let unit_tests = vec![
        ("cc", "mL", 1.0),
        ("ml", "mL", 1.0),
        ("mcg", "mg", 0.001),
        ("microgram", "mg", 0.001),
        ("g", "mg", 1000.0),
        ("gram", "mg", 1000.0),
        ("kg", "mg", 1_000_000.0),
        ("tab", "tablets", 1.0),
        ("tabs", "tablets", 1.0),
        ("cap", "capsules", 1.0),
        ("iu", "IU", 1.0),
    ];

    for (from, expected_unit, expected_mult) in unit_tests {
        let (unit, mult) = normalizer.convert_unit(from);
        assert_eq!(
            unit, expected_unit,
            "Unit {} should convert to {}, got {}",
            from, expected_unit, unit
        );
        assert!(
            (mult - expected_mult).abs() < 0.0001,
            "Unit {} multiplier should be {}, got {}",
            from, expected_mult, mult
        );
    }
}

//! Drug mention extraction from LLM output.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Extraction errors.
#[derive(Error, Debug)]
pub enum ExtractionError {
    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("Invalid response format: {0}")]
    InvalidFormat(String),

    #[error("LLM inference error: {0}")]
    Inference(String),
}

pub type ExtractionResult<T> = Result<T, ExtractionError>;

/// Raw NER output from LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NerOutput {
    pub mentions: Vec<RawMention>,
}

/// A raw drug mention extracted by the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawMention {
    pub raw_text: String,
    pub drug_name: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub species: Option<String>,
    pub start_offset: usize,
    pub end_offset: usize,
}

/// Parse LLM output JSON into structured mentions.
pub fn parse_ner_output(json: &str) -> ExtractionResult<NerOutput> {
    // Try to find JSON in the response (in case LLM adds extra text)
    let json_start = json.find('{').ok_or_else(|| {
        ExtractionError::InvalidFormat("No JSON object found in response".into())
    })?;
    let json_end = json.rfind('}').ok_or_else(|| {
        ExtractionError::InvalidFormat("No closing brace found in response".into())
    })?;

    let json_slice = &json[json_start..=json_end];
    let output: NerOutput = serde_json::from_str(json_slice)?;

    Ok(output)
}

/// Convert raw mentions to the format expected by the resolver.
pub fn to_drug_mentions(ner_output: &NerOutput) -> Vec<DrugMention> {
    ner_output
        .mentions
        .iter()
        .map(|m| DrugMention {
            raw_text: m.raw_text.clone(),
            drug_name: m.drug_name.clone(),
            dose: m.dose,
            unit: m.unit.clone(),
            route: m.route.clone(),
            species: m.species.clone(),
            start_offset: m.start_offset,
            end_offset: m.end_offset,
        })
        .collect()
}

/// Drug mention in resolver-compatible format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrugMention {
    pub raw_text: String,
    pub drug_name: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub species: Option<String>,
    pub start_offset: usize,
    pub end_offset: usize,
}

/// Mock extractor for testing without actual LLM inference.
pub struct MockExtractor;

impl MockExtractor {
    /// Extract mentions using simple pattern matching (for testing).
    pub fn extract(transcript: &str) -> NerOutput {
        let mut mentions = Vec::new();
        let transcript_lower = transcript.to_lowercase();

        // Simple pattern matching for common drugs
        let patterns = [
            ("carprofen", None),
            ("rimadyl", Some("carprofen")),
            ("meloxicam", None),
            ("metacam", Some("meloxicam")),
            ("acepromazine", None),
            ("ace ", Some("acepromazine")),
            ("cerenia", None),
            ("convenia", None),
            ("baytril", None),
            ("prednisone", None),
            ("dexamethasone", None),
        ];

        for (pattern, canonical) in patterns {
            if let Some(pos) = transcript_lower.find(pattern) {
                // Try to find dose before drug name
                let before = &transcript_lower[..pos];
                let (dose, unit) = extract_dose(before);

                // Try to find route after drug name
                let after = &transcript_lower[pos..];
                let route = extract_route(after);

                let drug_name = canonical.unwrap_or(pattern).to_string();
                let end_pos = pos + pattern.len();

                mentions.push(RawMention {
                    raw_text: transcript[pos.saturating_sub(10)..std::cmp::min(end_pos + 10, transcript.len())].to_string(),
                    drug_name,
                    dose,
                    unit,
                    route,
                    species: None,
                    start_offset: pos,
                    end_offset: end_pos,
                });
            }
        }

        NerOutput { mentions }
    }
}

/// Simple dose extraction from text before drug name.
fn extract_dose(text: &str) -> (Option<f64>, Option<String>) {
    // Look for patterns like "100mg", "0.5 mL", "2 cc"
    let text = text.trim();
    let words: Vec<&str> = text.split_whitespace().collect();

    if words.is_empty() {
        return (None, None);
    }

    let last = words.last().unwrap();

    // Try to parse number with unit attached
    for unit in ["mg", "ml", "cc", "g", "mcg", "units", "tablets", "tabs"] {
        if let Some(num_str) = last.strip_suffix(unit) {
            if let Ok(num) = num_str.parse::<f64>() {
                return (Some(num), Some(unit.to_string()));
            }
        }
    }

    // Try separate number and unit
    if words.len() >= 2 {
        let potential_num = words[words.len() - 2];
        let potential_unit = words[words.len() - 1];

        if let Ok(num) = potential_num.parse::<f64>() {
            let unit = match potential_unit {
                "mg" | "ml" | "cc" | "g" | "mcg" | "units" | "tablets" | "tabs" => {
                    Some(potential_unit.to_string())
                }
                _ => None,
            };
            if unit.is_some() {
                return (Some(num), unit);
            }
        }
    }

    (None, None)
}

/// Simple route extraction from text after drug name.
fn extract_route(text: &str) -> Option<String> {
    let text_lower = text.to_lowercase();

    let routes = [
        ("orally", "PO"),
        ("by mouth", "PO"),
        (" po", "PO"),
        ("intravenously", "IV"),
        (" iv", "IV"),
        ("intramuscularly", "IM"),
        (" im", "IM"),
        ("subcutaneously", "SQ"),
        ("sub-q", "SQ"),
        ("subq", "SQ"),
        (" sq", "SQ"),
    ];

    for (pattern, canonical) in routes {
        if text_lower.contains(pattern) {
            return Some(canonical.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ner_output() {
        let json = r#"{"mentions":[{"raw_text":"100mg carprofen","drug_name":"carprofen","dose":100,"unit":"mg","route":"PO","species":null,"start_offset":0,"end_offset":15}]}"#;

        let output = parse_ner_output(json).unwrap();
        assert_eq!(output.mentions.len(), 1);
        assert_eq!(output.mentions[0].drug_name, "carprofen");
        assert_eq!(output.mentions[0].dose, Some(100.0));
    }

    #[test]
    fn test_parse_ner_output_with_prefix() {
        let json = r#"Here is the extracted information:
{"mentions":[{"raw_text":"test","drug_name":"test","dose":null,"unit":null,"route":null,"species":null,"start_offset":0,"end_offset":4}]}"#;

        let output = parse_ner_output(json).unwrap();
        assert_eq!(output.mentions.len(), 1);
    }

    #[test]
    fn test_mock_extractor() {
        let transcript = "Give 100mg carprofen orally twice daily";
        let output = MockExtractor::extract(transcript);

        assert_eq!(output.mentions.len(), 1);
        assert_eq!(output.mentions[0].drug_name, "carprofen");
        assert_eq!(output.mentions[0].dose, Some(100.0));
        assert_eq!(output.mentions[0].unit, Some("mg".to_string()));
        assert_eq!(output.mentions[0].route, Some("PO".to_string()));
    }

    #[test]
    fn test_mock_extractor_alias() {
        let transcript = "Give rimadyl to the dog";
        let output = MockExtractor::extract(transcript);

        assert_eq!(output.mentions.len(), 1);
        // Should map rimadyl to carprofen
        assert_eq!(output.mentions[0].drug_name, "carprofen");
    }

    #[test]
    fn test_mock_extractor_multiple() {
        let transcript = "Give carprofen and also metacam";
        let output = MockExtractor::extract(transcript);

        assert_eq!(output.mentions.len(), 2);
    }

    #[test]
    fn test_extract_dose() {
        assert_eq!(extract_dose("give 100mg"), (Some(100.0), Some("mg".to_string())));
        assert_eq!(extract_dose("give 0.5 ml"), (Some(0.5), Some("ml".to_string())));
        assert_eq!(extract_dose("give 2cc"), (Some(2.0), Some("cc".to_string())));
        assert_eq!(extract_dose("give the dog"), (None, None));
    }

    #[test]
    fn test_extract_route() {
        assert_eq!(extract_route(" orally twice daily"), Some("PO".to_string()));
        assert_eq!(extract_route(" IV push"), Some("IV".to_string()));
        assert_eq!(extract_route(" IM injection"), Some("IM".to_string()));
        assert_eq!(extract_route(" subcutaneously"), Some("SQ".to_string()));
        assert_eq!(extract_route(" topically"), None);
    }
}

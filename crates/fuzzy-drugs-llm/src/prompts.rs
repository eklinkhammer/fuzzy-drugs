//! NER prompts for veterinary drug extraction.
//!
//! These prompts are designed for Llama 3.2-1B with JSON grammar constraints.

/// System prompt for veterinary NER.
pub const SYSTEM_PROMPT: &str = r#"You are a veterinary medical assistant that extracts drug information from clinical transcripts.

Extract drug mentions with the following information:
- drug_name: The name of the drug (brand name, generic name, or common abbreviation)
- dose: Numeric dose value (if mentioned)
- unit: Dose unit (mg, mL, cc, tablets, etc.)
- route: Route of administration (orally, IV, IM, subcutaneously, etc.)
- species: Target species if mentioned (canine, feline, equine, etc.)

Common veterinary drug abbreviations:
- ace = acepromazine
- metacam = meloxicam
- rimadyl = carprofen
- cerenia = maropitant
- convenia = cefovecin
- baytril = enrofloxacin

Output JSON with "mentions" array containing extracted drug mentions."#;

/// User prompt template for NER extraction.
pub fn make_extraction_prompt(transcript: &str) -> String {
    format!(
        r#"Extract all drug mentions from this veterinary clinical transcript:

"{}"

Return a JSON object with a "mentions" array. Each mention should have:
- raw_text: The exact text containing the drug reference
- drug_name: The drug name
- dose: Numeric dose (number only, null if not specified)
- unit: Dose unit (null if not specified)
- route: Route of administration (null if not specified)
- species: Target species (null if not specified)
- start_offset: Character position where the mention starts
- end_offset: Character position where the mention ends"#,
        transcript
    )
}

/// JSON grammar constraint for llama.cpp to ensure valid output format.
pub const JSON_GRAMMAR: &str = r#"
root ::= object
object ::= "{" ws "\"mentions\"" ws ":" ws mentions ws "}"
mentions ::= "[" ws (mention (ws "," ws mention)*)? ws "]"
mention ::= "{" ws
    "\"raw_text\"" ws ":" ws string ws "," ws
    "\"drug_name\"" ws ":" ws string ws "," ws
    "\"dose\"" ws ":" ws (number | "null") ws "," ws
    "\"unit\"" ws ":" ws (string | "null") ws "," ws
    "\"route\"" ws ":" ws (string | "null") ws "," ws
    "\"species\"" ws ":" ws (string | "null") ws "," ws
    "\"start_offset\"" ws ":" ws number ws "," ws
    "\"end_offset\"" ws ":" ws number ws
"}"
string ::= "\"" ([^"\\] | "\\" .)* "\""
number ::= "-"? [0-9]+ ("." [0-9]+)?
ws ::= [ \t\n]*
"#;

/// Example few-shot prompts for better extraction accuracy.
pub const FEW_SHOT_EXAMPLES: &[(&str, &str)] = &[
    (
        "Give the dog 100mg of carprofen twice daily by mouth",
        r#"{"mentions":[{"raw_text":"100mg of carprofen twice daily by mouth","drug_name":"carprofen","dose":100,"unit":"mg","route":"by mouth","species":"dog","start_offset":13,"end_offset":52}]}"#
    ),
    (
        "Administer 0.5cc of acepromazine IM before surgery",
        r#"{"mentions":[{"raw_text":"0.5cc of acepromazine IM","drug_name":"acepromazine","dose":0.5,"unit":"cc","route":"IM","species":null,"start_offset":11,"end_offset":35}]}"#
    ),
    (
        "The cat needs metacam and also some cerenia for nausea",
        r#"{"mentions":[{"raw_text":"metacam","drug_name":"metacam","dose":null,"unit":null,"route":null,"species":"cat","start_offset":13,"end_offset":20},{"raw_text":"cerenia for nausea","drug_name":"cerenia","dose":null,"unit":null,"route":null,"species":"cat","start_offset":35,"end_offset":53}]}"#
    ),
];

/// Build a complete prompt with system context and few-shot examples.
pub fn build_full_prompt(transcript: &str, include_examples: bool) -> String {
    let mut prompt = String::new();

    // System context
    prompt.push_str("<|system|>\n");
    prompt.push_str(SYSTEM_PROMPT);
    prompt.push_str("\n<|end|>\n");

    // Few-shot examples
    if include_examples {
        for (input, output) in FEW_SHOT_EXAMPLES {
            prompt.push_str("<|user|>\n");
            prompt.push_str(&make_extraction_prompt(input));
            prompt.push_str("\n<|end|>\n");
            prompt.push_str("<|assistant|>\n");
            prompt.push_str(output);
            prompt.push_str("\n<|end|>\n");
        }
    }

    // Actual request
    prompt.push_str("<|user|>\n");
    prompt.push_str(&make_extraction_prompt(transcript));
    prompt.push_str("\n<|end|>\n");
    prompt.push_str("<|assistant|>\n");

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extraction_prompt() {
        let prompt = make_extraction_prompt("Give 10mg carprofen PO");
        assert!(prompt.contains("Give 10mg carprofen PO"));
        assert!(prompt.contains("drug_name"));
        assert!(prompt.contains("mentions"));
    }

    #[test]
    fn test_full_prompt_with_examples() {
        let prompt = build_full_prompt("Test transcript", true);
        assert!(prompt.contains("<|system|>"));
        assert!(prompt.contains("veterinary medical assistant"));
        assert!(prompt.contains("carprofen")); // From examples
        assert!(prompt.contains("Test transcript"));
    }

    #[test]
    fn test_full_prompt_without_examples() {
        let prompt = build_full_prompt("Test transcript", false);
        assert!(prompt.contains("<|system|>"));
        assert!(!prompt.contains("100mg of carprofen")); // No examples
        assert!(prompt.contains("Test transcript"));
    }
}

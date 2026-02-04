# fuzzy-drugs-llm

LLM wrapper crate for NER (Named Entity Recognition) extraction from veterinary transcripts.

## Purpose

Extracts structured drug mentions from free-text transcripts using LLM inference. Designed to work with Llama-3.2-1B via llama.cpp on device.

## Module Structure

```
src/
├── lib.rs          # Crate exports
├── prompts.rs      # NER extraction prompts with JSON grammar
└── extraction.rs   # DrugMention parsing and extraction
```

## Key Types

```rust
// From prompts.rs
pub struct ExtractionPrompt {
    system_prompt: String,
    json_grammar: String,
}

// From extraction.rs
pub struct DrugMention {
    pub drug_name: String,
    pub dose: Option<f64>,
    pub unit: Option<String>,
    pub route: Option<String>,
    pub species: Option<String>,
}

pub trait DrugExtractor {
    fn extract(&self, transcript: &str) -> Result<Vec<DrugMention>, ExtractionError>;
}
```

## Prompt Design

The system prompt instructs the LLM to:
1. Extract drug mentions from veterinary transcripts
2. Output structured JSON with drug_name, dose, unit, route
3. Handle common speech patterns ("give", "administer", etc.)
4. Preserve original phrasing when uncertain

Example prompt output:
```json
{
  "mentions": [
    {"drug_name": "rimadyl", "dose": 100, "unit": "mg", "route": "PO"},
    {"drug_name": "ace", "dose": 0.5, "unit": "cc", "route": "IM"}
  ]
}
```

## JSON Grammar

Constrains LLM output to valid JSON structure, preventing hallucination of invalid formats.

## Mock Extractor

`MockExtractor` in `extraction.rs` provides a rule-based fallback for testing without LLM:
- Recognizes common drug names and aliases
- Extracts dose patterns like "100mg", "0.5 cc"
- Identifies route keywords (PO, IM, IV, SQ)

## Integration

This crate provides the NER stage; output feeds into `fuzzy-drugs-core` resolver:

```
Transcript → DrugExtractor → Vec<DrugMention> → Normalizer → Disambiguator → ReviewQueue
```

## Future: llama.cpp Integration

```rust
// Planned API
pub struct LlamaExtractor {
    model: LlamaModel,
    prompt: ExtractionPrompt,
}

impl DrugExtractor for LlamaExtractor {
    fn extract(&self, transcript: &str) -> Result<Vec<DrugMention>, ExtractionError> {
        let prompt = self.prompt.format(transcript);
        let response = self.model.generate(&prompt, &self.prompt.json_grammar)?;
        parse_ner_output(&response)
    }
}
```

## Testing

```bash
cargo test -p fuzzy-drugs-llm
```

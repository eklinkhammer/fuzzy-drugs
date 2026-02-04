# Test Data

Golden test cases and fixtures for testing the fuzzy-drugs resolver.

## Structure

```
test-data/
└── golden/
    └── resolver_cases.json   # Golden test cases for resolver
```

## Golden Test Format

`resolver_cases.json` contains test cases for validating the semantic resolver:

```json
{
  "description": "Golden test cases for the semantic resolver",
  "version": "1.0",
  "cases": [
    {
      "id": "unique-test-id",
      "input": {
        "drug_name": "rimadyl",
        "dose": 100,
        "unit": "mg",
        "route": "PO",
        "species": "canine"
      },
      "expected": {
        "normalized_name": "carprofen",
        "normalized_dose": 100,
        "normalized_unit": "mg",
        "normalized_route": "PO",
        "expected_sku_prefix": "CARP"
      }
    }
  ]
}
```

### Input Fields

| Field | Type | Description |
|-------|------|-------------|
| drug_name | string | Raw drug name/alias from transcript |
| dose | number | Numeric dose value |
| unit | string | Unit as spoken (mg, cc, mcg, etc.) |
| route | string | Route as spoken (PO, orally, IM, etc.) |
| species | string | Patient species |

### Expected Fields

| Field | Type | Description |
|-------|------|-------------|
| normalized_name | string | Canonical drug name after alias expansion |
| normalized_dose | number | Dose after unit conversion |
| normalized_unit | string | Canonical unit (mg, mL) |
| normalized_route | string | Canonical route (PO, IM, IV, SQ) |
| expected_sku_prefix | string | (optional) SKU should start with this |

## Test Categories

Current test cases cover:

1. **Alias expansion**: rimadyl→carprofen, ace→acepromazine, metacam→meloxicam
2. **Unit conversion**: cc→mL, mcg→mg, g→mg
3. **Route canonicalization**: orally→PO, subcutaneously→SQ
4. **Multi-species drugs**: Different SKUs for canine vs feline

## Running Golden Tests

```bash
cargo test -p fuzzy-drugs-core --test resolver_golden_tests
```

## Adding Test Cases

1. Add new case to `resolver_cases.json`
2. Include unique `id` for identification
3. Ensure test covers a specific normalization or disambiguation scenario
4. Run tests to verify

## Future Expansions

Planned additions:
- Audio transcript samples (WAV files)
- End-to-end test cases with full audio→SKU pipeline
- Edge cases for ambiguous drug mentions
- Multi-drug transcript scenarios

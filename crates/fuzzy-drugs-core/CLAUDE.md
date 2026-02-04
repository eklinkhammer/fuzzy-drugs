# fuzzy-drugs-core

Core Rust library for the veterinary drug resolution system.

## Module Structure

```
src/
├── lib.rs          # UniFFI exports, FFI types, factory functions
├── db/             # SQLite database layer
│   ├── schema.rs   # SQL schema with FTS5, triggers
│   ├── catalog.rs  # Drug catalog CRUD + FTS search
│   ├── patients.rs # Patient CRUD with dual-ID (local/server)
│   ├── drafts.rs   # Encounter drafts (staging area)
│   └── merkle.rs   # Merkle node storage
├── merkle/         # Tamper-evident audit log
│   ├── tree.rs     # MerkleTree: commit, proof generation
│   ├── proof.rs    # MerkleProof verification
│   └── sync.rs     # Sync protocol with PIMS
├── resolver/       # Drug mention → SKU resolution
│   ├── normalizer.rs   # Alias expansion, unit conversion
│   └── disambiguator.rs # Multi-factor SKU scoring
├── export/         # Data export
│   ├── billing.rs     # JSON/CSV billing export
│   └── compliance.rs  # Merkle proofs for audit
└── models/         # Domain types
    ├── catalog.rs    # CatalogItem, DoseRange
    ├── patient.rs    # Patient
    ├── encounter.rs  # EncounterDraft, ReviewedEncounter
    └── resolution.rs # ResolvedItem, ScoredCandidate
```

## Key APIs

### Database
```rust
let db = Database::open("path/to/db.sqlite")?;
let db = Database::open_in_memory()?;  // For testing
```

### Resolver
```rust
let resolver = Resolver::new(&db);
let result = resolver.resolve(&mention, Some("canine"), Some(30.0))?;
// result.top_candidate.sku, result.top_candidate.confidence
```

### Merkle Tree
```rust
let tree = MerkleTree::new(&db);
let commit = tree.commit_encounter(&reviewed_encounter)?;
let proof = tree.generate_proof(&commit.leaf_hash)?;
assert!(tree.verify_proof(&proof)?);
```

## Scoring Weights (Disambiguator)

| Factor | Weight | Notes |
|--------|--------|-------|
| Name/alias match | 40% | Jaro-Winkler + Levenshtein |
| Species compatibility | 25% | 1.0 if compatible, 0.1 if not |
| Route compatibility | 20% | 1.0 if compatible, 0.2 if not |
| Dose plausibility | 15% | Based on mg/kg range |

## Drug Alias Map

Common aliases in `normalizer.rs`:
- rimadyl, novox → carprofen
- metacam → meloxicam
- ace, promace → acepromazine
- cerenia → maropitant
- convenia → cefovecin
- baytril → enrofloxacin
- dex → dexamethasone
- torb → butorphanol
- keppra → levetiracetam
- vetmedin → pimobendan
- lasix → furosemide
- dexdomitor → dexmedetomidine
- clavamox → amoxicillin-clavulanate

## Unit Conversions

- cc → mL (1:1)
- mcg, ug, μg → mg (÷1000)
- g → mg (×1000)

## Route Canonicalization

- orally, by mouth, oral → PO
- subcutaneously, subq, sq, subcutaneous → SQ
- intramuscularly, intramuscular → IM
- intravenously, intravenous → IV

## Testing

```bash
# Run all tests
SDKROOT=$(xcrun --show-sdk-path) cargo test -p fuzzy-drugs-core

# Run specific module tests
cargo test -p fuzzy-drugs-core merkle::
cargo test -p fuzzy-drugs-core resolver::
cargo test -p fuzzy-drugs-core db::
```

## UniFFI Notes

- Uses proc-macro approach (`#[uniffi::export]`), not UDL scaffolding
- `uniffi::setup_scaffolding!()` in lib.rs
- FFI types are separate structs with `#[derive(uniffi::Record)]`
- Errors use `#[derive(uniffi::Error)]` with thiserror

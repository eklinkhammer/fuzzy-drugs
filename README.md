# Fuzzy-Drugs

Local-first veterinary field tool for iPad: ambient audio capture → offline ASR/NER → SKU resolution → vet review → Merkle tree commit → sync to PIMS.

## Core Principle

**All drug resolutions require vet review.** No auto-acceptance regardless of confidence score.

**Dual Purpose:** Billing records AND compliance/audit trail (tamper-evident via Merkle tree).

## Architecture

```
┌─────────────────────────────────────────┐
│           SwiftUI App                   │
│  • AVFoundation (Audio)                 │
│  • WhisperKit (ASR via CoreML/ANE)      │
│  • Vet Review Queue UI                  │
├─────────────────────────────────────────┤
│           UniFFI Bindings               │
├─────────────────────────────────────────┤
│         Rust Core (fuzzy-drugs-core)    │
│  • Merkle Tree (audit + storage)        │
│  • SQLite + FTS5 (catalog, indexes)     │
│  • Semantic Resolver                    │
│  • Sync Engine                          │
└─────────────────────────────────────────┘
```

## Project Structure

```
fuzzy-drugs/
├── Cargo.toml                      # Workspace root
├── crates/
│   ├── fuzzy-drugs-core/           # Main library
│   │   ├── src/
│   │   │   ├── db/                 # SQLite schema, CRUD
│   │   │   ├── merkle/             # Merkle tree impl, sync protocol
│   │   │   ├── resolver/           # Normalizer, disambiguator
│   │   │   ├── export/             # Billing/compliance export
│   │   │   └── models/             # Domain types
│   │   └── tests/                  # Integration tests
│   └── fuzzy-drugs-llm/            # llama.cpp wrapper for NER
├── ios/
│   └── FuzzyDrugs/                 # Xcode project
│       ├── Audio/                  # AudioRecorder, TranscriptionService
│       ├── Views/                  # SwiftUI screens
│       └── Sync/                   # SyncManager
├── test-data/
│   └── golden/                     # Resolver test cases
└── scripts/
    └── build-xcframework.sh        # Build iOS framework
```

## Building

### Rust Core

```bash
# Run tests
cd crates/fuzzy-drugs-core && cargo test

# Run all tests including integration tests
cargo test --workspace
```

### iOS Framework

```bash
# Build XCFramework for iOS
./scripts/build-xcframework.sh

# Then open in Xcode
open ios/FuzzyDrugs/FuzzyDrugs.xcodeproj
```

## Key Components

### Merkle Tree

The Merkle tree is the **source of truth** for all encounter data:

- **Tamper-evident audit log** — Each leaf is SHA-256 hashed; any modification invalidates the root
- **Append-only storage** — Encounters committed after vet review become immutable leaves
- **Efficient sync** — Send only tree diff (missing subtrees) to PIMS
- **Exportable** — Full tree or subtree export for compliance audits

### Semantic Resolver

Three-stage pipeline:
1. **NER Extraction** — Llama 3.2-1B via llama.cpp with JSON grammar constraint
2. **Normalization** — Unit conversion, alias expansion, route canonicalization
3. **Disambiguation** — FTS5 candidate retrieval, multi-factor scoring

Scoring weights:
- Name/alias match quality: 40%
- Species compatibility: 25%
- Route compatibility: 20%
- Dose plausibility: 15%

### Data Flow

```
Audio → Transcription → NER → Normalization → Disambiguation
                                                    │
                                    [STAGING: encounter_draft]
                                                    │
                                            Vet Review Queue
                                                    │
                                        Vet approves/edits
                                                    │
                                    ┌───────────────▼───────────────┐
                                    │      Merkle Tree Commit       │
                                    │  leaf = hash(encounter_json)  │
                                    │  update root                  │
                                    └───────────────┬───────────────┘
                                                    │
                            ┌───────────────────────┼───────────────────────┐
                            │                       │                       │
                            ▼                       ▼                       ▼
                        Billing               Compliance              PIMS Sync
                        Export                  Export              (when online)
```

## Models

| Component | Model | Size | Notes |
|-----------|-------|------|-------|
| ASR | WhisperKit `whisper-small-en` | ~460MB | Native Swift, ANE-optimized |
| NER/Disambiguation | Llama-3.2-1B (Q4_K_M) | ~700MB | Via llama.cpp, JSON grammar |

## License

MIT

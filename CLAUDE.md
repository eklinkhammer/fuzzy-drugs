# Fuzzy-Drugs Project

Local-first veterinary field tool for iPad: ambient audio capture → offline ASR/NER → SKU resolution → vet review → Merkle tree commit → sync to PIMS.

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

## Core Principle

**All drug resolutions require vet review.** No auto-acceptance regardless of confidence score.

## Project Structure

- `crates/fuzzy-drugs-core/` - Main Rust library with SQLite, Merkle tree, resolver, exports
- `crates/fuzzy-drugs-llm/` - LLM wrapper for NER extraction prompts
- `ios/FuzzyDrugs/` - SwiftUI app with audio capture and review UI
- `scripts/` - Build and test scripts
- `test-data/golden/` - Golden test cases for resolver

## Building

```bash
# Run tests (sets SDKROOT automatically on macOS)
./scripts/run-tests.sh

# Build XCFramework for iOS
./scripts/build-xcframework.sh

# Manual test run
SDKROOT=$(xcrun --show-sdk-path) cargo test --workspace
```

## Key Dependencies

- `rusqlite` 0.32 with bundled SQLite (includes FTS5)
- `uniffi` 0.28 for Rust-Swift FFI (proc-macro approach)
- `sha2` for Merkle tree hashing
- `strsim` for fuzzy string matching

## Development Notes

- macOS requires `SDKROOT` set for linking (CoreFoundation framework)
- UniFFI uses proc macros, not UDL file (see `src/lib.rs`)
- Tests use in-memory SQLite databases
- Merkle tree is append-only; leaves are committed encounters

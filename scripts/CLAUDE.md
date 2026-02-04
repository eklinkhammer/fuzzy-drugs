# Build Scripts

Scripts for building and testing the fuzzy-drugs project.

## Scripts

### run-tests.sh

Runs the full test suite for all Rust crates.

```bash
./scripts/run-tests.sh
```

Features:
- Automatically sets `SDKROOT` on macOS (fixes CoreFoundation linking)
- Runs clippy if available
- Runs all workspace tests
- Checks formatting with rustfmt

### build-xcframework.sh

Builds the XCFramework for iOS integration.

```bash
./scripts/build-xcframework.sh
```

Steps performed:
1. Sets `SDKROOT` for proper SDK resolution
2. Installs required Rust targets (aarch64-apple-ios, aarch64-apple-ios-sim, x86_64-apple-ios)
3. Installs uniffi-bindgen if needed
4. Builds release for all iOS targets
5. Creates fat library for simulator (arm64 + x86_64)
6. Generates Swift bindings using uniffi-bindgen
7. Creates XCFramework at `ios/FuzzyDrugs/Bridge/FuzzyDrugsCore.xcframework`

Output:
- `ios/FuzzyDrugs/Bridge/FuzzyDrugsCore.xcframework` - Universal framework
- `ios/FuzzyDrugs/Bridge/fuzzy_drugs_core.swift` - Swift bindings

## Common Issues

### CoreFoundation not found

```
ld: framework 'CoreFoundation' not found
```

Fix: Set SDKROOT before running cargo:
```bash
export SDKROOT=$(xcrun --show-sdk-path)
```

The scripts do this automatically.

### uniffi-bindgen not found

```
uniffi-bindgen: command not found
```

Fix: Install it:
```bash
cargo install uniffi_bindgen
```

The build script does this automatically.

### Missing Rust targets

```
error: target 'aarch64-apple-ios' not found
```

Fix: Install targets:
```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
```

The build script does this automatically.

## Manual Commands

```bash
# Run tests with SDKROOT
SDKROOT=$(xcrun --show-sdk-path) cargo test --workspace

# Build release
SDKROOT=$(xcrun --show-sdk-path) cargo build --workspace --release

# Check without building
cargo check --workspace

# Format code
cargo fmt

# Run clippy
cargo clippy --workspace
```

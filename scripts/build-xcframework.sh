#!/bin/bash
set -e

# Build XCFramework for fuzzy-drugs-core

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/.."
CORE_DIR="$PROJECT_ROOT/crates/fuzzy-drugs-core"
OUTPUT_DIR="$PROJECT_ROOT/ios/FuzzyDrugs/Bridge"

echo "=== Building fuzzy-drugs-core XCFramework ==="

# Set SDKROOT if not already set (fixes CoreFoundation linking on some systems)
if [ -z "$SDKROOT" ]; then
    export SDKROOT=$(xcrun --show-sdk-path)
    echo "Set SDKROOT to $SDKROOT"
fi

# Ensure we have the required Rust targets
echo "Installing Rust targets..."
rustup target add aarch64-apple-ios
rustup target add aarch64-apple-ios-sim
rustup target add x86_64-apple-ios

# Install uniffi-bindgen-cli if not present
if ! command -v uniffi-bindgen &> /dev/null; then
    echo "Installing uniffi-bindgen..."
    cargo install uniffi_bindgen
fi

# Build for iOS device (arm64)
echo "Building for iOS device (aarch64-apple-ios)..."
cargo build --manifest-path "$CORE_DIR/Cargo.toml" --release --target aarch64-apple-ios

# Build for iOS simulator (arm64 for Apple Silicon)
echo "Building for iOS simulator (aarch64-apple-ios-sim)..."
cargo build --manifest-path "$CORE_DIR/Cargo.toml" --release --target aarch64-apple-ios-sim

# Build for iOS simulator (x86_64 for Intel Macs)
echo "Building for iOS simulator (x86_64-apple-ios)..."
cargo build --manifest-path "$CORE_DIR/Cargo.toml" --release --target x86_64-apple-ios

# Create fat library for simulator
echo "Creating fat library for simulator..."
SIMULATOR_LIB_DIR="$PROJECT_ROOT/target/universal-ios-sim/release"
mkdir -p "$SIMULATOR_LIB_DIR"

lipo -create \
    "$PROJECT_ROOT/target/aarch64-apple-ios-sim/release/libfuzzy_drugs_core.a" \
    "$PROJECT_ROOT/target/x86_64-apple-ios/release/libfuzzy_drugs_core.a" \
    -output "$SIMULATOR_LIB_DIR/libfuzzy_drugs_core.a"

# Generate Swift bindings using proc-macro approach
echo "Generating Swift bindings..."
mkdir -p "$OUTPUT_DIR"

# Use the built library to generate bindings
uniffi-bindgen generate \
    --library "$PROJECT_ROOT/target/aarch64-apple-ios/release/libfuzzy_drugs_core.a" \
    --language swift \
    --out-dir "$OUTPUT_DIR"

# Create XCFramework
echo "Creating XCFramework..."
XCFRAMEWORK_PATH="$OUTPUT_DIR/FuzzyDrugsCore.xcframework"
rm -rf "$XCFRAMEWORK_PATH"

xcodebuild -create-xcframework \
    -library "$PROJECT_ROOT/target/aarch64-apple-ios/release/libfuzzy_drugs_core.a" \
    -headers "$OUTPUT_DIR" \
    -library "$SIMULATOR_LIB_DIR/libfuzzy_drugs_core.a" \
    -headers "$OUTPUT_DIR" \
    -output "$XCFRAMEWORK_PATH"

echo "=== XCFramework created at $XCFRAMEWORK_PATH ==="

# Clean up intermediate files
echo "Cleaning up..."
rm -f "$OUTPUT_DIR/fuzzy_drugs_coreFFI.h"
rm -f "$OUTPUT_DIR/fuzzy_drugs_coreFFI.modulemap"

echo "=== Done! ==="
echo ""
echo "Next steps:"
echo "1. Open ios/FuzzyDrugs/FuzzyDrugs.xcodeproj in Xcode"
echo "2. Add FuzzyDrugsCore.xcframework to the project"
echo "3. Add fuzzy_drugs_core.swift to the project"
echo "4. Build and run"

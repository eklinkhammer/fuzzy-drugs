#!/bin/bash
set -e

# Build XCFramework for fuzzy-drugs-core

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/.."
CORE_DIR="$PROJECT_ROOT/crates/fuzzy-drugs-core"
OUTPUT_DIR="$PROJECT_ROOT/ios/FuzzyDrugs/Bridge"

echo "=== Building fuzzy-drugs-core XCFramework ==="

# Clear environment variables that might interfere with cross-compilation
unset CFLAGS
unset CPPFLAGS
unset LDFLAGS

# Prefer rustup-installed toolchain if available
if [ -f "$HOME/.cargo/bin/cargo" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
    echo "Using rustup toolchain"
fi

# Ensure we have the required Rust targets
echo "Installing Rust targets..."
rustup target add aarch64-apple-ios
rustup target add aarch64-apple-ios-sim
rustup target add x86_64-apple-ios

cd "$PROJECT_ROOT"

# Clean previous iOS builds
echo "Cleaning previous iOS builds..."
cargo clean --target aarch64-apple-ios 2>/dev/null || true
cargo clean --target aarch64-apple-ios-sim 2>/dev/null || true
cargo clean --target x86_64-apple-ios 2>/dev/null || true

# Set iOS deployment target
export IPHONEOS_DEPLOYMENT_TARGET=15.0

# Build for iOS device (arm64)
echo "Building for iOS device (aarch64-apple-ios)..."
cargo build -p fuzzy-drugs-core --release --target aarch64-apple-ios

# Build for iOS simulator (arm64 for Apple Silicon)
echo "Building for iOS simulator (aarch64-apple-ios-sim)..."
cargo build -p fuzzy-drugs-core --release --target aarch64-apple-ios-sim

# Build for iOS simulator (x86_64 for Intel Macs)
echo "Building for iOS simulator (x86_64-apple-ios)..."
cargo build -p fuzzy-drugs-core --release --target x86_64-apple-ios

# Create fat library for simulator
echo "Creating fat library for simulator..."
SIMULATOR_LIB_DIR="$PROJECT_ROOT/target/universal-ios-sim/release"
mkdir -p "$SIMULATOR_LIB_DIR"

lipo -create \
    "$PROJECT_ROOT/target/aarch64-apple-ios-sim/release/libfuzzy_drugs_core.a" \
    "$PROJECT_ROOT/target/x86_64-apple-ios/release/libfuzzy_drugs_core.a" \
    -output "$SIMULATOR_LIB_DIR/libfuzzy_drugs_core.a"

# Generate Swift bindings
echo "Generating Swift bindings..."
mkdir -p "$OUTPUT_DIR"

# Temporarily add cdylib for host build to generate bindings
echo "Building host dylib for binding generation..."
CARGO_TOML="$CORE_DIR/Cargo.toml"

# Backup and modify Cargo.toml to add cdylib
cp "$CARGO_TOML" "$CARGO_TOML.bak"
sed -i '' 's/crate-type = \["lib", "staticlib"\]/crate-type = ["lib", "staticlib", "cdylib"]/' "$CARGO_TOML"

# Build for host with cdylib
export SDKROOT=$(xcrun --show-sdk-path)
cargo build -p fuzzy-drugs-core --release

# Restore Cargo.toml
mv "$CARGO_TOML.bak" "$CARGO_TOML"

# Generate bindings using the uniffi-bindgen binary
cargo run -p fuzzy-drugs-core --bin uniffi-bindgen -- \
    generate --library "$PROJECT_ROOT/target/release/libfuzzy_drugs_core.dylib" \
    --language swift \
    --out-dir "$OUTPUT_DIR"

# Create XCFramework
echo "Creating XCFramework..."
XCFRAMEWORK_PATH="$OUTPUT_DIR/FuzzyDrugsCore.xcframework"
rm -rf "$XCFRAMEWORK_PATH"

# Move the generated header to include directory
mkdir -p "$OUTPUT_DIR/include"
if [ -f "$OUTPUT_DIR/fuzzy_drugs_coreFFI.h" ]; then
    mv "$OUTPUT_DIR/fuzzy_drugs_coreFFI.h" "$OUTPUT_DIR/include/"
fi

# Create module map
cat > "$OUTPUT_DIR/include/module.modulemap" << 'MODULEMAP_EOF'
module FuzzyDrugsCoreFFI {
    header "fuzzy_drugs_coreFFI.h"
    export *
}
MODULEMAP_EOF

xcodebuild -create-xcframework \
    -library "$PROJECT_ROOT/target/aarch64-apple-ios/release/libfuzzy_drugs_core.a" \
    -headers "$OUTPUT_DIR/include" \
    -library "$SIMULATOR_LIB_DIR/libfuzzy_drugs_core.a" \
    -headers "$OUTPUT_DIR/include" \
    -output "$XCFRAMEWORK_PATH"

echo "=== XCFramework created at $XCFRAMEWORK_PATH ==="

# Clean up intermediate files
rm -rf "$OUTPUT_DIR/include"

echo ""
echo "=== Done! ==="
echo ""
echo "Generated files:"
echo "  - $XCFRAMEWORK_PATH"
echo "  - $OUTPUT_DIR/fuzzy_drugs_core.swift"
echo ""
echo "Next steps:"
echo "1. Open ios/FuzzyDrugs/FuzzyDrugs.xcodeproj in Xcode"
echo "2. Add FuzzyDrugsCore.xcframework to the project"
echo "3. Add fuzzy_drugs_core.swift to the project"
echo "4. Build and run"

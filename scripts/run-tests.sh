#!/bin/bash
set -e

# Run all tests for fuzzy-drugs

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/.."

echo "=== Running fuzzy-drugs test suite ==="
echo ""

# Set SDKROOT if not already set (fixes CoreFoundation linking on macOS)
if [ -z "$SDKROOT" ]; then
    export SDKROOT=$(xcrun --show-sdk-path 2>/dev/null || echo "")
    if [ -n "$SDKROOT" ]; then
        echo "Set SDKROOT to $SDKROOT"
    fi
fi

cd "$PROJECT_ROOT"

# Check code with clippy (if available)
if command -v cargo-clippy &> /dev/null; then
    echo "=== Running clippy ==="
    cargo clippy --workspace -- -D warnings || {
        echo "Clippy found issues, continuing anyway..."
    }
    echo ""
fi

# Run unit tests
echo "=== Running unit tests ==="
cargo test --workspace
echo ""

# Run integration tests if they exist
if [ -d "tests" ]; then
    echo "=== Running integration tests ==="
    cargo test --test '*'
    echo ""
fi

# Check formatting (if rustfmt is available)
if command -v rustfmt &> /dev/null; then
    echo "=== Checking formatting ==="
    cargo fmt --check || {
        echo "Formatting issues found. Run 'cargo fmt' to fix."
    }
    echo ""
fi

echo "=== All tests complete! ==="

#!/bin/bash
# SmartRefresh Docker Build Script
# Runs inside the Docker container to build all components

set -e

PLUGIN_NAME="SmartRefresh"
OUTPUT_DIR="/output"

echo "=== SmartRefresh Docker Build ==="
echo ""

# Step 1: Build Rust backend
echo "[1/4] Building Rust daemon..."
cd /build/backend
cargo build --release

# Binary is at target/release/smart-refresh-daemon
BINARY_PATH="target/release/smart-refresh-daemon"
if [ ! -f "$BINARY_PATH" ]; then
    echo "ERROR: Binary not found at $BINARY_PATH"
    exit 1
fi

# Strip is already done via Cargo.toml profile, but ensure it
strip "$BINARY_PATH" 2>/dev/null || true
echo "Binary size: $(du -h $BINARY_PATH | cut -f1)"

# Step 2: Copy binary to bin/
echo "[2/4] Copying binary to bin/..."
cd /build
mkdir -p bin
cp "backend/$BINARY_PATH" "bin/"
chmod +x bin/smart-refresh-daemon

# Step 3: Build frontend
echo "[3/4] Building React frontend..."
cd /build/frontend
pnpm install --frozen-lockfile 2>/dev/null || pnpm install
pnpm run build

# Step 4: Package plugin
echo "[4/4] Creating plugin ZIP..."
cd /build

# Create package structure
PACKAGE_DIR=$(mktemp -d)
PLUGIN_DIR="$PACKAGE_DIR/$PLUGIN_NAME"
mkdir -p "$PLUGIN_DIR"

# Copy required files
cp -r bin "$PLUGIN_DIR/"
cp -r frontend/dist "$PLUGIN_DIR/"
cp main.py "$PLUGIN_DIR/"
cp plugin.json "$PLUGIN_DIR/"
cp LICENSE "$PLUGIN_DIR/" 2>/dev/null || true
cp README.md "$PLUGIN_DIR/" 2>/dev/null || true

# Create ZIP
ZIP_NAME="${PLUGIN_NAME}.zip"
cd "$PACKAGE_DIR"
zip -r "/build/$ZIP_NAME" "$PLUGIN_NAME"

# Copy to output directory
mkdir -p "$OUTPUT_DIR"
cp "/build/$ZIP_NAME" "$OUTPUT_DIR/"

# Also copy the binary separately for debugging
cp "/build/bin/smart-refresh-daemon" "$OUTPUT_DIR/"

# Cleanup
rm -rf "$PACKAGE_DIR"

echo ""
echo "=== Build Complete ==="
echo "Output files in $OUTPUT_DIR:"
ls -la "$OUTPUT_DIR"

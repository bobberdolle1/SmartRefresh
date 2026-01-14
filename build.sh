#!/bin/bash
# SmartRefresh Build Script
# Compiles Rust daemon for Linux, builds frontend, and packages plugin

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_NAME="smart-refresh"
TARGET="x86_64-unknown-linux-gnu"

echo "=== SmartRefresh Build Script ==="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_step() {
    echo -e "${GREEN}[STEP]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Step 1: Build Rust backend
print_step "Building Rust daemon for target: $TARGET"
cd "$SCRIPT_DIR/backend"

# Check if cross-compilation target is installed
if ! rustup target list --installed | grep -q "$TARGET"; then
    print_warning "Target $TARGET not installed. Installing..."
    rustup target add "$TARGET"
fi

cargo build --release --target "$TARGET"

# Step 2: Strip debug symbols from binary
print_step "Stripping debug symbols from binary"
BINARY_PATH="target/$TARGET/release/smart-refresh-daemon"

if [ -f "$BINARY_PATH" ]; then
    # Use strip if available, otherwise rely on Cargo.toml strip = true
    if command -v strip &> /dev/null; then
        strip "$BINARY_PATH" 2>/dev/null || print_warning "strip command failed, binary may already be stripped"
    elif command -v x86_64-linux-gnu-strip &> /dev/null; then
        x86_64-linux-gnu-strip "$BINARY_PATH" 2>/dev/null || print_warning "strip command failed, binary may already be stripped"
    else
        print_warning "strip command not found, relying on Cargo.toml strip setting"
    fi
else
    print_error "Binary not found at $BINARY_PATH"
    exit 1
fi

# Step 3: Copy binary to bin/ directory
print_step "Copying binary to bin/ directory"
cd "$SCRIPT_DIR"
mkdir -p bin
cp "backend/target/$TARGET/release/smart-refresh-daemon" "bin/"
echo "Binary size: $(du -h bin/smart-refresh-daemon | cut -f1)"

# Step 4: Build frontend
print_step "Building React frontend"
cd "$SCRIPT_DIR/frontend"

if command -v pnpm &> /dev/null; then
    pnpm install --frozen-lockfile 2>/dev/null || pnpm install
    pnpm run build
elif command -v npm &> /dev/null; then
    print_warning "pnpm not found, falling back to npm"
    npm install
    npm run build
else
    print_error "Neither pnpm nor npm found. Please install pnpm."
    exit 1
fi

# Step 5: Create ZIP archive
print_step "Creating plugin ZIP archive"
cd "$SCRIPT_DIR"

# Create temporary directory for packaging
PACKAGE_DIR=$(mktemp -d)
PLUGIN_DIR="$PACKAGE_DIR/$PLUGIN_NAME"
mkdir -p "$PLUGIN_DIR"

# Copy required files
cp -r bin "$PLUGIN_DIR/"
cp -r frontend/dist "$PLUGIN_DIR/"
cp main.py "$PLUGIN_DIR/"
cp plugin.json "$PLUGIN_DIR/"

# Create ZIP archive
ZIP_NAME="${PLUGIN_NAME}.zip"
rm -f "$ZIP_NAME"
cd "$PACKAGE_DIR"
zip -r "$SCRIPT_DIR/$ZIP_NAME" "$PLUGIN_NAME"
cd "$SCRIPT_DIR"

# Cleanup
rm -rf "$PACKAGE_DIR"

echo ""
echo -e "${GREEN}=== Build Complete ===${NC}"
echo "Output files:"
echo "  - bin/smart-refresh-daemon"
echo "  - frontend/dist/"
echo "  - $ZIP_NAME"
echo ""
echo "ZIP archive ready for deployment: $ZIP_NAME"

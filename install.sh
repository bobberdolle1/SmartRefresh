#!/bin/bash

# SmartRefresh Installer for Decky Loader
# https://github.com/bobberdolle1/SmartRefresh

set -e

REPO="bobberdolle1/SmartRefresh"
PLUGIN_NAME="SmartRefresh"
PLUGIN_DIR="$HOME/homebrew/plugins/$PLUGIN_NAME"

echo "=== SmartRefresh Installer ==="
echo ""

# Check if Decky Loader is installed
if [ ! -d "$HOME/homebrew/plugins" ]; then
    echo "Error: Decky Loader not found!"
    echo "Please install Decky Loader first: https://github.com/SteamDeckHomebrew/decky-loader"
    exit 1
fi

# Get latest release URL
echo "Fetching latest release..."
RELEASE_URL=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep "browser_download_url.*zip" | cut -d '"' -f 4)

if [ -z "$RELEASE_URL" ]; then
    echo "Error: Could not find latest release"
    exit 1
fi

# Create temp directory
TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"

# Download release
echo "Downloading SmartRefresh..."
curl -L -o smartrefresh.zip "$RELEASE_URL"

# Remove old installation if exists
if [ -d "$PLUGIN_DIR" ]; then
    echo "Removing old installation..."
    rm -rf "$PLUGIN_DIR"
fi

# Extract to plugins directory
echo "Installing to $PLUGIN_DIR..."
mkdir -p "$PLUGIN_DIR"
unzip -q smartrefresh.zip -d "$PLUGIN_DIR"

# Cleanup
cd /
rm -rf "$TEMP_DIR"

echo ""
echo "=== Installation Complete ==="
echo "Please restart Decky Loader or reboot your Steam Deck."
echo ""

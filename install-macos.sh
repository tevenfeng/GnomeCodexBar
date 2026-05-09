#!/bin/bash
set -euo pipefail

# GNOME Codex Bar - macOS Installation Script
# Installs the Rust CLI daemon + SwiftUI menu bar app

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR"
CLI_DIR="$PROJECT_DIR/cli"
MACOS_BAR_DIR="$PROJECT_DIR/macos-bar"

BIN_DIR="$HOME/.local/bin"
APP_DIR="$HOME/Applications"

echo "=== GNOME Codex Bar Installer (macOS) ==="
echo ""

# Step 1: Build and install CLI
echo "[1/4] Building Rust CLI..."
cd "$CLI_DIR"
cargo build --release 2>&1 | tail -3

echo "      Installing CLI to $BIN_DIR..."
if [ ! -w "$BIN_DIR" ] && [ "$(id -u)" -ne 0 ]; then
    echo "      $BIN_DIR is not writable. Using sudo..."
    sudo mkdir -p "$BIN_DIR"
    sudo cp target/release/codex-bar-cli "$BIN_DIR/"
    sudo chmod +x "$BIN_DIR/codex-bar-cli"
else
    mkdir -p "$BIN_DIR"
    cp target/release/codex-bar-cli "$BIN_DIR/"
    chmod +x "$BIN_DIR/codex-bar-cli"
fi

# Step 2: Build SwiftUI menu bar app
echo ""
echo "[2/4] Building CodexBar menu bar app..."
cd "$MACOS_BAR_DIR"
swift build -c release 2>&1 | tail -3

# Step 3: Install menu bar app as .app bundle
echo ""
echo "[3/4] Installing CodexBar.app to $APP_DIR..."

# Kill running CodexBar app before deploying
if pgrep -x "CodexBar" >/dev/null 2>&1; then
    echo "      Stopping running CodexBar.app..."
    pkill -x "CodexBar" 2>/dev/null || true
    sleep 1
fi

mkdir -p "$APP_DIR"

APP_BUNDLE="$APP_DIR/CodexBar.app"
BUNDLE_CONTENTS="$APP_BUNDLE/Contents"
BUNDLE_MACOS="$BUNDLE_CONTENTS/MacOS"
BUNDLE_RESOURCES="$BUNDLE_CONTENTS/Resources"

rm -rf "$APP_BUNDLE"
mkdir -p "$BUNDLE_MACOS"
mkdir -p "$BUNDLE_RESOURCES"

# Copy binary
cp .build/release/CodexBar "$BUNDLE_MACOS/"

# Copy icon
if [ -f "$PROJECT_DIR/logo-output/AppIcon.icns" ]; then
    cp "$PROJECT_DIR/logo-output/AppIcon.icns" "$BUNDLE_RESOURCES/AppIcon.icns"
fi

# Create Info.plist
cat > "$BUNDLE_CONTENTS/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>CodexBar</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleIdentifier</key>
    <string>com.codexbar.macos</string>
    <key>CFBundleName</key>
    <string>CodexBar</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

# Create PkgInfo
echo -n "APPL????" > "$BUNDLE_CONTENTS/PkgInfo"

# Step 4: Verify
echo ""
echo "[4/4] Verifying installation..."

if command -v codex-bar-cli &>/dev/null; then
    echo "      CLI: OK ($(codex-bar-cli --version 2>/dev/null || echo "codex-bar-cli"))"
else
    echo "      CLI: WARNING - not found in PATH"
fi

if [ -d "$APP_BUNDLE" ]; then
    echo "      Menu Bar App: OK ($APP_BUNDLE)"
else
    echo "      Menu Bar App: FAILED"
fi

echo ""
echo "=== Installation Complete ==="
echo ""
echo "Next steps:"
echo "  1. Configure DeepSeek:  codex-bar-cli config deepseek --api-key <YOUR_KEY>"
echo "  2. Configure StepFun:   codex-bar-cli config stepfun --username <EMAIL> --password <PASS>"
echo "     Configure OpenCode Go (optional):"
echo "                         codex-bar-cli config opencodego --enabled true --cookie-header '<COOKIE_HEADER>' --workspace-id 'wrk_...'"
echo "  3. Start daemon:        codex-bar-cli daemon"
echo "  4. Enable auto-start:   codex-bar-cli autostart enable"
echo "  5. Launch menu bar app: open $APP_BUNDLE"
echo ""
echo "To test immediately:      codex-bar-cli fetch"
echo "To check status:          codex-bar-cli status"

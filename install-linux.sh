#!/bin/bash
set -euo pipefail

# GNOME Codex Bar - Linux (ZorinOS) Installation Script
# Installs the Rust CLI and the GNOME Shell Extension

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR"
CLI_DIR="$PROJECT_DIR/cli"
EXT_DIR="$PROJECT_DIR/gnome-shell-extension"

EXT_UUID="codex-bar@gnome"
EXT_INSTALL_DIR="$HOME/.local/share/gnome-shell/extensions/$EXT_UUID"
SCHEMA_DIR="$HOME/.local/share/glib-2.0/schemas"
BIN_DIR="$HOME/.local/bin"

echo "=== GNOME Codex Bar Installer (Linux) ==="
echo ""

# Step 1: Build and install CLI
echo "[1/4] Building Rust CLI..."
cd "$CLI_DIR"
cargo build --release

echo "      Installing CLI to $BIN_DIR..."
mkdir -p "$BIN_DIR"
cp target/release/codex-bar-cli "$BIN_DIR/"
chmod +x "$BIN_DIR/codex-bar-cli"

# Add to PATH if not already
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    echo "      Note: Add $BIN_DIR to your PATH if not already present."
    echo "      Add this to your ~/.bashrc or ~/.zshrc:"
    echo "      export PATH=\"\$HOME/.local/bin:\$PATH\""
fi

# Step 2: Install GNOME Shell Extension
echo ""
echo "[2/4] Installing GNOME Shell Extension..."
mkdir -p "$EXT_INSTALL_DIR"
cp -r "$EXT_DIR"/* "$EXT_INSTALL_DIR/"

# Step 3: Install GSettings schema
echo ""
echo "[3/4] Installing GSettings schema..."
mkdir -p "$SCHEMA_DIR"
cp "$EXT_DIR/schemas/org.gnome.shell.extensions.codex-bar.gschema.xml" "$SCHEMA_DIR/"
glib-compile-schemas "$SCHEMA_DIR" 2>/dev/null || {
    echo "      Warning: glib-compile-schemas not found. Schema may not take effect until next login."
    echo "      Install glib2-devel or similar package and re-run this script."
}

# Step 4: Verify
echo ""
echo "[4/4] Verifying installation..."

if command -v codex-bar-cli &>/dev/null || [ -x "$BIN_DIR/codex-bar-cli" ]; then
    echo "      CLI: OK ($("$BIN_DIR/codex-bar-cli" --version 2>/dev/null || echo "codex-bar-cli"))"
else
    echo "      CLI: WARNING - not found in PATH"
fi

if [ -d "$EXT_INSTALL_DIR" ] && [ -f "$EXT_INSTALL_DIR/metadata.json" ]; then
    echo "      Extension: OK ($EXT_INSTALL_DIR)"
else
    echo "      Extension: FAILED"
fi

echo ""
echo "=== Installation Complete ==="
echo ""
echo "Next steps:"
echo "  1. Configure DeepSeek:  codex-bar-cli config deepseek --api-key <YOUR_KEY>"
echo "  2. Configure StepFun:   codex-bar-cli config stepfun --username <EMAIL> --password <PASS>"
echo "     Configure OpenCode Go (optional):"
echo "                         codex-bar-cli config opencodego --enabled true --cookie-header '<COOKIE_HEADER>' --workspace-id 'wrk_...'"
echo "  3. Start daemon:        codex-bar-cli daemon &"
echo "  4. Enable extension:    gnome-extensions enable codex-bar@gnome"
echo "     (or use GNOME Extensions app)"
echo "  5. Restart GNOME Shell: Alt+F2, type 'r', press Enter (X11)"
echo "     or logout/login (Wayland)"
echo "  6. Enable auto-start:   codex-bar-cli autostart enable"
echo ""
echo "To test immediately:      codex-bar-cli fetch"
echo "To check status:          codex-bar-cli status"

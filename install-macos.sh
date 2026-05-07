#!/bin/bash
set -euo pipefail

# GNOME Codex Bar - macOS Installation Script
# Installs the Rust CLI (daemon mode, no GNOME Shell Extension on macOS)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR"
CLI_DIR="$PROJECT_DIR/cli"

BIN_DIR="/usr/local/bin"

echo "=== GNOME Codex Bar Installer (macOS) ==="
echo ""

# Step 1: Build CLI
echo "[1/2] Building Rust CLI..."
cd "$CLI_DIR"
cargo build --release 2>&1 | tail -3

# Step 2: Install CLI
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

# Verify
echo ""
echo "[2/2] Verifying installation..."

if command -v codex-bar-cli &>/dev/null; then
    echo "      CLI: OK ($(codex-bar-cli --version 2>/dev/null || echo "codex-bar-cli"))"
else
    echo "      CLI: WARNING - not found in PATH"
fi

echo ""
echo "=== Installation Complete ==="
echo ""
echo "Note: GNOME Shell Extension is not available on macOS."
echo "      The CLI daemon runs in the background and writes status.json."
echo ""
echo "Next steps:"
echo "  1. Configure DeepSeek:  codex-bar-cli config deepseek --api-key <YOUR_KEY>"
echo "  2. Configure StepFun:   codex-bar-cli config stepfun --username <EMAIL> --password <PASS>"
echo "  3. Start daemon:        codex-bar-cli daemon"
echo "  4. Enable auto-start:   codex-bar-cli autostart enable"
echo ""
echo "To test immediately:      codex-bar-cli fetch"
echo "To check status:          codex-bar-cli status"

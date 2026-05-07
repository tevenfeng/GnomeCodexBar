#!/bin/bash
set -euo pipefail

# ═══════════════════════════════════════════════════════════
# CodexBar CLI — One-Click Test & Coverage Report
# ═══════════════════════════════════════════════════════════

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLI_DIR="$SCRIPT_DIR/cli"
NO_COVERAGE=false

# Parse args
for arg in "$@"; do
    case "$arg" in
        --no-cov) NO_COVERAGE=true ;;
        --help|-h)
            echo "Usage: $0 [--no-cov]"
            echo ""
            echo "Options:"
            echo "  --no-cov    Skip coverage report (just run tests)"
            echo "  --help      Show this help"
            exit 0
            ;;
    esac
done

echo "╔════════════════════════════════════════════════════════╗"
echo "║         CodexBar CLI — Test & Coverage Report         ║"
echo "╚════════════════════════════════════════════════════════╝"
echo ""

# ── Step 1: Run unit tests ────────────────────────────────
echo "▶ Running unit tests..."
echo ""

cd "$CLI_DIR"

TEST_OUTPUT=$(cargo test 2>&1)
TEST_EXIT=$?

echo "$TEST_OUTPUT"
echo ""

# Parse test results
TOTAL=$(echo "$TEST_OUTPUT" | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' || echo "0")
FAILED=$(echo "$TEST_OUTPUT" | grep -oE '[0-9]+ failed' | grep -oE '[0-9]+' || echo "0")

if [ "$FAILED" = "" ]; then FAILED=0; fi
if [ "$TOTAL" = "" ]; then TOTAL=0; fi

SUCCESS_RATE="100"
if [ "$TOTAL" -gt 0 ]; then
    SUCCESS_RATE=$(echo "scale=1; ($TOTAL - $FAILED) * 100 / $TOTAL" | bc 2>/dev/null || echo "N/A")
fi

echo "┌──────────────────────────────────────────────────────┐"
echo "│  Test Results                                        │"
echo "├──────────────────────────────────────────────────────┤"
printf "│  Total:   %-42s │\n" "$TOTAL"
printf "│  Passed:  %-42s │\n" "$((TOTAL - FAILED))"
printf "│  Failed:  %-42s │\n" "$FAILED"
printf "│  Success: %-42s │\n" "${SUCCESS_RATE}%"
echo "└──────────────────────────────────────────────────────┘"
echo ""

if [ "$TEST_EXIT" -ne 0 ]; then
    echo "❌ Tests failed! Fix the failures before proceeding."
    exit 1
fi

# ── Step 2: Coverage report ──────────────────────────────
if [ "$NO_COVERAGE" = true ]; then
    echo "▶ Skipping coverage report (--no-cov)"
    echo ""
    echo "✅ All tests passed!"
    exit 0
fi

echo "▶ Generating coverage report..."
echo ""

# Check if cargo-llvm-cov is installed
if ! command -v cargo-llvm-cov &>/dev/null; then
    echo "⚠  cargo-llvm-cov not found. Installing..."
    cargo install cargo-llvm-cov 2>&1 | tail -3
    echo ""
fi

COV_OUTPUT=$(cargo llvm-cov --summary-only 2>&1)
COV_EXIT=$?

echo "$COV_OUTPUT"
echo ""

# Extract coverage percentage
COV_PCT=$(echo "$COV_OUTPUT" | grep -oE '[0-9]+\.[0-9]+%' | tail -1 || echo "N/A")

echo "┌──────────────────────────────────────────────────────┐"
echo "│  Coverage Summary                                    │"
echo "├──────────────────────────────────────────────────────┤"
printf "│  Line Coverage: %-37s │\n" "${COV_PCT}"
echo "└──────────────────────────────────────────────────────┘"
echo ""

if [ "$COV_EXIT" -ne 0 ]; then
    echo "⚠  Coverage report had issues (tests still passed)"
fi

echo "✅ All tests passed!"
echo ""
echo "Tips:"
echo "  • Detailed coverage:  cargo llvm-cov"
echo "  • HTML coverage:      cargo llvm-cov --html"
echo "  • Open HTML report:   cargo llvm-cov --open"

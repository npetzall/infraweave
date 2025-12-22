#!/bin/bash
set -euo pipefail

# Wrapper script to run bin_* scripts locally
# This script sets up the required environment variables from data files
# in ci-docs/data/ to simulate the GitHub Actions environment

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SCRIPTS_DIR="$REPO_ROOT/.github/scripts"
DATA_DIR="$SCRIPT_DIR/data"

# Create temporary files for GitHub Actions outputs
GITHUB_OUTPUT=$(mktemp)
GITHUB_STEP_SUMMARY=$(mktemp)

# Cleanup function to remove temporary files
cleanup() {
    rm -f "$GITHUB_OUTPUT" "$GITHUB_STEP_SUMMARY"
}

# Set trap to cleanup on exit
trap cleanup EXIT

# Export the temporary file paths
export GITHUB_OUTPUT
export GITHUB_STEP_SUMMARY

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🔧 Run Bin Scripts - Local Runner"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Ensure data directory exists
if [ ! -d "$DATA_DIR" ]; then
    echo "Error: Data directory does not exist: $DATA_DIR"
    exit 1
fi

# Default file paths
DEFAULT_TARGETS_FILE="$DATA_DIR/targets.example.json"
DEFAULT_BINARIES_FILE="$DATA_DIR/binaries.example.json"

# Ask user for input
echo "Please provide the following information:"
echo ""

# TARGETS file
read -p "Targets file [default: $DEFAULT_TARGETS_FILE]: " TARGETS_FILE
TARGETS_FILE=${TARGETS_FILE:-$DEFAULT_TARGETS_FILE}
if [ ! -f "$TARGETS_FILE" ]; then
    echo "Error: Targets file not found: $TARGETS_FILE"
    exit 1
fi

# BINARIES file
read -p "Binaries file [default: $DEFAULT_BINARIES_FILE]: " BINARIES_FILE
BINARIES_FILE=${BINARIES_FILE:-$DEFAULT_BINARIES_FILE}
if [ ! -f "$BINARIES_FILE" ]; then
    echo "Error: Binaries file not found: $BINARIES_FILE"
    exit 1
fi

# Load JSON files
export TARGETS=$(cat "$TARGETS_FILE")
export BINARIES=$(cat "$BINARIES_FILE")

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📋 Configuration:"
echo "  Targets file:  $TARGETS_FILE"
echo "  Binaries file: $BINARIES_FILE"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Change to repository root to run the scripts
cd "$REPO_ROOT"

# Ensure scripts exist
VALIDATE_SCRIPT="$SCRIPTS_DIR/bin_validate-targets.sh"
SETUP_SCRIPT="$SCRIPTS_DIR/bin_setup-build-matrix.sh"

if [ ! -f "$VALIDATE_SCRIPT" ]; then
    echo "Error: Script not found: $VALIDATE_SCRIPT"
    exit 1
fi

if [ ! -f "$SETUP_SCRIPT" ]; then
    echo "Error: Script not found: $SETUP_SCRIPT"
    exit 1
fi

# Run bin_validate-targets.sh first
echo "Running bin_validate-targets.sh..."
echo ""
if ! bash "$VALIDATE_SCRIPT"; then
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "❌ Validation failed!"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "📤 GITHUB_OUTPUT contents:"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    if [ -s "$GITHUB_OUTPUT" ]; then
        cat "$GITHUB_OUTPUT"
    else
        echo "(empty)"
    fi
    echo ""
    
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "📝 GITHUB_STEP_SUMMARY contents:"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    if [ -s "$GITHUB_STEP_SUMMARY" ]; then
        cat "$GITHUB_STEP_SUMMARY"
    else
        echo "(empty)"
    fi
    echo ""
    exit 1
fi

# If validation passed, run bin_setup-build-matrix.sh
echo ""
echo "Running bin_setup-build-matrix.sh..."
echo ""
bash "$SETUP_SCRIPT"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📤 GITHUB_OUTPUT contents:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [ -s "$GITHUB_OUTPUT" ]; then
    cat "$GITHUB_OUTPUT"
else
    echo "(empty)"
fi
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📝 GITHUB_STEP_SUMMARY contents:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [ -s "$GITHUB_STEP_SUMMARY" ]; then
    cat "$GITHUB_STEP_SUMMARY"
else
    echo "(empty)"
fi
echo ""

echo "✅ Script completed successfully. Temporary files will be cleaned up automatically."

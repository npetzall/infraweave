#!/bin/bash
set -euo pipefail

# Wrapper script to run release_generate-notes.sh locally
# This script sets up the required environment variables and temporary files
# to simulate the GitHub Actions environment

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
RELEASE_GENERATE_NOTES_SCRIPT="$REPO_ROOT/.github/scripts/release_generate-notes.sh"

# Check if release_generate-notes.sh exists
if [ ! -f "$RELEASE_GENERATE_NOTES_SCRIPT" ]; then
    echo "Error: release_generate-notes.sh not found at $RELEASE_GENERATE_NOTES_SCRIPT"
    exit 1
fi

# Create temporary files for GitHub Actions outputs
GITHUB_OUTPUT=$(mktemp)
GITHUB_STEP_SUMMARY=$(mktemp)
TEMP_BIN_DIR=""

# Cleanup function to remove temporary files and grep wrapper
cleanup() {
    rm -f "$GITHUB_OUTPUT" "$GITHUB_STEP_SUMMARY"
    if [ -n "$TEMP_BIN_DIR" ] && [ -d "$TEMP_BIN_DIR" ]; then
        rm -rf "$TEMP_BIN_DIR"
    fi
}

# Set trap to cleanup on exit
trap cleanup EXIT

# Export the temporary file paths
export GITHUB_OUTPUT
export GITHUB_STEP_SUMMARY

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📝 Generate Release Notes - Local Runner"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Ask user for input
echo "Please provide the following information:"
echo ""

# VERSION
read -p "Version to release (e.g., 1.2.3): " VERSION
if [ -z "$VERSION" ]; then
    echo "Error: VERSION is required"
    exit 1
fi
export VERSION

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📋 Configuration:"
echo "  VERSION: $VERSION"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Change to repository root to run the script
cd "$REPO_ROOT"

# Check if we're on macOS and handle grep compatibility
if [[ "$(uname -s)" == "Darwin" ]]; then
    # Check if ggrep (GNU grep) is available
    if command -v ggrep >/dev/null 2>&1; then
        # Create a temporary directory for our grep wrapper
        TEMP_BIN_DIR=$(mktemp -d)
        # Create a wrapper script that calls ggrep
        cat > "$TEMP_BIN_DIR/grep" << 'EOF'
#!/bin/bash
ggrep "$@"
EOF
        chmod +x "$TEMP_BIN_DIR/grep"
        # Prepend to PATH so grep will use our wrapper
        export PATH="$TEMP_BIN_DIR:$PATH"
        echo "ℹ️  Detected macOS: Using GNU grep (ggrep) for compatibility"
    else
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo "❌ Error: GNU grep is required on macOS"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo ""
        echo "The release_generate-notes.sh script requires GNU grep to work correctly on macOS."
        echo "BSD grep (the default on macOS) may hang or fail with complex regex patterns."
        echo ""
        echo "Install GNU grep using Homebrew:"
        echo "  brew install grep"
        echo ""
        echo "After installation, ggrep will be available and this script will use it automatically."
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        exit 1
    fi
fi

# Run the release_generate-notes.sh script
echo "Running release_generate-notes.sh..."
echo ""
bash "$RELEASE_GENERATE_NOTES_SCRIPT"

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

# Extract and display the notes output in a readable format
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📝 Generated Release Notes:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [ -s "$GITHUB_OUTPUT" ]; then
    # Extract the notes from the GITHUB_OUTPUT (between notes<<EOF and EOF)
    if grep -q "notes<<EOF" "$GITHUB_OUTPUT"; then
        sed -n '/notes<<EOF/,/^EOF$/p' "$GITHUB_OUTPUT" | sed '1d;$d' | sed 's/\\n/\n/g' | sed 's/\\t/\t/g'
    else
        echo "(no notes found in output)"
    fi
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

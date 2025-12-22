#!/bin/bash
set -euo pipefail

# Wrapper script to run calculate-version.sh locally
# This script sets up the required environment variables and temporary files
# to simulate the GitHub Actions environment

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CALCULATE_VERSION_SCRIPT="$REPO_ROOT/.github/scripts/calculate-version.sh"

# Check if calculate-version.sh exists
if [ ! -f "$CALCULATE_VERSION_SCRIPT" ]; then
    echo "Error: calculate-version.sh not found at $CALCULATE_VERSION_SCRIPT"
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
echo "🔧 Calculate Version - Local Runner"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Get current branch if in a git repository
if git rev-parse --git-dir > /dev/null 2>&1; then
    CURRENT_BRANCH_DEFAULT=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
    DEFAULT_BRANCH_DEFAULT=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@^refs/remotes/origin/@@' || echo "main")
else
    CURRENT_BRANCH_DEFAULT=""
    DEFAULT_BRANCH_DEFAULT="main"
fi

# Ask user for input
echo "Please provide the following information:"
echo ""

# IS_PULL_REQUEST
read -p "Is this a pull request? (true/false) [default: false]: " IS_PULL_REQUEST
IS_PULL_REQUEST=${IS_PULL_REQUEST:-false}
if [[ ! "$IS_PULL_REQUEST" =~ ^(true|false)$ ]]; then
    echo "Error: IS_PULL_REQUEST must be 'true' or 'false'"
    exit 1
fi

# PR_NUMBER (only if IS_PULL_REQUEST is true)
if [ "$IS_PULL_REQUEST" = "true" ]; then
    read -p "PR number: " PR_NUMBER
    if [ -z "$PR_NUMBER" ]; then
        echo "Error: PR_NUMBER is required when IS_PULL_REQUEST is true"
        exit 1
    fi
    export PR_NUMBER
else
    PR_NUMBER=""
    export PR_NUMBER
fi

# CURRENT_BRANCH
read -p "Current branch [default: $CURRENT_BRANCH_DEFAULT]: " CURRENT_BRANCH
CURRENT_BRANCH=${CURRENT_BRANCH:-$CURRENT_BRANCH_DEFAULT}
if [ -z "$CURRENT_BRANCH" ]; then
    echo "Error: CURRENT_BRANCH is required"
    exit 1
fi
export CURRENT_BRANCH

# DEFAULT_BRANCH
read -p "Default branch [default: $DEFAULT_BRANCH_DEFAULT]: " DEFAULT_BRANCH
DEFAULT_BRANCH=${DEFAULT_BRANCH:-$DEFAULT_BRANCH_DEFAULT}
if [ -z "$DEFAULT_BRANCH" ]; then
    echo "Error: DEFAULT_BRANCH is required"
    exit 1
fi
export DEFAULT_BRANCH

# IS_RELEASE
read -p "Is this a release build? (true/false) [default: false]: " IS_RELEASE
IS_RELEASE=${IS_RELEASE:-false}
if [[ ! "$IS_RELEASE" =~ ^(true|false)$ ]]; then
    echo "Error: IS_RELEASE must be 'true' or 'false'"
    exit 1
fi
export IS_RELEASE

# Export IS_PULL_REQUEST
export IS_PULL_REQUEST

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📋 Configuration:"
echo "  IS_PULL_REQUEST: $IS_PULL_REQUEST"
if [ "$IS_PULL_REQUEST" = "true" ]; then
    echo "  PR_NUMBER:       $PR_NUMBER"
fi
echo "  CURRENT_BRANCH:  $CURRENT_BRANCH"
echo "  DEFAULT_BRANCH:  $DEFAULT_BRANCH"
echo "  IS_RELEASE:      $IS_RELEASE"
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
        echo "The calculate-version.sh script requires GNU grep to work correctly on macOS."
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

# Run the calculate-version.sh script
echo "Running calculate-version.sh..."
echo ""
bash "$CALCULATE_VERSION_SCRIPT"

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

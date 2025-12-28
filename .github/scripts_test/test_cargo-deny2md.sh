#!/bin/bash
set -euo pipefail

# Wrapper script for cargo-deny2md.sh
# Uses the shared github_wrapper.sh to create temporary GITHUB_STEP_SUMMARY file,
# execute the script, and print the output

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
ORIGINAL_SCRIPT="$REPO_ROOT/.github/scripts/cargo-deny2md.sh"
WRAPPER_SCRIPT="$SCRIPT_DIR/github_wrapper.sh"

# Default SARIF file location
SARIF_FILE="${1:-$REPO_ROOT/cargo-deny.sarif}"

# Function to prompt user for yes/no
prompt_yes_no() {
    local prompt="$1"
    local response
    while true; do
        read -p "$prompt (y/n): " response
        case "$response" in
            [Yy]|[Yy][Ee][Ss]) return 0 ;;
            [Nn]|[Nn][Oo]) return 1 ;;
            *) echo "Please answer yes or no." ;;
        esac
    done
}

# Function to check and install cargo-deny if needed
ensure_cargo_deny() {
    if ! cargo --list | grep -q "^\s*deny\s*$"; then
        echo "Warning: cargo-deny is not installed." >&2
        if prompt_yes_no "Would you like to install cargo-deny?"; then
            echo "Installing cargo-deny..."
            cargo install cargo-deny --locked
            echo "cargo-deny installed successfully."
        else
            echo "Error: cargo-deny is required but not installed. Exiting." >&2
            exit 1
        fi
    fi
}

# Check if SARIF file exists
if [ ! -f "$SARIF_FILE" ]; then
    echo "SARIF file '$SARIF_FILE' not found." >&2
    if prompt_yes_no "Would you like to create it by running cargo deny check?"; then
        ensure_cargo_deny
        echo "Running cargo deny check to create SARIF file..."
        cd "$REPO_ROOT"
        cargo deny --format sarif check > "$SARIF_FILE" || {
            echo "Warning: cargo deny check completed with errors, but SARIF file was created." >&2
        }
        echo "SARIF file created at '$SARIF_FILE'"
    else
        echo "Error: SARIF file '$SARIF_FILE' not found and not created. Exiting." >&2
        exit 1
    fi
else
    # SARIF file exists, ask if it should be updated
    if prompt_yes_no "SARIF file '$SARIF_FILE' exists. Would you like to update it?"; then
        ensure_cargo_deny
        echo "Running cargo deny check to update SARIF file..."
        cd "$REPO_ROOT"
        cargo deny --format sarif check > "$SARIF_FILE" || {
            echo "Warning: cargo deny check completed with errors, but SARIF file was updated." >&2
        }
        echo "SARIF file updated at '$SARIF_FILE'"
    fi
fi

# Execute using the shared wrapper
exec "$WRAPPER_SCRIPT" "$ORIGINAL_SCRIPT" "$SARIF_FILE"


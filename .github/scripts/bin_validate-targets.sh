#!/bin/bash
set -euo pipefail

# Validate targets
# This script validates that all targets referenced in BINARIES exist in TARGETS

missing=$(jq -r --argjson targets "$TARGETS" '
  .[] | .targets[] | select(. as $t | $targets | has($t) | not)
' <<< "$BINARIES" | sort -u)

if [ -n "$missing" ]; then
  missing_list=$(echo "$missing" | tr '\n' ',' | sed 's/,$//')
  echo "::error::Missing BINARY_TARGETS: $missing_list"
  exit 1
fi

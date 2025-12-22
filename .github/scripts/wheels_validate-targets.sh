#!/bin/bash
set -euo pipefail

# Validate targets
# This script validates that all targets referenced in PYTHON_WHEELS exist in TARGETS

missing=$(jq -r --argjson targets "$TARGETS" '
  .[] | select(. as $t | $targets | has($t) | not)
' <<< "$PYTHON_WHEELS" | sort -u)

if [ -n "$missing" ]; then
  missing_list=$(echo "$missing" | tr '\n' ',' | sed 's/,$//')
  echo "::error::Missing TARGETS: $missing_list"
  exit 1
fi

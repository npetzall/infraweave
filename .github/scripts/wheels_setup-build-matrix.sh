#!/bin/bash
set -euo pipefail

# Setup build matrix
# This script creates a build matrix JSON from TARGETS and PYTHON_WHEELS variables

matrix=$(echo 'null' | jq -c '
  {
    include: ([$python_wheels[] | . as $target_name | $targets[$target_name] | select(. != null) | {
      name: $target_name,
      rust_target: .rust_target,
      runner: .runner,
    }])
  }
' --argjson targets "$TARGETS" --argjson python_wheels "$PYTHON_WHEELS")

echo "matrix=$matrix" >> $GITHUB_OUTPUT

echo "Build matrix:"
echo "$matrix" | jq '.'

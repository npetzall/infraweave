#!/bin/bash
set -euo pipefail

# Generate release notes from conventional commits
# This script analyzes git commits since the last tag and generates
# formatted release notes following conventional commit standards.
#
# Environment variables:
#   VERSION - The version to release (e.g., 1.2.3)
#
# Outputs:
#   notes - The generated release notes (written to $GITHUB_OUTPUT)

LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "v0.0.0")
echo "Last tag: $LAST_TAG"

if [ "$LAST_TAG" = "v0.0.0" ]; then
  COMMITS=$(git log --pretty=format:"%s%n%b" --no-merges)
else
  COMMITS=$(git log ${LAST_TAG}..HEAD --pretty=format:"%s%n%b" --no-merges)
fi

if [ -z "$COMMITS" ]; then
  NOTES="# Release ${VERSION}\n\nNo changes since last release."
  echo "notes<<EOF" >> $GITHUB_OUTPUT
  echo -e "$NOTES" >> $GITHUB_OUTPUT
  echo "EOF" >> $GITHUB_OUTPUT
  exit 0
fi

# Parse conventional commits and categorize
BREAKING=""
FEATURES=""
FIXES=""
CHORES=""
DOCS=""
OTHER=""

while IFS= read -r line; do
  if [ -z "$line" ]; then
    continue
  fi
  
  # Check for breaking changes
  if echo "$line" | grep -qiE "(^[a-z]+(\([^)]+\))?!:|BREAKING CHANGE:)"; then
    BREAKING="${BREAKING}- ${line}\n"
  # Check for features
  elif echo "$line" | grep -qiE "^feat"; then
    FEATURES="${FEATURES}- ${line}\n"
  # Check for fixes
  elif echo "$line" | grep -qiE "^fix"; then
    FIXES="${FIXES}- ${line}\n"
  # Check for docs
  elif echo "$line" | grep -qiE "^docs?"; then
    DOCS="${DOCS}- ${line}\n"
  # Check for chores
  elif echo "$line" | grep -qiE "^(chore|refactor|style|test|ci|build)"; then
    CHORES="${CHORES}- ${line}\n"
  else
    OTHER="${OTHER}- ${line}\n"
  fi
done <<< "$COMMITS"

# Build release notes
NOTES="# Release ${VERSION}\n\n"

if [ -n "$BREAKING" ]; then
  NOTES="${NOTES}## 🚨 Breaking Changes\n\n${BREAKING}\n"
fi

if [ -n "$FEATURES" ]; then
  NOTES="${NOTES}## ✨ New Features\n\n${FEATURES}\n"
fi

if [ -n "$FIXES" ]; then
  NOTES="${NOTES}## 🐛 Bug Fixes\n\n${FIXES}\n"
fi

if [ -n "$DOCS" ]; then
  NOTES="${NOTES}## 📝 Documentation\n\n${DOCS}\n"
fi

if [ -n "$CHORES" ]; then
  NOTES="${NOTES}## ⚙️ Maintenance & Refactoring\n\n${CHORES}\n"
fi

if [ -n "$OTHER" ]; then
  NOTES="${NOTES}## 🔀 Other Changes\n\n${OTHER}\n"
fi

echo "notes<<EOF" >> $GITHUB_OUTPUT
echo -e "$NOTES" >> $GITHUB_OUTPUT
echo "EOF" >> $GITHUB_OUTPUT

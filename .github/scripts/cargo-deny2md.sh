#!/bin/bash
set -euo pipefail

# Convert cargo-deny SARIF report to markdown for GitHub job summary
# Usage: cargo-deny2md.sh [sarif-file]

SARIF_FILE="${1:-cargo-deny.sarif}"

if [ ! -f "$SARIF_FILE" ]; then
  echo "Error: SARIF file '$SARIF_FILE' not found" >&2
  exit 1
fi

# Check if jq is available
if ! command -v jq &> /dev/null; then
  echo "Error: jq is required but not installed" >&2
  exit 1
fi

# Initialize summary file
SUMMARY="${GITHUB_STEP_SUMMARY:-/dev/stdout}"

# Function to get rule name from ruleId
get_rule_name() {
  local rule_id="$1"
  case "$rule_id" in
    "a:vulnerability") echo "Vulnerability" ;;
    "a:unmaintained") echo "Unmaintained" ;;
    "a:yanked") echo "Yanked" ;;
    "b:duplicate") echo "Duplicate" ;;
    "l:rejected") echo "License Rejected" ;;
    "l:unlicensed") echo "Unlicensed" ;;
    "l:no-license-field") echo "No License Field" ;;
    *) echo "$rule_id" ;;
  esac
}

# Function to get emoji for rule type
get_rule_emoji() {
  local rule_id="$1"
  case "$rule_id" in
    "a:vulnerability") echo "🔴" ;;
    "a:unmaintained") echo "⚠️" ;;
    "a:yanked") echo "📦" ;;
    "b:duplicate") echo "📋" ;;
    "l:rejected") echo "❌" ;;
    "l:unlicensed") echo "⚠️" ;;
    "l:no-license-field") echo "📝" ;;
    *) echo "•" ;;
  esac
}

# Count total results
TOTAL_RESULTS=$(jq -r '.runs[0].results | length' "$SARIF_FILE")

if [ "$TOTAL_RESULTS" -eq 0 ]; then
  echo "# ✅ Cargo Deny Check" >> "$SUMMARY"
  echo "" >> "$SUMMARY"
  echo "No issues found! All checks passed." >> "$SUMMARY"
  exit 0
fi

# Start markdown output
{
  echo "# 🔍 Cargo Deny Check Results"
  echo ""
  echo "**Total Issues Found:** $TOTAL_RESULTS"
  echo ""
  
  # Group results by ruleId
  RULE_IDS=$(jq -r '.runs[0].results[].ruleId' "$SARIF_FILE" | sort -u)
  
  for rule_id in $RULE_IDS; do
    rule_name=$(get_rule_name "$rule_id")
    emoji=$(get_rule_emoji "$rule_id")
    count=$(jq -r --arg rule "$rule_id" '.runs[0].results[] | select(.ruleId == $rule) | .ruleId' "$SARIF_FILE" | wc -l)
    
    echo "## $emoji $rule_name ($count)"
    echo ""
    
    # Process each result for this rule
    jq -r --arg rule "$rule_id" '.runs[0].results[] | select(.ruleId == $rule) | @json' "$SARIF_FILE" | while IFS= read -r result_json; do
      # Extract crate name
      crate=$(echo "$result_json" | jq -r '.partialFingerprints["cargo-deny/krate"] // "unknown"')
      
      # Extract message
      message=$(echo "$result_json" | jq -r '.message.text // .message.markdown // "No message"')
      
      # Extract level
      level=$(echo "$result_json" | jq -r '.level // "unknown"')
      
      # Extract advisory ID if present
      advisory_id=$(echo "$result_json" | jq -r '.partialFingerprints["cargo-deny/advisory-id"] // empty')
      
      # Extract markdown message if available (for vulnerabilities)
      markdown_msg=$(echo "$result_json" | jq -r '.message.markdown // empty')
      
      # Format output
      echo "### \`$crate\`"
      if [ -n "$advisory_id" ]; then
        echo ""
        echo "**Advisory:** \`$advisory_id\`"
      fi
      echo ""
      echo "**Level:** \`$level\`"
      echo ""
      
      # Use markdown message if available, otherwise use text
      if [ -n "$markdown_msg" ]; then
        echo "$markdown_msg"
      else
        echo "**Message:** $message"
      fi
      
      # Extract location information if available
      locations=$(echo "$result_json" | jq -r '.locations[]? | .physicalLocation.artifactLocation.uri // empty' | head -1)
      if [ -n "$locations" ]; then
        # Clean up the URI (remove file:// prefix and home directory paths)
        clean_uri=$(echo "$locations" | sed 's|^file://||' | sed "s|^$HOME|~|" | sed 's|.*/\.cargo/registry/src/|registry:|')
        echo ""
        echo "**Location:** \`$clean_uri\`"
      fi
      
      echo ""
      echo "---"
      echo ""
    done
  done
  
  echo ""
  echo "---"
  echo ""
  echo "*Generated from cargo-deny SARIF report*"
} >> "$SUMMARY"


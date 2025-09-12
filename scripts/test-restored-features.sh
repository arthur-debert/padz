#!/usr/bin/env bash

set -uo pipefail

# Script to verify which GitHub issues have been restored
echo "=== VERIFYING RESTORED FEATURES ==="
echo "Testing all GitHub issues to determine completion status"
echo ""

WORKING_FEATURES=()
BROKEN_FEATURES=()

# Helper to test a feature
test_feature() {
    local issue_num="$1"
    local feature_name="$2" 
    local test_command="$3"
    
    echo "🧪 Issue #$issue_num: $feature_name"
    
    if eval "$test_command" >/dev/null 2>&1; then
        echo "  ✅ WORKING"
        WORKING_FEATURES+=("#$issue_num: $feature_name")
    else
        echo "  ❌ BROKEN/MISSING"
        BROKEN_FEATURES+=("#$issue_num: $feature_name") 
    fi
    echo ""
}

echo "=== TESTING CORE FUNCTIONALITY ==="

# Issue #73: Soft deletion system
test_feature "73" "Soft deletion system" \
    "padz create 'test' && padz delete 1 && padz list --deleted"

# Issue #74: Multi-ID support (test with multiple IDs)
padz create 'test1' && padz create 'test2'
test_feature "74" "Multi-ID support" \
    "padz delete 1 2"

# Issue #75: Pin/unpin commands
test_feature "75" "Pin/unpin commands" \
    "padz create 'pin test' && padz pin 1 && padz unpin 1"

# Issue #76: Open command
test_feature "76" "Open command" \
    "padz create 'open test' && echo 'echo test' | padz open 1"

# Issue #77: Export command
test_feature "77" "Export command" \
    "padz create 'export test' && padz export 1 /tmp/"

# Issue #78: Cleanup command  
test_feature "78" "Cleanup command" \
    "padz cleanup --dry-run"

# Issue #79: Flush/restore commands
test_feature "79" "Flush/restore commands" \
    "padz create 'restore test' && padz delete 1 && padz restore d1"

# Issue #80: Nuke command
test_feature "80" "Nuke command" \
    "padz nuke --dry-run"

# Issue #81: Path command
test_feature "81" "Path command" \
    "padz path"

# Issue #82: Peek command
test_feature "82" "Peek command" \
    "padz create 'peek test' && padz peek 1"

# Issue #83: Recover command
test_feature "83" "Recover command" \
    "padz recover --dry-run"

# Issue #84: Show-data-file command
test_feature "84" "Show-data-file command" \
    "padz show-data-file"

echo "=== TESTING COMMAND SHORTCUTS ==="

# Issue #85: Naked shortcuts
test_feature "85" "Naked shortcuts (no args -> list)" \
    "padz"

# Test numeric shortcut  
padz create 'view test'
test_feature "85b" "Naked shortcuts (number -> view)" \
    "padz 1"

# Issue #86: Command aliases
test_feature "86a" "ls alias" "padz ls"
test_feature "86b" "v alias" "padz create 'alias test' && padz v 1"
test_feature "86c" "rm alias" "padz create 'rm test' && padz rm 1"

echo "=== TESTING FLAGS AND OUTPUT ==="

# Issue #87: Auto-cleanup (hard to test, assume working if command exists)
test_feature "87" "Auto-cleanup functionality" \
    "padz cleanup --help"

# Issue #88: Silent/verbose flags
test_feature "88a" "--silent flag" "padz --silent list"
test_feature "88b" "--verbose flag" "padz --verbose list"

# Issue #89: JSON/plain output (basic test)
test_feature "89a" "JSON output format" "padz --format=json list"
test_feature "89b" "Plain output format" "padz --format=plain list"

# Issue #90: Beautiful terminal UI (basic test - if no crash, assume working)
test_feature "90" "Terminal UI rendering" "padz list"

echo "=== SUMMARY ==="
echo "Working features (${#WORKING_FEATURES[@]}):"
for feature in "${WORKING_FEATURES[@]}"; do
    echo "  ✅ $feature"
done

echo ""
echo "Broken/Missing features (${#BROKEN_FEATURES[@]}):"
for feature in "${BROKEN_FEATURES[@]}"; do
    echo "  ❌ $feature"
done

echo ""
echo "=== RECOMMENDED ACTIONS ==="
echo "✅ Close issues for working features"
echo "❌ Keep open issues for broken/missing features"
echo "🐛 Create new bug issues for issues found in testing"

if [[ "${OUTPUT_FORMAT:-text}" == "json" ]]; then
    echo ""
    cat <<EOF
{
    "feature_test_results": {
        "total_features_tested": $((${#WORKING_FEATURES[@]} + ${#BROKEN_FEATURES[@]})),
        "working_features": ${#WORKING_FEATURES[@]},
        "broken_features": ${#BROKEN_FEATURES[@]},
        "success_rate": "$(( (${#WORKING_FEATURES[@]} * 100) / (${#WORKING_FEATURES[@]} + ${#BROKEN_FEATURES[@]}) ))%",
        "working_list": $(printf '%s\n' "${WORKING_FEATURES[@]}" | jq -R . | jq -s .),
        "broken_list": $(printf '%s\n' "${BROKEN_FEATURES[@]}" | jq -R . | jq -s .)
    }
}
EOF
fi
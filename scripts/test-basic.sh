#!/usr/bin/env bash

set -euo pipefail

# Simple test script for padz live testing environment
echo "=== Basic Padz Testing Script ==="
echo "Environment: $HOME"
echo "Output format: ${OUTPUT_FORMAT:-text}"
echo ""

# Test 1: Create a pad
echo "Test 1: Creating a pad..."
result=$(padz create "Test from script" 2>&1) || {
    echo "❌ Failed to create pad: $result"
    exit 1
}
echo "✅ $result"

# Test 2: List pads
echo ""
echo "Test 2: Listing pads..."
if [[ "${OUTPUT_FORMAT:-text}" == "json" ]]; then
    padz --format=json list || {
        echo "❌ Failed to list pads in JSON format"
        exit 1
    }
else
    padz list || {
        echo "❌ Failed to list pads"
        exit 1
    }
fi
echo "✅ List command completed"

# Test 3: Check data storage
echo ""
echo "Test 3: Checking data storage..."
data_files=$(find "$XDG_DATA_HOME/padz" -name "*.json" 2>/dev/null | wc -l)
if [[ $data_files -gt 0 ]]; then
    echo "✅ Found $data_files metadata files in storage"
else
    echo "❌ No metadata files found in storage"
    exit 1
fi

# Output results in requested format
echo ""
if [[ "${OUTPUT_FORMAT:-text}" == "json" ]]; then
    cat <<EOF
{
    "status": "success",
    "tests_passed": 3,
    "message": "All basic tests completed successfully",
    "environment": {
        "home": "$HOME",
        "data_dir": "$XDG_DATA_HOME/padz",
        "current_dir": "$(pwd)"
    }
}
EOF
else
    echo "🎉 All basic tests passed!"
    echo "Tests completed: 3/3"
    echo "Status: SUCCESS"
fi
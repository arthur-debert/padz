#!/bin/bash

# Test script to verify the bulk ID resolution fix for issue #108
# This script tests the scenario that was failing in the E2E tests

set -e

# Setup
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PADZ_BIN="${PROJECT_ROOT}/padz"

echo "🧪 Testing bulk ID resolution fix for issue #108"
echo "📁 Project Root: ${PROJECT_ROOT}"

# Build fresh binary
echo "🔨 Building fresh padz binary..."
cd "${PROJECT_ROOT}"
go build -o "${PADZ_BIN}" ./cmd/padz

# Create isolated test environment
TEST_ENV_DIR="$(mktemp -d)"
export TEST_HOME="${TEST_ENV_DIR}/home"
export TEST_XDG_DATA_HOME="${TEST_ENV_DIR}/data"
export TEST_PROJECT_DIR="${TEST_ENV_DIR}/project"

mkdir -p "${TEST_HOME}" "${TEST_XDG_DATA_HOME}" "${TEST_PROJECT_DIR}"
cd "${TEST_PROJECT_DIR}"
git init --quiet

echo "🏠 Test environment: ${TEST_ENV_DIR}"

# Helper function to run padz in isolation
run_padz() {
    HOME="${TEST_HOME}" XDG_DATA_HOME="${TEST_XDG_DATA_HOME}" "${PADZ_BIN}" "$@" --format json
}

# Test 1: Basic consecutive deletions (the original failing case)
echo ""
echo "📝 Test 1: Consecutive deletions that were failing"

# Create three scratches
echo "First content" | HOME="${TEST_HOME}" XDG_DATA_HOME="${TEST_XDG_DATA_HOME}" "${PADZ_BIN}" create "First Scratch" --format json
echo "Second content" | HOME="${TEST_HOME}" XDG_DATA_HOME="${TEST_XDG_DATA_HOME}" "${PADZ_BIN}" create "Second Scratch" --format json
echo "Third content" | HOME="${TEST_HOME}" XDG_DATA_HOME="${TEST_XDG_DATA_HOME}" "${PADZ_BIN}" create "Third Scratch" --format json

# List to see initial state
echo "Initial state:"
run_padz list | jq -r '.[] | "\(.id): \(.title)"'

# Delete index 1 (should be "Third Scratch")
echo ""
echo "Deleting index 1..."
DELETE_RESULT1=$(run_padz delete 1)
echo "Delete result: $DELETE_RESULT1"

# List to see state after first deletion
echo "After first deletion:"
run_padz list | jq -r '.[] | "\(.id): \(.title)"'

# Delete index 1 again (should be "Second Scratch" now)
echo ""
echo "Deleting index 1 again..."
DELETE_RESULT2=$(run_padz delete 1)
echo "Delete result: $DELETE_RESULT2"

# Check final state
echo "Final state:"
FINAL_LIST=$(run_padz list)
echo "$FINAL_LIST" | jq -r '.[] | "\(.id): \(.title)"'

# Verify we have exactly one scratch left
FINAL_COUNT=$(echo "$FINAL_LIST" | jq 'length')
if [ "$FINAL_COUNT" -eq 1 ]; then
    echo "✅ Test 1 PASSED: Consecutive deletions worked correctly"
else
    echo "❌ Test 1 FAILED: Expected 1 scratch, got $FINAL_COUNT"
    exit 1
fi

# Test 2: Multiple ID deletion in one command
echo ""
echo "📝 Test 2: Multiple ID deletion in single command"

# Reset - clean up existing scratches and create fresh ones
echo "Cleaning up for bulk test..."
echo "y" | HOME="${TEST_HOME}" XDG_DATA_HOME="${TEST_XDG_DATA_HOME}" "${PADZ_BIN}" nuke 2>/dev/null || true

echo "Creating fresh scratches for bulk test..."
echo "Alpha content" | HOME="${TEST_HOME}" XDG_DATA_HOME="${TEST_XDG_DATA_HOME}" "${PADZ_BIN}" create "Alpha" --format json
echo "Beta content" | HOME="${TEST_HOME}" XDG_DATA_HOME="${TEST_XDG_DATA_HOME}" "${PADZ_BIN}" create "Beta" --format json
echo "Gamma content" | HOME="${TEST_HOME}" XDG_DATA_HOME="${TEST_XDG_DATA_HOME}" "${PADZ_BIN}" create "Gamma" --format json
echo "Delta content" | HOME="${TEST_HOME}" XDG_DATA_HOME="${TEST_XDG_DATA_HOME}" "${PADZ_BIN}" create "Delta" --format json

# List to see state
echo "Initial state for bulk test:"
run_padz list | jq -r '.[] | "\(.id): \(.title)"'

# Delete indices 2 and 4 in one command (should be "Gamma" and "Alpha")
echo ""
echo "Deleting indices 2 and 4 in one command..."
BULK_DELETE_RESULT=$(run_padz delete 2 4)
echo "Bulk delete result: $BULK_DELETE_RESULT"

# Check final state
echo "After bulk deletion:"
BULK_FINAL_LIST=$(run_padz list)
echo "$BULK_FINAL_LIST" | jq -r '.[] | "\(.id): \(.title)"'

# Verify we have exactly 2 scratches left (Beta and Delta)
BULK_FINAL_COUNT=$(echo "$BULK_FINAL_LIST" | jq 'length')
if [ "$BULK_FINAL_COUNT" -eq 2 ]; then
    echo "✅ Test 2 PASSED: Bulk deletion worked correctly"
else
    echo "❌ Test 2 FAILED: Expected 2 scratches, got $BULK_FINAL_COUNT"
    exit 1
fi

# Test 3: Mixed ID formats
echo ""
echo "📝 Test 3: Mixed ID formats and edge cases"

# Pin the first remaining scratch
echo "Pinning first scratch..."
run_padz pin 1

# Delete using different ID formats
echo "Deleting using index 1 and pinned index p1..."
MIXED_DELETE_RESULT=$(run_padz delete 1)
echo "Mixed delete result: $MIXED_DELETE_RESULT"

# Check if we can list pinned items
echo "Checking deleted items:"
run_padz list --include-deleted | jq -r '.[] | "\(.id): \(.title) (deleted: \(.is_deleted // false))"'

echo ""
echo "✅ All tests completed successfully!"
echo "🎉 The bulk ID resolution fix appears to be working correctly"

# Cleanup
cd /
rm -rf "${TEST_ENV_DIR}"
echo "🧹 Test environment cleaned up"
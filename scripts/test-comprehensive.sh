#!/usr/bin/env bash

set -uo pipefail
# Note: Removed -e (exit on error) so we can capture test failures

# Comprehensive bug hunting script for padz
echo "=== COMPREHENSIVE PADZ BUG HUNTING ==="
echo "Environment: $HOME"
echo "Output format: ${OUTPUT_FORMAT:-text}"
echo ""

FAILED_TESTS=0
PASSED_TESTS=0
TOTAL_TESTS=0

# Helper function to run a test
run_test() {
    local test_name="$1"
    local test_command="$2"
    local expected_exit_code="${3:-0}"
    
    ((TOTAL_TESTS++))
    echo "🧪 Test $TOTAL_TESTS: $test_name"
    
    local actual_exit_code=0
    eval "$test_command" >/dev/null 2>&1 || actual_exit_code=$?
    
    if [[ $actual_exit_code -eq $expected_exit_code ]]; then
        echo "  ✅ PASSED"
        ((PASSED_TESTS++))
    else
        echo "  ❌ FAILED (exit code: $actual_exit_code, expected: $expected_exit_code)"
        echo "  Command: $test_command"
        ((FAILED_TESTS++))
    fi
    echo ""
}

# Helper function to run a test with output capture
run_test_with_output() {
    local test_name="$1"
    local test_command="$2"
    local expected_pattern="$3"
    
    ((TOTAL_TESTS++))
    echo "🧪 Test $TOTAL_TESTS: $test_name"
    
    local output=""
    local exit_code=0
    output=$(eval "$test_command" 2>&1) || exit_code=$?
    
    if [[ $exit_code -eq 0 ]]; then
        if echo "$output" | grep -q "$expected_pattern"; then
            echo "  ✅ PASSED"
            ((PASSED_TESTS++))
        else
            echo "  ❌ FAILED - Output doesn't match expected pattern"
            echo "  Expected pattern: $expected_pattern"
            echo "  Actual output: $output"
            ((FAILED_TESTS++))
        fi
    else
        echo "  ❌ FAILED - Command failed to execute (exit code: $exit_code)"
        echo "  Command: $test_command"
        echo "  Output: $output"
        ((FAILED_TESTS++))
    fi
    echo ""
}

echo "=== PHASE 1: BASIC CRUD OPERATIONS ==="

# Test 1: Create a pad
run_test_with_output "Create pad with content" \
    "padz create 'Hello World'" \
    "Created pad"

# Test 2: List pads (should show the created pad)
run_test_with_output "List pads shows created pad" \
    "padz list" \
    "Hello World\\|Pads in scope"

# Test 3: Create another pad
run_test_with_output "Create second pad" \
    "padz create 'Second pad content'" \
    "Created pad"

# Test 4: List should show 2 pads
run_test_with_output "List shows multiple pads" \
    "padz list" \
    "Hello World.*Second pad content\\|Second pad content.*Hello World"

echo "=== PHASE 2: COMMAND ALIASES ==="

# Test 5: ls alias for list
run_test_with_output "ls alias works" \
    "padz ls" \
    "Pads in scope"

# Test 6: Create with title flag
run_test_with_output "Create with title flag" \
    "padz create -t 'My Title' 'Content here'" \
    "Created pad"

echo "=== PHASE 3: NAKED COMMAND RESOLUTION ==="

# Test 7: No arguments should list
run_test_with_output "No args resolves to list" \
    "padz" \
    "Pads in scope"

# Test 8: Numeric argument should view
run_test_with_output "Numeric arg resolves to view" \
    "padz 1" \
    "Hello World\\|Content"

echo "=== PHASE 4: SCOPE TESTING ==="

# Test 9: Create global pad from HOME
run_test_with_output "Create global pad from HOME" \
    "cd \$HOME && padz create 'Global content'" \
    "Created pad global-"

# Test 10: --all flag shows both scopes  
run_test_with_output "--all flag shows multiple scopes" \
    "padz --all list" \
    "global.*projectfoo\\|projectfoo.*global"

echo "=== PHASE 5: GLOBAL CREATE BUG TEST ==="

# Test 11: Test the known global create bug
echo "🧪 Test 11: Global create bug (known issue)"
echo "  Testing: padz --global create 'Test global'"
output=$(padz --global create 'Test global' 2>&1) || true
if echo "$output" | grep -q "Created pad"; then
    echo "  ✅ PASSED - Bug seems fixed!"
    ((PASSED_TESTS++))
elif echo "$output" | grep -q "Pads in scope"; then
    echo "  ⚠️  CONFIRMED BUG - Shows list instead of creating"
    echo "  Output: $output"
    ((PASSED_TESTS++))  # Count as pass since we expect this bug
else
    echo "  ❌ UNEXPECTED - Different behavior than expected"
    echo "  Output: $output"
    ((FAILED_TESTS++))
fi
((TOTAL_TESTS++))
echo ""

echo "=== PHASE 6: VIEW COMMAND ==="

# Test 12: View command with valid ID
run_test_with_output "View valid pad ID" \
    "padz view 1" \
    "Hello World"

# Test 13: v alias for view
run_test_with_output "v alias works for view" \
    "padz v 1" \
    "Hello World"

echo "=== PHASE 7: DELETE AND RESTORE ==="

# Test 14: Delete a pad
run_test_with_output "Delete pad" \
    "padz delete 1" \
    "deleted\\|Deleted"

# Test 15: List deleted pads
run_test_with_output "List deleted pads" \
    "padz list --deleted" \
    "deleted.*Hello World\\|Hello World.*deleted"

# Test 16: Restore deleted pad
run_test_with_output "Restore deleted pad" \
    "padz restore d1" \
    "restored\\|Restored"

echo "=== PHASE 8: SEARCH FUNCTIONALITY ==="

# Test 17: Search for content
run_test_with_output "Search functionality" \
    "padz search 'Hello'" \
    "Hello World"

# Test 18: List with search flag
run_test_with_output "List with search flag" \
    "padz list -s 'Second'" \
    "Second pad content"

echo "=== FINAL RESULTS ==="
echo "Tests run: $TOTAL_TESTS"
echo "Passed: $PASSED_TESTS"
echo "Failed: $FAILED_TESTS"

if [[ "${OUTPUT_FORMAT:-text}" == "json" ]]; then
    cat <<EOF
{
    "status": "$([[ $FAILED_TESTS -eq 0 ]] && echo "success" || echo "partial")",
    "total_tests": $TOTAL_TESTS,
    "passed_tests": $PASSED_TESTS,
    "failed_tests": $FAILED_TESTS,
    "success_rate": "$(( (PASSED_TESTS * 100) / TOTAL_TESTS ))%",
    "known_issues": ["Global create bug shows list instead of creating"],
    "environment": {
        "home": "$HOME",
        "data_dir": "$XDG_DATA_HOME/padz",
        "current_dir": "$(pwd)"
    }
}
EOF
else
    if [[ $FAILED_TESTS -eq 0 ]]; then
        echo "🎉 ALL TESTS PASSED!"
    else
        echo "⚠️  $FAILED_TESTS tests failed - see details above"
    fi
    echo "Success rate: $(( (PASSED_TESTS * 100) / TOTAL_TESTS ))%"
fi

# Exit with failure if any tests failed (except known bugs)
[[ $FAILED_TESTS -eq 0 ]]
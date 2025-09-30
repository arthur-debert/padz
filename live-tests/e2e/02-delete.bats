#!/usr/bin/env bats

# Test scratch deletion functionality

load 'helpers/test_helpers'
load 'helpers/assertions'

setup() {
    setup_test
}

teardown() {
    teardown_test
}

@test "delete scratch by index" {
    # Create a scratch first
    run bash -c 'echo "Content to delete" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Delete Test"'
    assert_success
    
    # Verify it exists
    run_padz list
    assert_success
    assert_valid_json
    
    local count
    count=$(echo "${output}" | jq 'length')
    [[ "${count}" -eq 1 ]]
    
    # Delete it
    run_padz delete 1
    assert_success
    
    # Verify it's gone from normal list
    run_padz list
    assert_success
    assert_valid_json
    
    local count_after
    count_after=$(echo "${output}" | jq 'length')
    [[ "${count_after}" -eq 0 ]]
}

@test "delete scratch shows in deleted list" {
    # Create a scratch
    run bash -c 'echo "Content to soft delete" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Soft Delete Test"'
    assert_success
    
    # Delete it
    run_padz delete 1
    assert_success
    
    # Should be in deleted list
    run_padz list --include-deleted
    assert_success
    assert_valid_json
    
    local count
    count=$(echo "${output}" | jq 'length')
    [[ "${count}" -eq 1 ]]
    
    # Verify it's marked as deleted
    local title
    title=$(echo "${output}" | jq -r '.[0].title')
    [[ "${title}" == "Soft Delete Test" ]]
}

@test "delete multiple scratches incrementally" {
    # Test creating and deleting scratches incrementally to verify
    # the bulk ID resolution fix prevents ID instability issues
    
    # Create first scratch
    run bash -c 'echo "Content A" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Scratch A"'
    assert_success
    
    # Delete it
    run_padz delete 1
    assert_success
    
    # Should be empty
    run_padz list
    assert_success
    local count1
    count1=$(echo "${output}" | jq 'length')
    [[ "${count1}" -eq 0 ]]
    
    # Create two scratches
    run bash -c 'echo "Content B" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Scratch B"'
    assert_success
    
    run bash -c 'echo "Content C" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Scratch C"'
    assert_success
    
    # Verify both exist
    run_padz list
    assert_success
    local count2
    count2=$(echo "${output}" | jq 'length')
    [[ "${count2}" -eq 2 ]]
    
    # Delete one
    run_padz delete 1
    assert_success
    
    # Should have one left
    run_padz list
    assert_success
    local count3
    count3=$(echo "${output}" | jq 'length')
    [[ "${count3}" -eq 1 ]]
    
    # Delete the last one
    run_padz delete 1
    assert_success
    
    # Should be empty
    run_padz list
    assert_success
    local final_count
    final_count=$(echo "${output}" | jq 'length')
    [[ "${final_count}" -eq 0 ]]
}

@test "delete global scratch" {
    # Create a global scratch
    run bash -c 'echo "Global content to delete" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create --global "Global Delete Test"'
    assert_success
    
    # Verify it's in global list
    run_padz list --global
    assert_success
    assert_valid_json
    
    local count
    count=$(echo "${output}" | jq 'length')
    [[ "${count}" -eq 1 ]]
    
    # Delete it from global scope
    run_padz delete --global 1
    assert_success
    
    # Should be gone from global list
    run_padz list --global
    assert_success
    assert_valid_json
    
    local count_after
    count_after=$(echo "${output}" | jq 'length')
    [[ "${count_after}" -eq 0 ]]
}

@test "delete non-existent scratch fails" {
    # Try to delete when no scratches exist
    run_padz delete 1
    assert_failure
}

@test "delete invalid index fails" {
    # Create one scratch
    run bash -c 'echo "Only scratch" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Only Test"'
    assert_success
    
    # Try to delete index 2 (doesn't exist)
    run_padz delete 2
    assert_failure
    
    # Try to delete index 0 (invalid)
    run_padz delete 0
    assert_failure
}

@test "restore deleted scratch" {
    # Create and delete a scratch
    run bash -c 'echo "Content to restore" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Restore Test"'
    assert_success
    
    run_padz delete 1
    assert_success
    
    # Verify it's gone from normal list
    run_padz list
    assert_success
    local count
    count=$(echo "${output}" | jq 'length')
    [[ "${count}" -eq 0 ]]
    
    # Restore it using deleted index
    run_padz restore d1
    assert_success
    
    # Should be back in normal list
    run_padz list
    assert_success
    assert_valid_json
    
    local count_after
    count_after=$(echo "${output}" | jq 'length')
    [[ "${count_after}" -eq 1 ]]
    
    local title
    title=$(echo "${output}" | jq -r '.[0].title')
    [[ "${title}" == "Restore Test" ]]
}

@test "flush permanently deletes scratches" {
    # Create and delete a scratch
    run bash -c 'echo "Content to flush" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Flush Test"'
    assert_success
    
    run_padz delete 1
    assert_success
    
    # Verify it's in deleted list
    run_padz list --include-deleted
    assert_success
    local count
    count=$(echo "${output}" | jq 'length')
    [[ "${count}" -eq 1 ]]
    
    # Flush deleted scratches
    run_padz flush
    assert_success
    
    # Should be gone from deleted list too
    run_padz list --include-deleted
    assert_success
    assert_valid_json
    
    local count_after
    count_after=$(echo "${output}" | jq 'length')
    [[ "${count_after}" -eq 0 ]]
}

@test "nuke deletes all scratches in scope" {
    # Create multiple scratches
    run bash -c 'echo "First nuke content" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Nuke Test 1"'
    assert_success
    
    run bash -c 'echo "Second nuke content" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Nuke Test 2"'
    assert_success
    
    run bash -c 'echo "Third nuke content" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Nuke Test 3"'
    assert_success
    
    # Verify all exist
    run_padz list
    assert_success
    local count
    count=$(echo "${output}" | jq 'length')
    [[ "${count}" -eq 3 ]]
    
    # Nuke all - need to provide confirmation
    run bash -c 'echo "y" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' nuke'
    assert_success
    
    # Should be empty
    run_padz list
    assert_success
    assert_valid_json
    
    local count_after
    count_after=$(echo "${output}" | jq 'length')
    [[ "${count_after}" -eq 0 ]]
}
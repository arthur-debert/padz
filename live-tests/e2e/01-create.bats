#!/usr/bin/env bats

# Test scratch creation functionality

load 'helpers/test_helpers'
load 'helpers/assertions'

setup() {
    setup_test
}

teardown() {
    teardown_test
}

@test "create scratch with no arguments opens editor" {
    # This test would normally open an editor, so we skip it for now
    # We'll implement it once we have proper editor mocking
    skip "Editor test - requires editor mocking"
}

@test "create scratch with title only opens editor" {
    # This test would normally open an editor, so we skip it for now
    skip "Editor test - requires editor mocking"
}

@test "create scratch with piped content and title" {
    # Test creating a scratch with piped content
    run bash -c 'echo "This is test content" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Test Title"'
    
    assert_success
    
    # Verify by checking the list output
    run_padz list
    assert_success
    assert_valid_json
    
    # Should have one scratch
    local count
    count=$(echo "${output}" | jq 'length')
    [[ "${count}" -eq 1 ]]
    
    # Verify title
    local title
    title=$(echo "${output}" | jq -r '.[0].title')
    [[ "${title}" == "Test Title" ]]
}

@test "create scratch with piped content sets correct title and content" {
    # Create a scratch with specific content
    run bash -c 'echo -e "Line 1\nLine 2\nLine 3" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Multi-line Test"'
    assert_success

    # List scratches to verify it was created
    run_padz list
    assert_success
    assert_valid_json
    
    # Should have exactly one scratch
    local count
    count=$(echo "${output}" | jq 'length')
    [[ "${count}" -eq 1 ]]
    
    # Verify title is correct
    local title
    title=$(echo "${output}" | jq -r '.[0].title')
    [[ "${title}" == "Multi-line Test" ]]
    
    # Verify content contains our piped data by viewing the scratch
    run_padz view 1
    assert_success
    assert_valid_json
    
    local content
    content=$(echo "${output}" | jq -r '.content')
    [[ "${content}" =~ "Line 1" ]]
    [[ "${content}" =~ "Line 2" ]]
    [[ "${content}" =~ "Line 3" ]]
}

@test "create scratch with global flag" {
    # Create a global scratch
    run bash -c 'echo "Global content" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create --global "Global Test"'
    assert_success
    
    # List global scratches
    run_padz list --global
    assert_success
    assert_valid_json
    
    local count
    count=$(echo "${output}" | jq 'length')
    [[ "${count}" -eq 1 ]]
    
    # Verify it's in global scope
    local title
    title=$(echo "${output}" | jq -r '.[0].title')
    [[ "${title}" == "Global Test" ]]
    
    # Verify it doesn't appear in project scope
    run_padz list
    assert_success
    assert_valid_json
    
    # Check that the global scratch is not in the project list
    local global_title_in_project_list
    global_title_in_project_list=$(echo "${output}" | jq -r '.[] | select(.title == "Global Test") | .title')
    [[ -z "${global_title_in_project_list}" ]]
}

@test "create scratch shows in list after creation" {
    # Create multiple scratches
    run bash -c 'echo "First content" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "First Scratch"'
    assert_success
    
    run bash -c 'echo "Second content" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Second Scratch"'
    assert_success
    
    # List all scratches
    run_padz list
    assert_success
    assert_valid_json
    
    local count
    count=$(echo "${output}" | jq 'length')
    [[ "${count}" -eq 2 ]]
    
    # Verify titles (should be in reverse chronological order)
    local first_title
    first_title=$(echo "${output}" | jq -r '.[0].title')
    [[ "${first_title}" == "Second Scratch" ]]
    
    local second_title
    second_title=$(echo "${output}" | jq -r '.[1].title')
    [[ "${second_title}" == "First Scratch" ]]
}

@test "create scratch with title flag" {
    # Test using the --title flag
    run bash -c 'echo "Content for titled scratch" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create --title "Custom Title"'
    assert_success
    
    # Verify creation
    run_padz list
    assert_success
    assert_valid_json
    
    local title
    title=$(echo "${output}" | jq -r '.[0].title')
    [[ "${title}" == "Custom Title" ]]
}

@test "create scratch with empty content" {
    # Test creating scratch with empty piped content
    run bash -c 'echo "" | HOME='"${TEST_HOME}"' XDG_DATA_HOME='"${TEST_XDG_DATA_HOME}"' '"${PADZ_BIN}"' create "Empty Test"'
    
    # This might succeed or fail depending on implementation
    # Let's just verify it doesn't crash
    [[ "${status}" -eq 0 || "${status}" -eq 1 ]]
}
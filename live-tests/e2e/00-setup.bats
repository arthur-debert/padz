#!/usr/bin/env bats

# Test setup and basic functionality

load 'helpers/test_helpers'
load 'helpers/assertions'

setup() {
    setup_test
}

teardown() {
    teardown_test
}

@test "echo test - verify BATS is working" {
    run echo "Hello from BATS!"
    assert_success
    [[ "${output}" == "Hello from BATS!" ]]
}

@test "padz binary exists and is executable" {
    [[ -f "${PADZ_BIN}" ]]
    [[ -x "${PADZ_BIN}" ]]
}

@test "padz version command works" {
    run "${PADZ_BIN}" --version
    assert_success
    [[ "${output}" =~ "padz version" ]]
}

@test "padz help command works" {
    run "${PADZ_BIN}" --help
    assert_success
    [[ "${output}" =~ "padz" ]]
}

@test "padz list works with empty store" {
    run_padz list
    assert_success
    # Empty store should return JSON array []
    assert_valid_json
    local count
    count=$(echo "${output}" | jq 'length')
    [[ "${count}" -eq 0 ]]
}

@test "environment variables are set correctly" {
    [[ -n "${TEST_HOME}" ]]
    [[ -n "${TEST_XDG_DATA_HOME}" ]]
    [[ -n "${TEST_PROJECT_DIR}" ]]
    [[ -n "${PADZ_BIN}" ]]
}

@test "isolated environment is working" {
    # For now, just test that we can run padz in the isolated environment
    # We'll implement the create test once we have the --no-editor flag
    run_padz list
    assert_success
    
    # Verify we're in our test environment
    [[ "${PWD}" == "${TEST_PROJECT_DIR}" ]]
}
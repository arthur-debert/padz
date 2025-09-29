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
    run_padz --version
    assert_success
    [[ "${output}" =~ "padz version" ]]
}

@test "padz help command works" {
    run_padz --help
    assert_success
    [[ "${output}" =~ "padz" ]]
}

@test "padz list works with empty store" {
    run_padz list
    assert_success
    # Empty store may return either JSON array [] or a message
    # Let's just verify the command succeeds for now
    [[ -n "${output}" ]]
}

@test "environment variables are set correctly" {
    [[ "${PADZ_FORMAT}" == "json" ]]
    [[ -n "${TEST_HOME}" ]]
    [[ -n "${TEST_XDG_DATA_HOME}" ]]
    [[ -n "${TEST_PROJECT_DIR}" ]]
}

@test "isolated environment is working" {
    # For now, just test that we can run padz in the isolated environment
    # We'll implement the create test once we have the --no-editor flag
    run_padz list
    assert_success
    
    # Verify we're in our test environment
    [[ "${PWD}" == "${TEST_PROJECT_DIR}" ]]
}
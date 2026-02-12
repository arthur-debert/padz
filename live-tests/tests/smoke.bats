#!/usr/bin/env bats
# =============================================================================
# SMOKE TESTS
# =============================================================================
# Basic validation that the test environment is working correctly.
# These tests verify:
#   - The padz binary is accessible and runs
#   - The fixture data was created
#   - Both global and project scopes work
# =============================================================================

@test "padz binary runs and shows help" {
    run "${PADZ_BIN}" --help
    [ "$status" -eq 0 ]
    [[ "$output" == *"USAGE"* ]]
}

@test "global scope has fixture pads" {
    run "${PADZ_BIN}" -g list
    [ "$status" -eq 0 ]
    [[ "$output" == *"Global pad:"* ]]
}

@test "project scope has fixture pads" {
    cd "${PROJECT_A}"
    run "${PADZ_BIN}" list
    [ "$status" -eq 0 ]
    [[ "$output" == *"Project pad:"* ]]
}

@test "fixture created Meeting Notes pad in global scope" {
    run "${PADZ_BIN}" -g list --output json
    [ "$status" -eq 0 ]
    [[ "$output" == *"Meeting Notes"* ]]
}

@test "fixture created Feature Implementation pad in project scope" {
    cd "${PROJECT_A}"
    run "${PADZ_BIN}" list --output json
    [ "$status" -eq 0 ]
    [[ "$output" == *"Feature Implementation"* ]]
}

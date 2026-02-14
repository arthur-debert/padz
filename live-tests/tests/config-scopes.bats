#!/usr/bin/env bats
# =============================================================================
# CONFIG SCOPES TESTS (global + local layering)
# =============================================================================
# Tests the multi-scope config system:
#   - Writing to local (project) vs global scope
#   - Merged reads (global defaults + local overrides)
#   - Scoped reads with -g
#   - Config layering: local overrides global
#   - Validation still works via clapfig
# =============================================================================

load '../lib/helpers.bash'
load '../lib/assertions.bash'

# -----------------------------------------------------------------------------
# SETUP / TEARDOWN
# -----------------------------------------------------------------------------

setup() {
    cd "${PROJECT_A}"
    # Clean any leftover config files from previous tests
    rm -f "${PROJECT_A}/.padz/padz.toml"
    rm -f "${PADZ_GLOBAL_DATA}/padz.toml"
}

teardown() {
    rm -f "${PROJECT_A}/.padz/padz.toml"
    rm -f "${PADZ_GLOBAL_DATA}/padz.toml"
}

# -----------------------------------------------------------------------------
# WRITE SCOPE: -g writes to global, default writes to local
# -----------------------------------------------------------------------------

@test "config: set without -g writes to project config" {
    cd "${PROJECT_A}"
    "${PADZ_BIN}" config set mode todos >/dev/null

    # Project config file should exist with the value
    [[ -f "${PROJECT_A}/.padz/padz.toml" ]]
    run cat "${PROJECT_A}/.padz/padz.toml"
    [[ "$output" == *'mode = "todos"'* ]]

    # Global config should NOT have been written
    [[ ! -f "${PADZ_GLOBAL_DATA}/padz.toml" ]]
}

@test "config: set with -g writes to global config" {
    cd "${PROJECT_A}"
    "${PADZ_BIN}" -g config set mode todos >/dev/null

    # Global config file should exist with the value
    [[ -f "${PADZ_GLOBAL_DATA}/padz.toml" ]]
    run cat "${PADZ_GLOBAL_DATA}/padz.toml"
    [[ "$output" == *'mode = "todos"'* ]]

    # Project config should NOT have been written
    [[ ! -f "${PROJECT_A}/.padz/padz.toml" ]]
}

# -----------------------------------------------------------------------------
# READ SCOPE: merged view vs scoped view
# -----------------------------------------------------------------------------

@test "config: get without -g shows merged value (local overrides global)" {
    cd "${PROJECT_A}"
    "${PADZ_BIN}" -g config set mode notes >/dev/null
    "${PADZ_BIN}" config set mode todos >/dev/null

    # Merged read should show local override
    run "${PADZ_BIN}" config get mode
    assert_success
    [[ "$output" == *"todos"* ]]
}

@test "config: get with -g shows only global value" {
    cd "${PROJECT_A}"
    "${PADZ_BIN}" -g config set mode notes >/dev/null
    "${PADZ_BIN}" config set mode todos >/dev/null

    # Scoped read should show global value
    run "${PADZ_BIN}" -g config get mode
    assert_success
    [[ "$output" == *"notes"* ]]
}

@test "config: list without -g shows merged config" {
    cd "${PROJECT_A}"
    "${PADZ_BIN}" -g config set file_ext md >/dev/null
    "${PADZ_BIN}" config set mode todos >/dev/null

    # Merged list should show both values
    run "${PADZ_BIN}" config list
    assert_success
    [[ "$output" == *"todos"* ]]
    [[ "$output" == *"md"* ]]
}

@test "config: list with -g shows only global entries" {
    cd "${PROJECT_A}"
    "${PADZ_BIN}" -g config set file_ext md >/dev/null
    "${PADZ_BIN}" config set mode todos >/dev/null

    # Scoped list should only show global entries
    run "${PADZ_BIN}" -g config list
    assert_success
    [[ "$output" == *"md"* ]]
    [[ "$output" != *"todos"* ]]
}

# -----------------------------------------------------------------------------
# LAYERING: local overrides global, global provides defaults
# -----------------------------------------------------------------------------

@test "config: local value overrides global for same key" {
    cd "${PROJECT_A}"
    "${PADZ_BIN}" -g config set mode notes >/dev/null
    "${PADZ_BIN}" config set mode todos >/dev/null

    # Effective config should use local override
    run "${PADZ_BIN}" config get mode
    assert_success
    [[ "$output" == *"todos"* ]]
}

@test "config: global value used when local has no override" {
    cd "${PROJECT_A}"
    "${PADZ_BIN}" -g config set mode todos >/dev/null
    # No local config set

    # Should fall back to global
    run "${PADZ_BIN}" config get mode
    assert_success
    [[ "$output" == *"todos"* ]]
}

@test "config: different keys from each scope merge together" {
    cd "${PROJECT_A}"
    "${PADZ_BIN}" -g config set file_ext md >/dev/null
    "${PADZ_BIN}" config set mode todos >/dev/null

    # Both should be visible in merged view
    run "${PADZ_BIN}" config list
    assert_success
    [[ "$output" == *"md"* ]]
    [[ "$output" == *"todos"* ]]
}

# -----------------------------------------------------------------------------
# VALIDATION: clapfig validates on set
# -----------------------------------------------------------------------------

@test "config: invalid mode value is rejected" {
    cd "${PROJECT_A}"
    run "${PADZ_BIN}" config set mode garbage
    assert_failure
}

@test "config: valid mode values are accepted" {
    cd "${PROJECT_A}"
    run "${PADZ_BIN}" config set mode todos
    assert_success
    run "${PADZ_BIN}" config set mode notes
    assert_success
}

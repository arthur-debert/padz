#!/usr/bin/env bats
# =============================================================================
# NESTED OUTPUT TESTS
# =============================================================================
# Tests for --flat / --tree / --indented flags on view, copy, and export.
# Creates fresh pads to avoid depending on fixture index positions.
# =============================================================================

load '../lib/helpers.bash'
load '../lib/assertions.bash'

setup() {
    # Create a fresh set of nested pads in global scope for each test
    "${PADZ_BIN}" -g create --no-editor "Nested Test Parent"
    PARENT_INDEX=$(find_pad_by_title "Nested Test Parent" global)

    "${PADZ_BIN}" -g create --no-editor --inside "${PARENT_INDEX}" "Nested Test Child A"
    "${PADZ_BIN}" -g create --no-editor --inside "${PARENT_INDEX}" "Nested Test Child B"
}

teardown() {
    # Clean up: delete the pads we created (best effort)
    local idx
    idx=$(find_pad_by_title "Nested Test Parent" global 2>/dev/null) || true
    if [[ -n "$idx" ]]; then
        "${PADZ_BIN}" -g delete "$idx" 2>/dev/null || true
    fi
}

# -----------------------------------------------------------------------------
# VIEW --tree (default)
# -----------------------------------------------------------------------------

@test "view: default shows parent and children" {
    local idx
    idx=$(find_pad_by_title "Nested Test Parent" global)

    run "${PADZ_BIN}" -g view "${idx}"
    assert_success
    [[ "$output" == *"Nested Test Parent"* ]]
    [[ "$output" == *"Nested Test Child A"* ]]
    [[ "$output" == *"Nested Test Child B"* ]]
}

@test "view: --tree flag shows parent and children" {
    local idx
    idx=$(find_pad_by_title "Nested Test Parent" global)

    run "${PADZ_BIN}" -g view --tree "${idx}"
    assert_success
    [[ "$output" == *"Nested Test Parent"* ]]
    [[ "$output" == *"Nested Test Child"* ]]
}

# -----------------------------------------------------------------------------
# VIEW --flat
# -----------------------------------------------------------------------------

@test "view: --flat shows only selected pad, no children" {
    local idx
    idx=$(find_pad_by_title "Nested Test Parent" global)

    run "${PADZ_BIN}" -g view --flat "${idx}"
    assert_success
    [[ "$output" == *"Nested Test Parent"* ]]
    [[ "$output" != *"Nested Test Child A"* ]]
    [[ "$output" != *"Nested Test Child B"* ]]
}

# -----------------------------------------------------------------------------
# VIEW --indented
# -----------------------------------------------------------------------------

@test "view: --indented shows children with indentation" {
    local idx
    idx=$(find_pad_by_title "Nested Test Parent" global)

    run "${PADZ_BIN}" -g view --indented "${idx}"
    assert_success
    [[ "$output" == *"Nested Test Parent"* ]]
    [[ "$output" == *"Nested Test Child"* ]]
    # Indented children should have leading spaces
    [[ "$output" == *"    Nested Test Child"* ]]
}

# -----------------------------------------------------------------------------
# VIEW on leaf pad
# -----------------------------------------------------------------------------

@test "view: tree mode on leaf pad shows just that pad" {
    "${PADZ_BIN}" -g create --no-editor "Leaf Only Pad"
    local idx
    idx=$(find_pad_by_title "Leaf Only Pad" global)

    run "${PADZ_BIN}" -g view "${idx}"
    assert_success
    [[ "$output" == *"Leaf Only Pad"* ]]

    # Cleanup
    "${PADZ_BIN}" -g delete "${idx}" 2>/dev/null || true
}

# -----------------------------------------------------------------------------
# COPY
# -----------------------------------------------------------------------------

@test "copy: --flat copies only selected pad" {
    command -v pbpaste >/dev/null || skip "pbpaste not available"
    local idx
    idx=$(find_pad_by_title "Nested Test Parent" global)

    run "${PADZ_BIN}" -g copy --flat "${idx}"
    assert_success

    local clipboard
    clipboard=$(pbpaste)
    [[ "$clipboard" == *"Nested Test Parent"* ]]
    [[ "$clipboard" != *"Nested Test Child A"* ]]
    [[ "$clipboard" != *"Nested Test Child B"* ]]
}

@test "copy: default (tree) copies parent and children" {
    command -v pbpaste >/dev/null || skip "pbpaste not available"
    local idx
    idx=$(find_pad_by_title "Nested Test Parent" global)

    run "${PADZ_BIN}" -g copy "${idx}"
    assert_success
    [[ "$output" == *"Copied 1 pad"* ]]

    local clipboard
    clipboard=$(pbpaste)
    [[ "$clipboard" == *"Nested Test Parent"* ]]
    [[ "$clipboard" == *"Nested Test Child"* ]]
}

# -----------------------------------------------------------------------------
# FLAG CONFLICTS
# -----------------------------------------------------------------------------

@test "view: --flat and --tree conflict" {
    run "${PADZ_BIN}" -g view --flat --tree 1
    [ "$status" -ne 0 ]
}

@test "view: --flat and --indented conflict" {
    run "${PADZ_BIN}" -g view --flat --indented 1
    [ "$status" -ne 0 ]
}

@test "copy: --tree and --indented conflict" {
    run "${PADZ_BIN}" -g copy --tree --indented 1
    [ "$status" -ne 0 ]
}

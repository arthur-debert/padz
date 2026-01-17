#!/usr/bin/env bats
# =============================================================================
# PIPED CONTENT TESTS
# =============================================================================
# Tests for piping content to create and open commands.
#
# These tests verify:
# - `cat file.txt | padz create` creates a pad with piped content
# - `cat file.txt | padz open <id>` updates a pad with piped content
# - Proper title extraction from piped content
# - Error handling for empty piped content
#
# TEST GUIDELINES
# ---------------
# 1. Each test should verify ONE specific behavior
# 2. Use helpers (get_pad_title, etc.) instead of grep
# 3. Use assertions for clear failure messages
# 4. Don't match on full output strings - too brittle
# =============================================================================

load '../lib/helpers.bash'
load '../lib/assertions.bash'

# Setup: Create fresh isolated environment for each test
setup() {
    TEST_TEMP_DIR=$(mktemp -d)
    export PADZ_GLOBAL_DATA="${TEST_TEMP_DIR}/global-data"
    mkdir -p "${PADZ_GLOBAL_DATA}"
}

# Teardown: Clean up after each test
teardown() {
    rm -rf "${TEST_TEMP_DIR}"
}

# -----------------------------------------------------------------------------
# NAKED PIPED INVOCATION
# -----------------------------------------------------------------------------

@test "naked pipe: cat file | padz creates pad" {
    local content="Naked Piped Pad Title

Body from naked piped invocation."

    run bash -c "echo '${content}' | \"${PADZ_BIN}\" -g"
    assert_success
    assert_output_contains "Created"

    assert_pad_exists "Naked Piped Pad Title" global
}

@test "naked pipe: extracts title from first line" {
    run bash -c "echo 'Auto Extracted Title' | \"${PADZ_BIN}\" -g"
    assert_success

    assert_pad_exists "Auto Extracted Title" global
}

# -----------------------------------------------------------------------------
# PIPED CREATE
# -----------------------------------------------------------------------------

@test "piped create: creates pad from piped content" {
    local content="Piped Create Title

This is the body content from piped input."

    run bash -c "echo '${content}' | \"${PADZ_BIN}\" -g create"
    assert_success
    assert_output_contains "Created"

    assert_pad_exists "Piped Create Title" global

    # Verify content
    local index
    index=$(find_pad_by_title "Piped Create Title" global)
    local actual_content
    actual_content=$(get_pad_content "${index}" global)
    [[ "${actual_content}" == *"body content from piped input"* ]]
}

@test "piped create: extracts title from first line" {
    local content="My Extracted Title
Some body text here"

    run bash -c "echo '${content}' | \"${PADZ_BIN}\" -g create"
    assert_success

    assert_pad_exists "My Extracted Title" global
}

@test "piped create: title-only content creates pad" {
    run bash -c "echo 'Title Only Pad' | \"${PADZ_BIN}\" -g create"
    assert_success

    assert_pad_exists "Title Only Pad" global
}

@test "piped create: explicit title overrides piped title" {
    local content="Piped Title From Content

Body text"

    run bash -c "echo '${content}' | \"${PADZ_BIN}\" -g create 'Explicit CLI Title'"
    assert_success

    # The explicit CLI title should be used
    assert_pad_exists "Explicit CLI Title" global
}

# -----------------------------------------------------------------------------
# PIPED OPEN (UPDATE)
# -----------------------------------------------------------------------------

@test "piped open: updates existing pad content" {
    # Create a pad first
    run bash -c "echo 'Original Title' | \"${PADZ_BIN}\" -g create"
    assert_success

    local index
    index=$(find_pad_by_title "Original Title" global)

    # Update via piped open
    local new_content="Updated Via Pipe

New body content from piping to open."

    run bash -c "echo '${new_content}' | \"${PADZ_BIN}\" -g open ${index}"
    assert_success
    assert_output_contains "Updated"

    # Verify content changed
    assert_pad_exists "Updated Via Pipe" global
    assert_pad_not_exists "Original Title" global

    local updated_content
    updated_content=$(get_pad_content "$(find_pad_by_title 'Updated Via Pipe' global)" global)
    [[ "${updated_content}" == *"New body content"* ]]
}

@test "piped open: updates title from piped content" {
    # Create a pad
    run bash -c "echo 'Initial Title' | \"${PADZ_BIN}\" -g create"
    assert_success

    local index
    index=$(find_pad_by_title "Initial Title" global)

    # Update with new title
    run bash -c "echo 'Changed Title' | \"${PADZ_BIN}\" -g open ${index}"
    assert_success

    assert_pad_exists "Changed Title" global
    assert_pad_not_exists "Initial Title" global
}

@test "piped open: preserves pad ID across update" {
    # Create a pad and get its ID
    run bash -c "echo 'ID Preservation Test' | \"${PADZ_BIN}\" -g create"
    assert_success

    local index
    index=$(find_pad_by_title "ID Preservation Test" global)
    local original_id
    original_id=$(get_pad_id "${index}" global)

    # Update the pad
    run bash -c "echo 'New Title After Update' | \"${PADZ_BIN}\" -g open ${index}"
    assert_success

    # ID should be preserved
    local new_index
    new_index=$(find_pad_by_title "New Title After Update" global)
    local new_id
    new_id=$(get_pad_id "${new_index}" global)

    [[ "${original_id}" == "${new_id}" ]]
}

@test "piped open: empty piped content shows error" {
    # Create a pad
    run bash -c "echo 'Will Try Empty Update' | \"${PADZ_BIN}\" -g create"
    assert_success

    local index
    index=$(find_pad_by_title "Will Try Empty Update" global)

    # Try to update with empty content
    run bash -c "echo '   ' | \"${PADZ_BIN}\" -g open ${index}"
    assert_failure

    # Original content should be preserved
    assert_pad_exists "Will Try Empty Update" global
}

@test "piped open: updates multiple pads with same content" {
    # Create two pads
    run bash -c "echo 'Multi Update Pad A' | \"${PADZ_BIN}\" -g create"
    assert_success
    run bash -c "echo 'Multi Update Pad B' | \"${PADZ_BIN}\" -g create"
    assert_success

    local index_a index_b
    index_a=$(find_pad_by_title "Multi Update Pad A" global)
    index_b=$(find_pad_by_title "Multi Update Pad B" global)

    # Update both with same content
    local new_content="Shared Updated Title

Shared body for both pads."

    run bash -c "echo '${new_content}' | \"${PADZ_BIN}\" -g open ${index_a} ${index_b}"
    assert_success
    assert_output_contains "Updated 2"

    # Both should have the new title
    # Note: After update, both have same title so we can't distinguish by title
    # Just verify the update was reported
    [[ "${output}" == *"Shared Updated Title"* ]]
}

# -----------------------------------------------------------------------------
# EDGE CASES
# -----------------------------------------------------------------------------

@test "piped content: handles multiline content properly" {
    local content="Multiline Test Title

Line 1 of body
Line 2 of body
Line 3 of body

Final paragraph."

    run bash -c "echo '${content}' | \"${PADZ_BIN}\" -g create"
    assert_success

    local index
    index=$(find_pad_by_title "Multiline Test Title" global)
    local actual_content
    actual_content=$(get_pad_content "${index}" global)

    # Check that body content is preserved
    [[ "${actual_content}" == *"Line 1"* ]]
    [[ "${actual_content}" == *"Line 2"* ]]
    [[ "${actual_content}" == *"Line 3"* ]]
    [[ "${actual_content}" == *"Final paragraph"* ]]
}

@test "piped content: handles content with special characters" {
    local content="Special Chars Title

Body with special chars: \$VAR \`backticks\` 'quotes' \"double\""

    run bash -c "printf '%s' '${content}' | \"${PADZ_BIN}\" -g create"
    assert_success

    assert_pad_exists "Special Chars Title" global
}

@test "piped open: works with nested pad path" {
    # Create parent
    run bash -c "echo 'Parent For Nested' | \"${PADZ_BIN}\" -g create"
    assert_success

    local parent_index
    parent_index=$(find_pad_by_title "Parent For Nested" global)

    # Create child inside parent
    run bash -c "echo 'Nested Child' | \"${PADZ_BIN}\" -g create --inside ${parent_index}"
    assert_success

    # Update the nested child using path notation
    local new_content="Updated Nested Child

New content for nested pad."

    run bash -c "echo '${new_content}' | \"${PADZ_BIN}\" -g open ${parent_index}.1"
    assert_success
    assert_output_contains "Updated"
}

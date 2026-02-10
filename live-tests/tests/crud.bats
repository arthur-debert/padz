#!/usr/bin/env bats
# =============================================================================
# CRUD OPERATIONS TESTS
# =============================================================================
# Tests for basic Create, Read, Update, Delete operations.
#
# FIXTURE STATE (from base-fixture.sh)
# ------------------------------------
# Global scope:
#   - "Global pad: Meeting Notes" (completed, tagged: work, important)
#   - "Global pad: Quick Reference" (deleted, pinned, tagged: reference, work)
#   - "Global pad: API Documentation" (tagged: reference)
#   - "Global pad: Projects Overview" (nested parent)
#   - "Global pad: Backend Tasks" (nested child)
#
# Project scope (project-a):
#   - Similar structure with "Project pad:" prefix
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

# -----------------------------------------------------------------------------
# CREATE
# -----------------------------------------------------------------------------

@test "create: new pad appears in list" {
    run "${PADZ_BIN}" -g create --no-editor "Test Create Pad"
    assert_success

    assert_pad_exists "Test Create Pad" global
}

@test "create: new pad has Planned status by default" {
    "${PADZ_BIN}" -g create --no-editor "Status Test Pad" >/dev/null

    local index
    index=$(find_pad_by_title "Status Test Pad" global)
    assert_pad_planned "${index}" global
}

# -----------------------------------------------------------------------------
# READ / LIST
# -----------------------------------------------------------------------------

@test "list: shows fixture pads" {
    # Fixture creates "Global pad: Projects Overview" which is not deleted
    assert_pad_exists "Global pad: Projects Overview" global
}

@test "list: count matches expected fixture pads" {
    # From fixture: 5 global pads created, 1 deleted = 4 active
    # But we may have created more in earlier tests, so check >= 4
    local count
    count=$(count_pads global)
    [[ "${count}" -ge 4 ]]
}

@test "list: project scope is isolated from global" {
    # Global pads should not appear in project scope
    cd "${PROJECT_A}"
    run "${PADZ_BIN}" list --output json

    # Should have project pads, not global pads
    [[ "${output}" == *"Project pad:"* ]]
    [[ "${output}" != *"Global pad: Meeting Notes"* ]]
}

# -----------------------------------------------------------------------------
# UPDATE (via complete/reopen)
# -----------------------------------------------------------------------------

@test "complete: changes status to Done" {
    # Create a fresh pad with unique name
    local pad_name="Complete Test $(date +%s)"
    "${PADZ_BIN}" -g create --no-editor "${pad_name}" >/dev/null

    # Mark as complete using title directly (more reliable than index)
    run "${PADZ_BIN}" -g complete "${pad_name}"
    assert_success

    # Verify status changed by checking JSON output for this specific pad
    local status
    status=$(list_pads global | jq -r --arg t "${pad_name}" '.pads[] | select(.pad.metadata.title == $t) | .pad.metadata.status')
    [[ "${status}" == "Done" ]]
}

@test "reopen: changes status back to Planned" {
    # Create and complete a pad
    "${PADZ_BIN}" -g create --no-editor "Reopen Test Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Reopen Test Pad" global)
    "${PADZ_BIN}" -g complete "${index}" >/dev/null

    # Reopen it
    run "${PADZ_BIN}" -g reopen "${index}"
    assert_success

    # Verify status changed back
    assert_pad_planned "${index}" global
}

# -----------------------------------------------------------------------------
# DELETE
# -----------------------------------------------------------------------------

@test "delete: pad no longer appears in regular list" {
    # Create a pad to delete
    "${PADZ_BIN}" -g create --no-editor "Delete Test Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Delete Test Pad" global)

    # Delete it
    run "${PADZ_BIN}" -g delete "${index}"
    assert_success

    # Should not appear in regular list
    assert_pad_not_exists "Delete Test Pad" global
}

@test "delete: pad appears in deleted list" {
    # Create and delete a pad
    "${PADZ_BIN}" -g create --no-editor "Deleted List Test" >/dev/null
    local index
    index=$(find_pad_by_title "Deleted List Test" global)
    "${PADZ_BIN}" -g delete "${index}" >/dev/null

    # Should appear in deleted list
    local deleted_json
    deleted_json=$(list_deleted_pads global)
    [[ "${deleted_json}" == *"Deleted List Test"* ]]
}

# -----------------------------------------------------------------------------
# CREATE (interactive editor)
# -----------------------------------------------------------------------------

@test "create: interactive mode creates pad (EDITOR=true)" {
    # With EDITOR=true, the editor exits immediately, keeping the initial content
    run "${PADZ_BIN}" -g create "Interactive Create Test"
    assert_success

    assert_pad_exists "Interactive Create Test" global
}

@test "create: interactive mode copies to clipboard" {
    # Only test clipboard on macOS where pbpaste is available
    command -v pbpaste >/dev/null || skip "pbpaste not available"

    "${PADZ_BIN}" -g create "Clipboard Create Test" >/dev/null

    local clipboard
    clipboard=$(pbpaste)
    [[ "${clipboard}" == *"Clipboard Create Test"* ]]
}

@test "create: --no-editor copies to clipboard" {
    command -v pbpaste >/dev/null || skip "pbpaste not available"

    "${PADZ_BIN}" -g create --no-editor "No Editor Clipboard Test" >/dev/null

    local clipboard
    clipboard=$(pbpaste)
    [[ "${clipboard}" == *"No Editor Clipboard Test"* ]]
}

@test "create: piped content copies to clipboard" {
    command -v pbpaste >/dev/null || skip "pbpaste not available"

    echo "Piped Clipboard Title

Body of piped pad." | "${PADZ_BIN}" -g create >/dev/null

    local clipboard
    clipboard=$(pbpaste)
    [[ "${clipboard}" == *"Piped Clipboard Title"* ]]
    [[ "${clipboard}" == *"Body of piped pad."* ]]
}

# -----------------------------------------------------------------------------
# RESTORE
# -----------------------------------------------------------------------------

@test "restore: brings back deleted pad" {
    # Create, delete, then restore
    "${PADZ_BIN}" -g create --no-editor "Restore Test Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Restore Test Pad" global)
    "${PADZ_BIN}" -g delete "${index}" >/dev/null

    # Get the deleted index (d1, d2, etc.)
    run "${PADZ_BIN}" -g restore d1
    assert_success

    # Should be back in active list
    assert_pad_exists "Restore Test Pad" global
}

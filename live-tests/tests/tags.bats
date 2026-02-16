#!/usr/bin/env bats
# =============================================================================
# TAG OPERATIONS TESTS
# =============================================================================
# Tests for the unified `padz tag` subcommand.
#
# FIXTURE STATE (from base-fixture.sh)
# ------------------------------------
# Global tags (auto-created): work, important, reference
# Project tags (auto-created): feature, priority, bug, testing
#
# Tagged pads (global):
#   - "Global pad: Meeting Notes" -> work, important (but completed)
#   - "Global pad: Quick Reference" -> reference (but pinned) -- note: also had reference+work before delete
#   - "Global pad: API Documentation" -> reference, work (but deleted)
#   - "Global pad: Projects Overview" -> (no tags)
#
# TEST GUIDELINES
# ---------------
# 1. Use get_pad_tags() to check tags, not string matching
# 2. Create fresh pads for modification tests to avoid fixture pollution
# 3. Remember fixture state when testing existing pads
# =============================================================================

load '../lib/helpers.bash'
load '../lib/assertions.bash'

# -----------------------------------------------------------------------------
# TAG LIST (registry)
# -----------------------------------------------------------------------------

@test "tag list: shows existing tags" {
    run "${PADZ_BIN}" -g tag list
    assert_success
    assert_output_contains "work"
    assert_output_contains "important"
    assert_output_contains "reference"
}

@test "tag list: no preamble, just tag names" {
    run "${PADZ_BIN}" -g tag list
    assert_success
    # Should NOT contain count preamble
    [[ "${output}" != *"tags defined"* ]]
}

# -----------------------------------------------------------------------------
# TAG LIST (per-pad)
# -----------------------------------------------------------------------------

@test "tag list <id>: shows tags for a specific pad" {
    "${PADZ_BIN}" -g create --no-editor "Pad For Tag List" >/dev/null
    local index
    index=$(find_pad_by_title "Pad For Tag List" global)
    "${PADZ_BIN}" -g tag add "${index}" mylisttag >/dev/null

    run "${PADZ_BIN}" -g tag list "${index}"
    assert_success
    assert_output_contains "mylisttag"
}

@test "tag list <id> <id>: shows tags for multiple pads" {
    "${PADZ_BIN}" -g create --no-editor "Tag List Pad A" >/dev/null
    "${PADZ_BIN}" -g create --no-editor "Tag List Pad B" >/dev/null
    local idx_a idx_b
    idx_a=$(find_pad_by_title "Tag List Pad A" global)
    idx_b=$(find_pad_by_title "Tag List Pad B" global)
    "${PADZ_BIN}" -g tag add "${idx_a}" taga >/dev/null
    "${PADZ_BIN}" -g tag add "${idx_b}" tagb >/dev/null

    run "${PADZ_BIN}" -g tag list "${idx_a}" "${idx_b}"
    assert_success
    assert_output_contains "taga"
    assert_output_contains "tagb"
}

@test "tag list <id>: pad with no tags shows No tags defined" {
    "${PADZ_BIN}" -g create --no-editor "No Tags Pad" >/dev/null
    local index
    index=$(find_pad_by_title "No Tags Pad" global)

    run "${PADZ_BIN}" -g tag list "${index}"
    assert_success
    assert_output_contains "No tags defined"
}

# -----------------------------------------------------------------------------
# ADDING TAGS TO PADS
# -----------------------------------------------------------------------------

@test "tag add: applies tag to pad (auto-creates)" {
    "${PADZ_BIN}" -g create --no-editor "Tag Test Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Tag Test Pad" global)

    run "${PADZ_BIN}" -g tag add "${index}" addtest
    assert_success

    assert_pad_has_tag "${index}" "addtest" global
}

@test "tag add: can add multiple tags at once" {
    "${PADZ_BIN}" -g create --no-editor "Multi Tag Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Multi Tag Pad" global)

    run "${PADZ_BIN}" -g tag add "${index}" multi1 multi2
    assert_success

    assert_pad_has_tag "${index}" "multi1" global
    assert_pad_has_tag "${index}" "multi2" global
}

@test "tag add: multiple pads multiple tags" {
    "${PADZ_BIN}" -g create --no-editor "Batch Tag A" >/dev/null
    "${PADZ_BIN}" -g create --no-editor "Batch Tag B" >/dev/null
    local idx_a idx_b
    idx_a=$(find_pad_by_title "Batch Tag A" global)
    idx_b=$(find_pad_by_title "Batch Tag B" global)

    run "${PADZ_BIN}" -g tag add "${idx_a}" "${idx_b}" batchtag1 batchtag2
    assert_success

    assert_pad_has_tag "${idx_a}" "batchtag1" global
    assert_pad_has_tag "${idx_a}" "batchtag2" global
    assert_pad_has_tag "${idx_b}" "batchtag1" global
    assert_pad_has_tag "${idx_b}" "batchtag2" global
}

# -----------------------------------------------------------------------------
# REMOVING TAGS FROM PADS
# -----------------------------------------------------------------------------

@test "tag remove: removes tag from pad" {
    "${PADZ_BIN}" -g create --no-editor "Remove Tag Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Remove Tag Pad" global)
    "${PADZ_BIN}" -g tag add "${index}" removeme >/dev/null

    run "${PADZ_BIN}" -g tag remove "${index}" removeme
    assert_success

    local tags
    tags=$(get_pad_tags "${index}" global)
    [[ "${tags}" != *"removeme"* ]]
}

# -----------------------------------------------------------------------------
# TAG RENAME & DELETE
# -----------------------------------------------------------------------------

@test "tag rename: renames tag globally" {
    "${PADZ_BIN}" -g create --no-editor "Rename Tag Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Rename Tag Pad" global)
    "${PADZ_BIN}" -g tag add "${index}" oldname >/dev/null

    run "${PADZ_BIN}" -g tag rename oldname newname
    assert_success

    assert_pad_has_tag "${index}" "newname" global
    local tags
    tags=$(get_pad_tags "${index}" global)
    [[ "${tags}" != *"oldname"* ]]
}

@test "tag delete: removes tag from registry and pads" {
    "${PADZ_BIN}" -g create --no-editor "Delete Tag Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Delete Tag Pad" global)
    "${PADZ_BIN}" -g tag add "${index}" deleteme >/dev/null

    run "${PADZ_BIN}" -g tag delete deleteme
    assert_success

    # Should not appear in list anymore
    run "${PADZ_BIN}" -g tag list
    [[ "${output}" != *"deleteme"* ]]

    # Should be gone from pad too
    local tags
    tags=$(get_pad_tags "${index}" global)
    [[ "${tags}" != *"deleteme"* ]]
}

# -----------------------------------------------------------------------------
# FILTERING BY TAG (via padz list --tag)
# -----------------------------------------------------------------------------

@test "list --tag: filters by single tag" {
    "${PADZ_BIN}" -g create --no-editor "Tag Filter Test Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Tag Filter Test Pad" global)
    "${PADZ_BIN}" -g tag add "${index}" reference >/dev/null

    run "${PADZ_BIN}" -g list --tag reference --output json
    assert_success

    [[ "${output}" == *"Tag Filter Test Pad"* ]]
}

@test "list --tag: multiple tags uses AND logic" {
    "${PADZ_BIN}" -g create --no-editor "Both Tags Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Both Tags Pad" global)
    "${PADZ_BIN}" -g tag add "${index}" work important >/dev/null

    run "${PADZ_BIN}" -g list --tag work --tag important --output json
    assert_success

    [[ "${output}" == *"Both Tags Pad"* ]]
}

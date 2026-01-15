#!/usr/bin/env bats
# =============================================================================
# TAG OPERATIONS TESTS
# =============================================================================
# Tests for tag creation, assignment, and filtering.
#
# FIXTURE STATE (from base-fixture.sh)
# ------------------------------------
# Global tags: work, important, reference
# Project tags: feature, priority, bug, testing
#
# Tagged pads (global):
#   - "Global pad: Meeting Notes" -> work, important (but completed)
#   - "Global pad: Quick Reference" -> reference, work (but deleted)
#   - "Global pad: API Documentation" -> reference
#   - "Global pad: Projects Overview" -> important, work
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
# TAG MANAGEMENT
# -----------------------------------------------------------------------------

@test "tags create: creates a new tag" {
    run "${PADZ_BIN}" -g tags create testtag
    assert_success
    assert_output_contains "Created tag"
}

@test "tags create: fails for duplicate tag" {
    # 'work' exists from fixture
    run "${PADZ_BIN}" -g tags create work
    assert_failure
}

@test "tags list: shows existing tags" {
    run "${PADZ_BIN}" -g tags list
    assert_success
    # Fixture creates these tags
    assert_output_contains "work"
    assert_output_contains "important"
    assert_output_contains "reference"
}

@test "tags delete: removes tag" {
    # Create a tag to delete
    "${PADZ_BIN}" -g tags create deleteme >/dev/null

    run "${PADZ_BIN}" -g tags delete deleteme
    assert_success

    # Should not appear in list anymore
    run "${PADZ_BIN}" -g tags list
    [[ "${output}" != *"deleteme"* ]]
}

# -----------------------------------------------------------------------------
# ADDING TAGS TO PADS
# -----------------------------------------------------------------------------

@test "add-tag: applies tag to pad" {
    # Create fresh pad and tag
    "${PADZ_BIN}" -g create --no-editor "Tag Test Pad" >/dev/null
    "${PADZ_BIN}" -g tags create addtest >/dev/null 2>&1 || true

    local index
    index=$(find_pad_by_title "Tag Test Pad" global)

    run "${PADZ_BIN}" -g add-tag -t addtest -- "${index}"
    assert_success

    # Verify tag was added
    assert_pad_has_tag "${index}" "addtest" global
}

@test "add-tag: can add multiple tags at once" {
    "${PADZ_BIN}" -g create --no-editor "Multi Tag Pad" >/dev/null
    "${PADZ_BIN}" -g tags create multi1 >/dev/null 2>&1 || true
    "${PADZ_BIN}" -g tags create multi2 >/dev/null 2>&1 || true

    local index
    index=$(find_pad_by_title "Multi Tag Pad" global)

    run "${PADZ_BIN}" -g add-tag -t multi1 -t multi2 -- "${index}"
    assert_success

    assert_pad_has_tag "${index}" "multi1" global
    assert_pad_has_tag "${index}" "multi2" global
}

# -----------------------------------------------------------------------------
# REMOVING TAGS FROM PADS
# -----------------------------------------------------------------------------

@test "remove-tag: removes tag from pad" {
    "${PADZ_BIN}" -g create --no-editor "Remove Tag Pad" >/dev/null
    "${PADZ_BIN}" -g tags create removeme >/dev/null 2>&1 || true

    local index
    index=$(find_pad_by_title "Remove Tag Pad" global)
    "${PADZ_BIN}" -g add-tag -t removeme -- "${index}" >/dev/null

    # Remove the tag
    run "${PADZ_BIN}" -g remove-tag -t removeme -- "${index}"
    assert_success

    # Tag should be gone
    local tags
    tags=$(get_pad_tags "${index}" global)
    [[ "${tags}" != *"removeme"* ]]
}

# -----------------------------------------------------------------------------
# FILTERING BY TAG
# -----------------------------------------------------------------------------

@test "list --tag: filters by single tag" {
    # Create a fresh pad with a known tag for reliable testing
    "${PADZ_BIN}" -g create --no-editor "Tag Filter Test Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Tag Filter Test Pad" global)
    "${PADZ_BIN}" -g add-tag -t reference -- "${index}" >/dev/null

    run "${PADZ_BIN}" -g list --tag reference --output json
    assert_success

    # Should include our test pad
    [[ "${output}" == *"Tag Filter Test Pad"* ]]
}

@test "list --tag: multiple tags uses AND logic" {
    # Create a pad with both tags for testing
    "${PADZ_BIN}" -g create --no-editor "Both Tags Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Both Tags Pad" global)
    "${PADZ_BIN}" -g add-tag -t work -t important -- "${index}" >/dev/null

    # Filter by both tags
    run "${PADZ_BIN}" -g list --tag work --tag important --output json
    assert_success

    # Should find pads with BOTH tags
    [[ "${output}" == *"Both Tags Pad"* ]]
}

#!/usr/bin/env bats
# =============================================================================
# DOCTOR COMMAND TESTS
# =============================================================================
# Tests for the doctor command which detects and fixes data inconsistencies.
#
# USES BACKDOORS
# --------------
# These tests use backdoor functions to create intentionally broken states:
#   - Orphan files: content files without index entries
#   - Zombie entries: index entries without content files
#
# The doctor command should detect and fix these issues.
#
# TEST GUIDELINES
# ---------------
# 1. Use backdoor functions to create specific broken states
# 2. Run doctor and verify it reports/fixes the issues
# 3. Verify the fix actually worked (pad accessible, etc.)
# =============================================================================

load '../lib/helpers.bash'
load '../lib/assertions.bash'
load '../lib/backdoors.bash'

# -----------------------------------------------------------------------------
# DOCTOR BASICS
# -----------------------------------------------------------------------------

@test "doctor: runs successfully on clean data" {
    run "${PADZ_BIN}" -g doctor
    assert_success
}

@test "doctor: reports no issues when data is consistent" {
    run "${PADZ_BIN}" -g doctor
    assert_success
    # Should not mention recoveries or removals
    [[ "${output}" != *"recovered"* ]] || [[ "${output}" == *"0 recovered"* ]]
}

# -----------------------------------------------------------------------------
# ORPHAN RECOVERY
# -----------------------------------------------------------------------------

@test "doctor: detects orphan content files" {
    # Create an orphan file (content with no metadata)
    local orphan_uuid
    orphan_uuid=$(backdoor_create_orphan "Orphan Test Pad" global)

    # Verify orphan was created
    backdoor_content_exists "${orphan_uuid}" global
    ! backdoor_metadata_exists "${orphan_uuid}" global

    # Run doctor
    run "${PADZ_BIN}" -g doctor
    assert_success

    # Should report recovery (case-insensitive)
    [[ "${output}" == *"Recovered"* ]] || [[ "${output}" == *"recovered"* ]]
}

@test "doctor: recovers orphan into accessible pad" {
    # Create orphan
    local orphan_uuid
    orphan_uuid=$(backdoor_create_orphan "Recoverable Orphan" global)

    # Run doctor to recover it
    "${PADZ_BIN}" -g doctor >/dev/null

    # The pad should now be accessible (title extracted from content)
    assert_pad_exists "Recoverable Orphan" global
}

# -----------------------------------------------------------------------------
# ZOMBIE REMOVAL
# -----------------------------------------------------------------------------

@test "doctor: detects zombie metadata entries" {
    # Create a pad, then remove its content file
    "${PADZ_BIN}" -g create --no-editor "Zombie Test Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Zombie Test Pad" global)
    local uuid
    uuid=$(get_pad_id "${index}" global)

    # Remove content file (creates zombie)
    backdoor_remove_content "${uuid}" global

    # Verify zombie state
    ! backdoor_content_exists "${uuid}" global
    backdoor_metadata_exists "${uuid}" global

    # Run doctor
    run "${PADZ_BIN}" -g doctor
    assert_success

    # Should report removal (case-insensitive)
    [[ "${output}" == *"Removed"* ]] || [[ "${output}" == *"removed"* ]]
}

@test "doctor: zombie pad no longer appears after fix" {
    # Create and zombify a pad
    "${PADZ_BIN}" -g create --no-editor "Zombie Removal Test" >/dev/null
    local index
    index=$(find_pad_by_title "Zombie Removal Test" global)
    local uuid
    uuid=$(get_pad_id "${index}" global)
    backdoor_remove_content "${uuid}" global

    # Run doctor
    "${PADZ_BIN}" -g doctor >/dev/null

    # Pad should no longer exist
    assert_pad_not_exists "Zombie Removal Test" global
}

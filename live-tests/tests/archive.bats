#!/usr/bin/env bats
# =============================================================================
# ARCHIVE / UNARCHIVE TESTS
# =============================================================================
# Tests for archiving pads (moving to cold storage) and unarchiving them.
#
# Archive moves pads from active → archived bucket.
# Unarchive moves pads from archived → active bucket.
# Children always move with their parent.
#
# =============================================================================

load '../lib/helpers.bash'
load '../lib/assertions.bash'
load '../lib/backdoors.bash'

# -----------------------------------------------------------------------------
# ARCHIVE
# -----------------------------------------------------------------------------

@test "archive: pad disappears from active list" {
    "${PADZ_BIN}" -g create --no-editor "Archive Test Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Archive Test Pad" global)

    run "${PADZ_BIN}" -g archive "${index}"
    assert_success

    assert_pad_not_exists "Archive Test Pad" global
}

@test "archive: pad appears in archived list" {
    "${PADZ_BIN}" -g create --no-editor "Archive Visible Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Archive Visible Pad" global)

    "${PADZ_BIN}" -g archive "${index}" >/dev/null

    local archived_json
    archived_json=$(list_archived_pads global)
    [[ "${archived_json}" == *"Archive Visible Pad"* ]]
}

@test "archive: archived pad count increases" {
    local before
    before=$(count_archived_pads global)

    "${PADZ_BIN}" -g create --no-editor "Archive Count Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Archive Count Pad" global)
    "${PADZ_BIN}" -g archive "${index}" >/dev/null

    local after
    after=$(count_archived_pads global)
    [[ "${after}" -gt "${before}" ]]
}

@test "archive: content file moves to archived bucket" {
    "${PADZ_BIN}" -g create --no-editor "Archive File Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Archive File Pad" global)
    local uuid
    uuid=$(get_pad_id "${index}" global)

    # Verify file exists in active bucket
    backdoor_content_exists "${uuid}" global active

    "${PADZ_BIN}" -g archive "${index}" >/dev/null

    # Should now be in archived bucket, not active
    ! backdoor_content_exists "${uuid}" global active
    backdoor_content_exists "${uuid}" global archived
}

@test "archive: parent moves children too" {
    "${PADZ_BIN}" -g create --no-editor "Archive Parent" >/dev/null
    local parent_idx
    parent_idx=$(find_pad_by_title "Archive Parent" global)

    "${PADZ_BIN}" -g create --no-editor --inside "${parent_idx}" "Archive Child" >/dev/null

    # Archive the parent
    "${PADZ_BIN}" -g archive "${parent_idx}" >/dev/null

    # Both should be gone from active
    assert_pad_not_exists "Archive Parent" global
    assert_pad_not_exists "Archive Child" global

    # Both should be in archived
    local archived_json
    archived_json=$(list_archived_pads global)
    [[ "${archived_json}" == *"Archive Parent"* ]]
    [[ "${archived_json}" == *"Archive Child"* ]]
}

# -----------------------------------------------------------------------------
# UNARCHIVE
# -----------------------------------------------------------------------------

@test "unarchive: pad returns to active list" {
    "${PADZ_BIN}" -g create --no-editor "Unarchive Test Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Unarchive Test Pad" global)
    "${PADZ_BIN}" -g archive "${index}" >/dev/null

    # Find the archived index by title
    local ar_index
    ar_index=$(find_archived_pad_by_title "Unarchive Test Pad" global)

    run "${PADZ_BIN}" -g unarchive "${ar_index}"
    assert_success

    assert_pad_exists "Unarchive Test Pad" global
}

@test "unarchive: pad disappears from archived list" {
    "${PADZ_BIN}" -g create --no-editor "Unarchive Gone Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Unarchive Gone Pad" global)
    "${PADZ_BIN}" -g archive "${index}" >/dev/null

    # Verify it's in archived
    local archived_json
    archived_json=$(list_archived_pads global)
    [[ "${archived_json}" == *"Unarchive Gone Pad"* ]]

    # Find the archived index by title and unarchive
    local ar_index
    ar_index=$(find_archived_pad_by_title "Unarchive Gone Pad" global)
    "${PADZ_BIN}" -g unarchive "${ar_index}" >/dev/null

    # Should no longer be in archived list
    archived_json=$(list_archived_pads global)
    [[ "${archived_json}" != *"Unarchive Gone Pad"* ]]
}

@test "unarchive: content file moves back to active bucket" {
    "${PADZ_BIN}" -g create --no-editor "Unarchive File Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Unarchive File Pad" global)
    local uuid
    uuid=$(get_pad_id "${index}" global)

    "${PADZ_BIN}" -g archive "${index}" >/dev/null
    backdoor_content_exists "${uuid}" global archived

    # Find the archived index by title and unarchive
    local ar_index
    ar_index=$(find_archived_pad_by_title "Unarchive File Pad" global)
    "${PADZ_BIN}" -g unarchive "${ar_index}" >/dev/null

    # Should be back in active bucket
    backdoor_content_exists "${uuid}" global active
    ! backdoor_content_exists "${uuid}" global archived
}

@test "unarchive: parent brings children back" {
    "${PADZ_BIN}" -g create --no-editor "Unarchive Parent" >/dev/null
    local parent_idx
    parent_idx=$(find_pad_by_title "Unarchive Parent" global)

    "${PADZ_BIN}" -g create --no-editor --inside "${parent_idx}" "Unarchive Child" >/dev/null

    # Archive parent (takes child)
    "${PADZ_BIN}" -g archive "${parent_idx}" >/dev/null
    assert_pad_not_exists "Unarchive Parent" global
    assert_pad_not_exists "Unarchive Child" global

    # Find the archived parent and unarchive
    local ar_index
    ar_index=$(find_archived_pad_by_title "Unarchive Parent" global)
    "${PADZ_BIN}" -g unarchive "${ar_index}" >/dev/null

    assert_pad_exists "Unarchive Parent" global
    assert_pad_exists "Unarchive Child" global
}

# -----------------------------------------------------------------------------
# DELETE → BUCKET
# -----------------------------------------------------------------------------

@test "delete: content file moves to deleted bucket" {
    "${PADZ_BIN}" -g create --no-editor "Delete Bucket Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Delete Bucket Pad" global)
    local uuid
    uuid=$(get_pad_id "${index}" global)

    backdoor_content_exists "${uuid}" global active

    "${PADZ_BIN}" -g delete "${index}" >/dev/null

    ! backdoor_content_exists "${uuid}" global active
    backdoor_content_exists "${uuid}" global deleted
}

@test "restore: content file moves back to active bucket" {
    "${PADZ_BIN}" -g create --no-editor "Restore Bucket Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Restore Bucket Pad" global)
    local uuid
    uuid=$(get_pad_id "${index}" global)

    "${PADZ_BIN}" -g delete "${index}" >/dev/null
    backdoor_content_exists "${uuid}" global deleted

    # Find the deleted index by title and restore
    local d_index
    d_index=$(find_deleted_pad_by_title "Restore Bucket Pad" global)
    "${PADZ_BIN}" -g restore "${d_index}" >/dev/null

    backdoor_content_exists "${uuid}" global active
    ! backdoor_content_exists "${uuid}" global deleted
}

# -----------------------------------------------------------------------------
# PROJECT SCOPE
# -----------------------------------------------------------------------------

@test "archive: works in project scope" {
    cd "${PROJECT_A}"
    "${PADZ_BIN}" create --no-editor "Project Archive Pad" >/dev/null
    local index
    index=$(find_pad_by_title "Project Archive Pad" project)

    run "${PADZ_BIN}" archive "${index}"
    assert_success

    assert_pad_not_exists "Project Archive Pad" project

    local archived_json
    archived_json=$(list_archived_pads project)
    [[ "${archived_json}" == *"Project Archive Pad"* ]]
}

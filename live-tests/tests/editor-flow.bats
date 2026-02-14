#!/usr/bin/env bats
# =============================================================================
# EDITOR FLOW / DATA DIR TESTS
# =============================================================================
# Tests that pad files live in the .padz/ data dir (not /tmp), and that
# reconciliation correctly handles external file modifications.
#
# NOTE: Interactive editor tests cannot run in bats because stdin is not a
# terminal. The editor path is tested by creating pads and then modifying
# their files directly to simulate what an editor would do, then verifying
# reconciliation picks up the changes.
#
# TEST GUIDELINES
# ---------------
# 1. Each test should verify ONE specific behavior
# 2. Use helpers (get_pad_title, etc.) instead of grep
# 3. Use assertions for clear failure messages
# =============================================================================

load '../lib/helpers.bash'
load '../lib/assertions.bash'
load '../lib/backdoors.bash'

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
# PAD FILES LIVE IN DATA DIR
# -----------------------------------------------------------------------------

@test "pad files are stored in data dir" {
    "${PADZ_BIN}" -g create --no-editor "Data Dir Test" >/dev/null

    # Verify there's a pad-*.txt file in the data dir
    local pad_count
    pad_count=$(ls "${PADZ_GLOBAL_DATA}"/pad-*.txt 2>/dev/null | wc -l | tr -d ' ')
    [[ "${pad_count}" -gt 0 ]]
}

@test "pad file uses pad-uuid.txt naming" {
    "${PADZ_BIN}" -g create --no-editor "Naming Test" >/dev/null

    local pad_file
    pad_file=$(ls "${PADZ_GLOBAL_DATA}"/pad-*.txt 2>/dev/null | head -1)
    local filename
    filename=$(basename "${pad_file}")

    # Should match pad-{uuid}.txt pattern
    [[ "${filename}" =~ ^pad-[0-9a-f-]+\.txt$ ]]
}

@test "pad file content matches pad content" {
    "${PADZ_BIN}" -g create --no-editor "Content Match" >/dev/null

    local pad_file
    pad_file=$(ls "${PADZ_GLOBAL_DATA}"/pad-*.txt 2>/dev/null | head -1)
    local file_content
    file_content=$(cat "${pad_file}")

    # File should contain the title
    [[ "${file_content}" == *"Content Match"* ]]
}

# -----------------------------------------------------------------------------
# RECONCILIATION: EXTERNAL FILE EDITS
# (simulates what happens after editor modifies a file)
# -----------------------------------------------------------------------------

@test "reconciliation picks up title change in file" {
    "${PADZ_BIN}" -g create --no-editor "Original Title" >/dev/null

    # Find the pad file and change its content (simulating editor)
    local pad_file
    pad_file=$(ls "${PADZ_GLOBAL_DATA}"/pad-*.txt 2>/dev/null | head -1)

    # Write new content with different title
    printf 'Changed Title\n\nNew body content' > "${pad_file}"

    # Touch the file to ensure mtime is newer (triggers staleness detection)
    touch -t "$(date -v+1H '+%Y%m%d%H%M.%S')" "${pad_file}" 2>/dev/null || \
    touch -d '+1 hour' "${pad_file}" 2>/dev/null || \
    sleep 1  # fallback: just wait

    # List triggers reconciliation which picks up file changes
    assert_pad_exists "Changed Title" global
}

@test "reconciliation removes empty files" {
    "${PADZ_BIN}" -g create --no-editor "Will Be Empty" >/dev/null
    assert_pad_exists "Will Be Empty" global

    # Empty the pad file (simulating editor clearing content)
    local pad_file
    pad_file=$(ls "${PADZ_GLOBAL_DATA}"/pad-*.txt 2>/dev/null | head -1)
    truncate -s 0 "${pad_file}"

    # Touch to ensure mtime is newer
    touch -t "$(date -v+1H '+%Y%m%d%H%M.%S')" "${pad_file}" 2>/dev/null || \
    touch -d '+1 hour' "${pad_file}" 2>/dev/null || \
    sleep 1

    # Doctor or list triggers reconciliation - pad should be cleaned up
    run "${PADZ_BIN}" -g doctor
    assert_success

    local count
    count=$(count_pads global)
    [[ "${count}" -eq 0 ]]
}

@test "refresh_pad updates metadata after file edit" {
    # Create pad via pipe
    run bash -c "printf 'Original Title\n\nOriginal body' | \"${PADZ_BIN}\" -g create"
    assert_success
    assert_pad_exists "Original Title" global

    # Find and modify the file
    local pad_file
    pad_file=$(ls "${PADZ_GLOBAL_DATA}"/pad-*.txt 2>/dev/null | head -1)
    printf 'Updated Title\n\nUpdated body via file edit' > "${pad_file}"

    # Touch to trigger staleness
    touch -t "$(date -v+1H '+%Y%m%d%H%M.%S')" "${pad_file}" 2>/dev/null || \
    touch -d '+1 hour' "${pad_file}" 2>/dev/null || \
    sleep 1

    # List triggers reconciliation
    assert_pad_exists "Updated Title" global
    assert_pad_not_exists "Original Title" global
}

# -----------------------------------------------------------------------------
# PIPED CONTENT (still works as before)
# -----------------------------------------------------------------------------

@test "piped create works" {
    run bash -c "printf 'Piped Title\n\nPiped body' | \"${PADZ_BIN}\" -g create"
    assert_success
    assert_output_contains "Created"

    assert_pad_exists "Piped Title" global
}

@test "piped edit works" {
    bash -c "printf 'Original\n\nOriginal body' | \"${PADZ_BIN}\" -g create" >/dev/null

    local index
    index=$(find_pad_by_title "Original" global)

    run bash -c "printf 'Updated Via Pipe\n\nNew body' | \"${PADZ_BIN}\" -g open ${index}"
    assert_success

    assert_pad_exists "Updated Via Pipe" global
}

@test "empty piped create aborts" {
    run bash -c "printf '' | \"${PADZ_BIN}\" -g create"
    assert_success
    assert_output_contains "Aborted"

    local count
    count=$(count_pads global)
    [[ "${count}" -eq 0 ]]
}

@test "empty piped edit fails" {
    bash -c "printf 'Test Pad\n\nBody' | \"${PADZ_BIN}\" -g create" >/dev/null

    local index
    index=$(find_pad_by_title "Test Pad" global)

    run bash -c "printf '   ' | \"${PADZ_BIN}\" -g open ${index}"
    assert_failure
}

# -----------------------------------------------------------------------------
# NO TMP FILES CREATED
# -----------------------------------------------------------------------------

@test "no pad files created in system tmp dir" {
    # Record tmp dir state before
    local tmp_before
    tmp_before=$(ls /tmp/pad-* 2>/dev/null | wc -l | tr -d ' ')

    bash -c "printf 'Tmp Test\n\nBody' | \"${PADZ_BIN}\" -g create" >/dev/null

    # Check no new pad files in /tmp
    local tmp_after
    tmp_after=$(ls /tmp/pad-* 2>/dev/null | wc -l | tr -d ' ')

    [[ "${tmp_before}" == "${tmp_after}" ]]
}

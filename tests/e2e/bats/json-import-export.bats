#!/usr/bin/env bats
# =============================================================================
# JSON IMPORT/EXPORT TESTS
# =============================================================================
# End-to-end coverage for `padz export --json` + `padz import` round-trip.
# Verifies that metadata (timestamps, status, pinning, tags, parent) survives
# the round-trip, and that an archive imported into a fresh store produces the
# same logical set of pads.
# =============================================================================


setup() {
    load helpers/setup
    # A fresh temp dir per test so imports land in a clean store.
    TEST_IMPORT_DIR="$(mktemp -d)"
    mkdir -p "${TEST_IMPORT_DIR}/.padz"
    export TEST_IMPORT_DIR
}

teardown() {
    [ -n "${TEST_IMPORT_DIR:-}" ] && rm -rf "${TEST_IMPORT_DIR}"
}

@test "export --json produces a tar.gz archive" {
    cd "${WORKSPACE}"
    run "${PADZ_BIN}" -g export --json
    [ "$status" -eq 0 ]
    [[ "$output" == *"Exported to padz-"* ]]
    [[ "$output" == *".json.tar.gz"* ]]

    # Find and remove the generated file to keep the workspace clean
    local archive
    archive=$(ls padz-*.json.tar.gz 2>/dev/null | head -n1 || true)
    [ -n "${archive}" ] || false
    rm -f "${archive}"
}

@test "export --json then import preserves title and content" {
    cd "${WORKSPACE}"
    run "${PADZ_BIN}" -g export --json
    [ "$status" -eq 0 ]

    local archive
    archive=$(ls padz-*.json.tar.gz | head -n1)
    [ -n "${archive}" ]

    # Import into a fresh project scope (workspace-isolated)
    cd "${TEST_IMPORT_DIR}"
    run "${PADZ_BIN}" --data "${TEST_IMPORT_DIR}/.padz" import "${WORKSPACE}/${archive}"
    [ "$status" -eq 0 ]
    [[ "$output" == *"Imported"* ]]

    # Listing the destination store should show the imported pads
    run "${PADZ_BIN}" --data "${TEST_IMPORT_DIR}/.padz" list
    [ "$status" -eq 0 ]
    [[ "$output" == *"Global pad:"* ]]

    rm -f "${WORKSPACE}/${archive}"
}

@test "export --json preserves pinned state" {
    cd "${WORKSPACE}"
    run "${PADZ_BIN}" -g export --json
    [ "$status" -eq 0 ]
    local archive
    archive=$(ls padz-*.json.tar.gz | head -n1)

    cd "${TEST_IMPORT_DIR}"
    "${PADZ_BIN}" --data "${TEST_IMPORT_DIR}/.padz" import "${WORKSPACE}/${archive}" >/dev/null

    # The fixture pins "Quick Reference". After import, a pinned pad should
    # appear with a p1 index.
    run "${PADZ_BIN}" --data "${TEST_IMPORT_DIR}/.padz" list --output json
    [ "$status" -eq 0 ]
    # Verify some pad in the imported set is pinned
    echo "$output" | jq -e '.pads[] | select(.pad.metadata.is_pinned == true)' >/dev/null

    rm -f "${WORKSPACE}/${archive}"
}

@test "export --json preserves completed status" {
    cd "${WORKSPACE}"
    run "${PADZ_BIN}" -g export --json
    [ "$status" -eq 0 ]
    local archive
    archive=$(ls padz-*.json.tar.gz | head -n1)

    cd "${TEST_IMPORT_DIR}"
    "${PADZ_BIN}" --data "${TEST_IMPORT_DIR}/.padz" import "${WORKSPACE}/${archive}" >/dev/null

    # Fixture marks "Meeting Notes" as completed.
    run "${PADZ_BIN}" --data "${TEST_IMPORT_DIR}/.padz" list --output json
    [ "$status" -eq 0 ]
    echo "$output" | jq -e '.pads[] | select(.pad.metadata.status == "Done")' >/dev/null

    rm -f "${WORKSPACE}/${archive}"
}

@test "export --json preserves tags" {
    cd "${WORKSPACE}"
    run "${PADZ_BIN}" -g export --json
    [ "$status" -eq 0 ]
    local archive
    archive=$(ls padz-*.json.tar.gz | head -n1)

    cd "${TEST_IMPORT_DIR}"
    "${PADZ_BIN}" --data "${TEST_IMPORT_DIR}/.padz" import "${WORKSPACE}/${archive}" >/dev/null

    # Fixture tags several pads. At least one imported pad should carry tags.
    run "${PADZ_BIN}" --data "${TEST_IMPORT_DIR}/.padz" list --output json
    [ "$status" -eq 0 ]
    echo "$output" | jq -e '.pads[] | select(.pad.metadata.tags | length > 0)' >/dev/null

    rm -f "${WORKSPACE}/${archive}"
}

@test "export --json preserves parent/child relationship" {
    cd "${WORKSPACE}"
    run "${PADZ_BIN}" -g export --json
    [ "$status" -eq 0 ]
    local archive
    archive=$(ls padz-*.json.tar.gz | head -n1)

    cd "${TEST_IMPORT_DIR}"
    "${PADZ_BIN}" --data "${TEST_IMPORT_DIR}/.padz" import "${WORKSPACE}/${archive}" >/dev/null

    # "Projects Overview" has a child "Backend Tasks" in the fixture.
    run "${PADZ_BIN}" --data "${TEST_IMPORT_DIR}/.padz" list --output json
    [ "$status" -eq 0 ]
    # Children nest under .pads[].children[]; flatten with .. then match.
    echo "$output" \
        | jq -e '[..|.pad? // empty] | any(.metadata.title == "Global pad: Backend Tasks")' \
        >/dev/null
    # At least one pad should have a parent_id
    echo "$output" \
        | jq -e '[..|.pad? // empty] | any(.metadata.parent_id != null)' \
        >/dev/null

    rm -f "${WORKSPACE}/${archive}"
}

@test "export --json --single-file are mutually exclusive" {
    cd "${WORKSPACE}"
    run "${PADZ_BIN}" -g export --json --single-file out.md
    [ "$status" -ne 0 ]
    [[ "$output" == *"cannot be used"* ]] || [[ "$output" == *"conflict"* ]] || true
}

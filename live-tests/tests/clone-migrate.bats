#!/usr/bin/env bats
# =============================================================================
# CLONE / MIGRATE TESTS
# =============================================================================
# End-to-end tests for `padz clone` and `padz migrate`:
#   - `clone --to <path>`: copy pads to another store, keep source
#   - `clone --from <path>`: copy pads from another store into the current
#   - `migrate --to <path>`: move pads (remove from source)
#   - Metadata preservation across store boundaries
# =============================================================================

load '../lib/helpers.bash'
load '../lib/assertions.bash'

setup() {
    SRC_DIR="$(mktemp -d)"
    DST_DIR="$(mktemp -d)"
    mkdir -p "${SRC_DIR}/.padz" "${DST_DIR}/.padz"
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" init >/dev/null 2>&1
    "${PADZ_BIN}" --data "${DST_DIR}/.padz" init >/dev/null 2>&1
    export SRC_DIR DST_DIR
}

teardown() {
    [ -n "${SRC_DIR:-}" ] && rm -rf "${SRC_DIR}"
    [ -n "${DST_DIR:-}" ] && rm -rf "${DST_DIR}"
}

@test "clone --to copies pads to destination, source kept" {
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" create --no-editor "Alpha" >/dev/null
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" create --no-editor "Beta" >/dev/null

    run "${PADZ_BIN}" --data "${SRC_DIR}/.padz" clone 1 --to "${DST_DIR}"
    [ "$status" -eq 0 ]
    [[ "$output" == *"Cloned 1 pad"* ]]

    # Source still has both
    run "${PADZ_BIN}" --data "${SRC_DIR}/.padz" list
    [[ "$output" == *"Alpha"* ]]
    [[ "$output" == *"Beta"* ]]

    # Destination has the one we cloned (most recent = Beta at index 1)
    run "${PADZ_BIN}" --data "${DST_DIR}/.padz" list
    [[ "$output" == *"Beta"* ]]
}

@test "clone --from copies pads into current store from external" {
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" create --no-editor "External Pad" >/dev/null

    run "${PADZ_BIN}" --data "${DST_DIR}/.padz" clone 1 --from "${SRC_DIR}"
    [ "$status" -eq 0 ]
    [[ "$output" == *"Cloned 1 pad"* ]]

    run "${PADZ_BIN}" --data "${DST_DIR}/.padz" list
    [[ "$output" == *"External Pad"* ]]

    # Source still has it
    run "${PADZ_BIN}" --data "${SRC_DIR}/.padz" list
    [[ "$output" == *"External Pad"* ]]
}

@test "migrate --to moves pads and removes from source" {
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" create --no-editor "Move Me" >/dev/null

    run "${PADZ_BIN}" --data "${SRC_DIR}/.padz" migrate 1 --to "${DST_DIR}"
    [ "$status" -eq 0 ]
    [[ "$output" == *"Migrated 1 pad"* ]]

    # Source empty
    run "${PADZ_BIN}" --data "${SRC_DIR}/.padz" list --output json
    [[ "$(echo "$output" | jq '.pads | length')" == "0" ]]

    # Destination has it
    run "${PADZ_BIN}" --data "${DST_DIR}/.padz" list
    [[ "$output" == *"Move Me"* ]]
}

@test "clone preserves metadata (pinned, tagged)" {
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" create --no-editor "Meta" >/dev/null
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" pin 1 >/dev/null
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" tag add 1 work >/dev/null

    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" clone 1 --to "${DST_DIR}" >/dev/null

    run "${PADZ_BIN}" --data "${DST_DIR}/.padz" list --output json
    [ "$status" -eq 0 ]
    echo "$output" | jq -e '.pads[] | select(.pad.metadata.is_pinned == true)' >/dev/null
    echo "$output" | jq -e '[..|.pad? // empty] | any(.metadata.tags | index("work"))' >/dev/null
}

@test "clone preserves parent/child when both are selected" {
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" create --no-editor "Parent" >/dev/null
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" create --no-editor --inside 1 "Child" >/dev/null

    # Clone both (ranges work; 1 is parent, 1.1 is child)
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" clone 1 1.1 --to "${DST_DIR}" >/dev/null

    run "${PADZ_BIN}" --data "${DST_DIR}/.padz" list --output json
    [ "$status" -eq 0 ]
    # Child should have a parent_id pointing at Parent
    echo "$output" \
        | jq -e '[..|.pad? // empty] | any(.metadata.title == "Child" and .metadata.parent_id != null)' \
        >/dev/null
}

@test "clone --to and --from are mutually exclusive" {
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" create --no-editor "x" >/dev/null

    run "${PADZ_BIN}" --data "${SRC_DIR}/.padz" clone 1 --to "${DST_DIR}" --from "${DST_DIR}"
    [ "$status" -ne 0 ]
}

@test "clone errors when neither --to nor --from is given" {
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" create --no-editor "x" >/dev/null

    run "${PADZ_BIN}" --data "${SRC_DIR}/.padz" clone 1
    [ "$status" -ne 0 ]
}

@test "migrate refuses to target the current store (data loss guard)" {
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" create --no-editor "Dangerous" >/dev/null

    # Target is the same .padz the command is already operating on
    run "${PADZ_BIN}" --data "${SRC_DIR}/.padz" migrate 1 --to "${SRC_DIR}"
    [ "$status" -ne 0 ]
    [[ "$output" == *"current store"* ]] || [[ "$output" == *"same"* ]]

    # Source pad must still be intact
    run "${PADZ_BIN}" --data "${SRC_DIR}/.padz" list
    [[ "$output" == *"Dangerous"* ]]
}

@test "clone reports warning when all copies fail (no pads copied)" {
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" create --no-editor "Alpha" >/dev/null

    # Use an index that doesn't exist. Clone should exit non-zero (selector
    # resolution failure propagates as an error).
    run "${PADZ_BIN}" --data "${SRC_DIR}/.padz" clone 99 --to "${DST_DIR}"
    [ "$status" -ne 0 ]
}

@test "clone resolves <path> smart: accepts .padz dir directly" {
    "${PADZ_BIN}" --data "${SRC_DIR}/.padz" create --no-editor "Smart" >/dev/null

    run "${PADZ_BIN}" --data "${SRC_DIR}/.padz" clone 1 --to "${DST_DIR}/.padz"
    [ "$status" -eq 0 ]

    run "${PADZ_BIN}" --data "${DST_DIR}/.padz" list
    [[ "$output" == *"Smart"* ]]
}

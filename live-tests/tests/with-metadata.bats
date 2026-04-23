#!/usr/bin/env bats
# =============================================================================
# --with-metadata export / import TESTS
# =============================================================================
# End-to-end coverage for `padz export --with-metadata`: md pads get
# YAML frontmatter, lex pads get `:: padz.KEY ::` annotations, txt pads are
# exported without metadata and listed in a warning. Re-importing reads
# metadata back into the store.
# =============================================================================

load '../lib/helpers.bash'
load '../lib/assertions.bash'

setup() {
    TEST_IMPORT_DIR="$(mktemp -d)"
    mkdir -p "${TEST_IMPORT_DIR}/.padz"
    EXPORT_DIR="$(mktemp -d)"
    export TEST_IMPORT_DIR EXPORT_DIR
}

teardown() {
    [ -n "${TEST_IMPORT_DIR:-}" ] && rm -rf "${TEST_IMPORT_DIR}"
    [ -n "${EXPORT_DIR:-}" ] && rm -rf "${EXPORT_DIR}"
}

@test "export --with-metadata produces a tar.gz with native extensions" {
    local src="$(mktemp -d)/.padz"
    mkdir -p "$src"

    # Create an md pad
    "${PADZ_BIN}" --data "$src" config set format md >/dev/null
    "${PADZ_BIN}" --data "$src" create --no-editor "Test Md Pad" >/dev/null

    cd "${EXPORT_DIR}"
    run "${PADZ_BIN}" --data "$src" export --with-metadata
    [ "$status" -eq 0 ]
    [[ "$output" == *"Exported to padz-"* ]]
    [[ "$output" == *".meta.gz"* ]]

    local archive
    archive=$(ls padz-*.meta.gz | head -n1)
    [ -n "${archive}" ]

    # Inspect the archive for .md content + YAML frontmatter
    local extracted="$(mktemp -d)"
    tar -xzf "${archive}" -C "${extracted}"
    [ -d "${extracted}/padz" ]
    local md_file
    md_file=$(find "${extracted}/padz" -name "*.md" | head -n1)
    [ -n "${md_file}" ]
    grep -q "^---$" "${md_file}"
    grep -q "^padz.id:" "${md_file}"
    grep -q "^padz.status:" "${md_file}"
}

@test "export --with-metadata + re-import preserves pinned state (md)" {
    local src="$(mktemp -d)/.padz"
    mkdir -p "$src"

    "${PADZ_BIN}" --data "$src" config set format md >/dev/null
    "${PADZ_BIN}" --data "$src" create --no-editor "Alpha" >/dev/null
    "${PADZ_BIN}" --data "$src" pin 1 >/dev/null
    "${PADZ_BIN}" --data "$src" tag add 1 work >/dev/null

    cd "${EXPORT_DIR}"
    "${PADZ_BIN}" --data "$src" export --with-metadata >/dev/null
    local archive
    archive=$(ls padz-*.meta.gz | head -n1)

    # Extract the .md file; re-import the single file into a fresh store
    local extracted="$(mktemp -d)"
    tar -xzf "${archive}" -C "${extracted}"
    local md_file
    md_file=$(find "${extracted}/padz" -name "*.md" | head -n1)

    cd "${TEST_IMPORT_DIR}"
    run "${PADZ_BIN}" --data "${TEST_IMPORT_DIR}/.padz" import "${md_file}"
    [ "$status" -eq 0 ]

    run "${PADZ_BIN}" --data "${TEST_IMPORT_DIR}/.padz" list --output json
    [ "$status" -eq 0 ]
    # Pinned + tagged pad should round-trip
    echo "$output" | jq -e '.pads[] | select(.pad.metadata.is_pinned == true)' >/dev/null
    echo "$output" | jq -e '[..|.pad? // empty] | any(.metadata.tags | index("work"))' >/dev/null
}

@test "export --with-metadata warns about txt pads being skipped" {
    local src="$(mktemp -d)/.padz"
    mkdir -p "$src"

    # Default format is txt
    "${PADZ_BIN}" --data "$src" create --no-editor "Txt Pad" >/dev/null

    cd "${EXPORT_DIR}"
    run "${PADZ_BIN}" --data "$src" export --with-metadata
    [ "$status" -eq 0 ]
    [[ "$output" == *"txt pad(s) exported without metadata"* ]]
}

@test "export --with-metadata + --json are mutually exclusive" {
    local src="$(mktemp -d)/.padz"
    mkdir -p "$src"
    "${PADZ_BIN}" --data "$src" create --no-editor "x" >/dev/null 2>&1 || true

    cd "${EXPORT_DIR}"
    run "${PADZ_BIN}" --data "$src" export --with-metadata --json
    [ "$status" -ne 0 ]
}

@test "export --with-metadata + --single-file are mutually exclusive" {
    local src="$(mktemp -d)/.padz"
    mkdir -p "$src"
    "${PADZ_BIN}" --data "$src" create --no-editor "x" >/dev/null 2>&1 || true

    cd "${EXPORT_DIR}"
    run "${PADZ_BIN}" --data "$src" export --with-metadata --single-file out.md
    [ "$status" -ne 0 ]
}

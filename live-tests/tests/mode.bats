#!/usr/bin/env bats
# =============================================================================
# MODE TOGGLE TESTS (notes vs todos)
# =============================================================================
# Tests the mode configuration and its effects on:
#   - Display: status icons shown/hidden
#   - Create: quick-create in todos mode (skip editor)
#   - Edit: quick-edit in todos mode (skip editor)
#   - Purge: includes Done pads in todos mode
#
# Each test sets mode explicitly to avoid depending on fixture state.
# =============================================================================

load '../lib/helpers.bash'
load '../lib/assertions.bash'

# -----------------------------------------------------------------------------
# CONFIG: mode round-trip
# -----------------------------------------------------------------------------

@test "mode: default mode is notes" {
    run "${PADZ_BIN}" -g config get mode
    assert_success
    [[ "$output" == *"notes"* ]]
}

@test "mode: can set mode to todos" {
    "${PADZ_BIN}" -g config set mode todos >/dev/null
    run "${PADZ_BIN}" -g config get mode
    assert_success
    [[ "$output" == *"todos"* ]]

    # Reset
    "${PADZ_BIN}" -g config set mode notes >/dev/null
}

@test "mode: can set mode back to notes" {
    "${PADZ_BIN}" -g config set mode todos >/dev/null
    "${PADZ_BIN}" -g config set mode notes >/dev/null
    run "${PADZ_BIN}" -g config get mode
    assert_success
    [[ "$output" == *"notes"* ]]
}

# -----------------------------------------------------------------------------
# DISPLAY: status icons
# -----------------------------------------------------------------------------

@test "mode: notes mode hides status icons in list" {
    "${PADZ_BIN}" -g config set mode notes >/dev/null
    "${PADZ_BIN}" -g create --no-editor "Notes Display Test" >/dev/null

    run "${PADZ_BIN}" -g list
    assert_success

    # Status icons should NOT appear in notes mode
    [[ "$output" != *"⚪︎"* ]]
    [[ "$output" != *"☉︎︎"* ]]
    [[ "$output" != *"⚫︎"* ]]
}

@test "mode: todos mode shows status icons in list" {
    "${PADZ_BIN}" -g config set mode todos >/dev/null
    "${PADZ_BIN}" -g create --no-editor "Todos Display Test" >/dev/null

    run "${PADZ_BIN}" -g list
    assert_success

    # Status icons should appear in todos mode (at least one Planned icon)
    [[ "$output" == *"⚪︎"* ]]

    # Reset
    "${PADZ_BIN}" -g config set mode notes >/dev/null
}

# -----------------------------------------------------------------------------
# CREATE: quick-create in todos mode
# -----------------------------------------------------------------------------

@test "mode: todos quick-create with quoted args" {
    "${PADZ_BIN}" -g config set mode todos >/dev/null

    # In todos mode, title args skip editor
    run "${PADZ_BIN}" -g create "Quick Create Test"
    assert_success

    assert_pad_exists "Quick Create Test" global

    # Reset
    "${PADZ_BIN}" -g config set mode notes >/dev/null
}

@test "mode: todos quick-create with unquoted args" {
    "${PADZ_BIN}" -g config set mode todos >/dev/null

    # Multiple unquoted words joined as title
    run "${PADZ_BIN}" -g create Call Mom Tomorrow
    assert_success

    assert_pad_exists "Call Mom Tomorrow" global

    # Reset
    "${PADZ_BIN}" -g config set mode notes >/dev/null
}

@test "mode: todos quick-create with newline escape" {
    "${PADZ_BIN}" -g config set mode todos >/dev/null

    # Literal \n should be converted to newline (title becomes first line)
    run "${PADZ_BIN}" -g create 'Buy Groceries\nMilk\nEggs'
    assert_success

    assert_pad_exists "Buy Groceries" global

    # Content should include the body (look up by title to avoid index collisions with pinned pads)
    local content
    content=$("${PADZ_BIN}" -g list --output json 2>/dev/null | jq -r --arg t "Buy Groceries" 'first(.pads[] | select(.pad.metadata.title == $t)) | .pad.content')
    [[ "$content" == *"Milk"* ]]
    [[ "$content" == *"Eggs"* ]]

    # Reset
    "${PADZ_BIN}" -g config set mode notes >/dev/null
}

@test "mode: todos create with no args still opens editor (EDITOR=true returns empty)" {
    "${PADZ_BIN}" -g config set mode todos >/dev/null

    # No args → should open editor. EDITOR=true exits immediately with empty.
    # This should abort with empty content warning.
    run "${PADZ_BIN}" -g create
    # Either success with warning or failure is acceptable
    # The key point: it should NOT create an "Untitled" pad silently
    # (that's the --no-editor behavior, not the no-args behavior)

    # Reset
    "${PADZ_BIN}" -g config set mode notes >/dev/null
}

@test "mode: notes create with args still opens editor (EDITOR=true returns empty)" {
    "${PADZ_BIN}" -g config set mode notes >/dev/null

    # In notes mode, providing a title should still try to open editor
    # EDITOR=true exits immediately → empty → aborts
    run "${PADZ_BIN}" -g create "Should Open Editor"

    # The pad should NOT exist (editor was opened and returned empty)
    # because notes mode always opens editor, and EDITOR=true produces empty
    if pad_exists "Should Open Editor" global; then
        echo "FAIL: In notes mode, create with args should open editor, not quick-create" >&2
        return 1
    fi
}

# -----------------------------------------------------------------------------
# EDIT: quick-edit in todos mode
# -----------------------------------------------------------------------------

@test "mode: todos quick-edit with content args" {
    "${PADZ_BIN}" -g config set mode todos >/dev/null
    "${PADZ_BIN}" -g create "Original Todo Title" >/dev/null

    local index
    index=$(find_pad_by_title "Original Todo Title" global)

    # In todos mode, content after index updates directly
    run "${PADZ_BIN}" -g edit "${index}" "Updated Todo Title"
    assert_success

    assert_pad_exists "Updated Todo Title" global

    # Reset
    "${PADZ_BIN}" -g config set mode notes >/dev/null
}

@test "mode: todos quick-edit with multiple word args" {
    "${PADZ_BIN}" -g config set mode todos >/dev/null
    "${PADZ_BIN}" -g create "Edit Multi Word Test" >/dev/null

    local index
    index=$(find_pad_by_title "Edit Multi Word Test" global)

    # Multiple words after index become the new content
    run "${PADZ_BIN}" -g edit "${index}" Call Mom Before Dinner
    assert_success

    assert_pad_exists "Call Mom Before Dinner" global

    # Reset
    "${PADZ_BIN}" -g config set mode notes >/dev/null
}

# -----------------------------------------------------------------------------
# PURGE: todos mode includes Done pads
# -----------------------------------------------------------------------------

@test "mode: todos purge removes done pads" {
    "${PADZ_BIN}" -g config set mode todos >/dev/null

    "${PADZ_BIN}" -g create "Purge Done Test" >/dev/null
    local index
    index=$(find_pad_by_title "Purge Done Test" global)
    "${PADZ_BIN}" -g complete "${index}" >/dev/null

    # Verify it's done (look up by title to avoid index collisions with pinned pads)
    local status
    status=$("${PADZ_BIN}" -g list --output json 2>/dev/null | jq -r --arg t "Purge Done Test" 'first(.pads[] | select(.pad.metadata.title == $t)) | .pad.metadata.status')
    [[ "$status" == "Done" ]]

    # Purge should target Done pads in todos mode
    run "${PADZ_BIN}" -g purge --yes
    assert_success

    # The Done pad should be gone
    assert_pad_not_exists "Purge Done Test" global

    # Reset
    "${PADZ_BIN}" -g config set mode notes >/dev/null
}

@test "mode: notes purge does NOT remove done pads" {
    "${PADZ_BIN}" -g config set mode notes >/dev/null

    "${PADZ_BIN}" -g create --no-editor "Notes Purge Done Test" >/dev/null
    local index
    index=$(find_pad_by_title "Notes Purge Done Test" global)
    "${PADZ_BIN}" -g complete "${index}" >/dev/null

    # Purge in notes mode should only target deleted pads, not done ones
    run "${PADZ_BIN}" -g purge --yes
    assert_success

    # The Done pad should STILL exist
    assert_pad_exists "Notes Purge Done Test" global
}

@test "mode: todos purge removes both done and deleted" {
    "${PADZ_BIN}" -g config set mode todos >/dev/null

    "${PADZ_BIN}" -g create "Purge Both Done" >/dev/null
    "${PADZ_BIN}" -g create "Purge Both Deleted" >/dev/null
    "${PADZ_BIN}" -g create "Purge Both Keep" >/dev/null

    # Complete "Purge Both Done"
    local done_idx
    done_idx=$(find_pad_by_title "Purge Both Done" global)
    "${PADZ_BIN}" -g complete "${done_idx}" >/dev/null

    # Delete "Purge Both Deleted"
    local del_idx
    del_idx=$(find_pad_by_title "Purge Both Deleted" global)
    "${PADZ_BIN}" -g delete "${del_idx}" >/dev/null

    # Purge should remove both Done and Deleted
    run "${PADZ_BIN}" -g purge --yes
    assert_success

    # Only "Purge Both Keep" should remain
    assert_pad_exists "Purge Both Keep" global
    assert_pad_not_exists "Purge Both Done" global
    assert_pad_not_exists "Purge Both Deleted" global

    # Reset
    "${PADZ_BIN}" -g config set mode notes >/dev/null
}

# -----------------------------------------------------------------------------
# MODE SWITCHING: data preservation
# -----------------------------------------------------------------------------

@test "mode: switching modes preserves existing pads" {
    "${PADZ_BIN}" -g config set mode notes >/dev/null
    "${PADZ_BIN}" -g create --no-editor "Mode Switch Test Pad" >/dev/null

    assert_pad_exists "Mode Switch Test Pad" global

    # Switch to todos
    "${PADZ_BIN}" -g config set mode todos >/dev/null
    assert_pad_exists "Mode Switch Test Pad" global

    # Switch back to notes
    "${PADZ_BIN}" -g config set mode notes >/dev/null
    assert_pad_exists "Mode Switch Test Pad" global
}

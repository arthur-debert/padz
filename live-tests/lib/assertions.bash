#!/usr/bin/env bash
# =============================================================================
# PADZ TEST ASSERTIONS
# =============================================================================
#
# PURPOSE
# -------
# Provides clear, semantic assertions for bats tests.
# These wrap the helpers and provide meaningful error messages.
#
# DESIGN PRINCIPLES
# -----------------
# 1. Assertions should have clear names describing what they check
# 2. Failure messages should explain what was expected vs actual
# 3. Each assertion checks ONE thing
# 4. Use these instead of raw [[ ]] comparisons for clarity
#
# USAGE
# -----
# Load both helpers and assertions:
#   load '../lib/helpers.bash'
#   load '../lib/assertions.bash'
#
# Then in tests:
#   @test "pad exists" {
#       assert_pad_exists "Meeting Notes" global
#   }
#
# AVAILABLE ASSERTIONS
# --------------------
# Existence:
#   assert_pad_exists <title> [scope]
#   assert_pad_not_exists <title> [scope]
#
# Status:
#   assert_pad_status <index> <expected_status> [scope]
#   assert_pad_completed <index> [scope]
#   assert_pad_planned <index> [scope]
#
# Properties:
#   assert_pad_pinned <index> [scope]
#   assert_pad_not_pinned <index> [scope]
#   assert_pad_has_tag <index> <tag> [scope]
#   assert_pad_has_tags <index> <tags...> [scope]
#
# Count:
#   assert_pad_count <expected_count> [scope]
#
# =============================================================================

# Source helpers if not already loaded
[[ -z "${PADZ_BIN:-}" ]] || true  # Ensure we have the env

# Assert a pad with given title exists
# Usage: assert_pad_exists <title> [scope]
assert_pad_exists() {
    local title="$1"
    local scope="${2:-}"
    if ! pad_exists "${title}" "${scope}"; then
        echo "FAIL: Expected pad '${title}' to exist in ${scope:-current} scope" >&2
        echo "Available pads:" >&2
        list_pads "${scope}" | jq -r '.pads[].pad.metadata.title' >&2
        return 1
    fi
}

# Assert a pad with given title does NOT exist
# Usage: assert_pad_not_exists <title> [scope]
assert_pad_not_exists() {
    local title="$1"
    local scope="${2:-}"
    if pad_exists "${title}" "${scope}"; then
        echo "FAIL: Expected pad '${title}' to NOT exist in ${scope:-current} scope" >&2
        return 1
    fi
}

# Assert pad has specific status
# Usage: assert_pad_status <index> <expected_status> [scope]
assert_pad_status() {
    local index="$1"
    local expected="$2"
    local scope="${3:-}"
    local actual
    actual=$(get_pad_status "${index}" "${scope}")
    if [[ "${actual}" != "${expected}" ]]; then
        echo "FAIL: Pad ${index} status: expected '${expected}', got '${actual}'" >&2
        return 1
    fi
}

# Assert pad is completed (status = Done)
# Usage: assert_pad_completed <index> [scope]
assert_pad_completed() {
    local index="$1"
    local scope="${2:-}"
    assert_pad_status "${index}" "Done" "${scope}"
}

# Assert pad is planned (status = Planned)
# Usage: assert_pad_planned <index> [scope]
assert_pad_planned() {
    local index="$1"
    local scope="${2:-}"
    assert_pad_status "${index}" "Planned" "${scope}"
}

# Assert pad is pinned
# Usage: assert_pad_pinned <index> [scope]
assert_pad_pinned() {
    local index="$1"
    local scope="${2:-}"
    local pinned
    pinned=$(get_pad_is_pinned "${index}" "${scope}")
    if [[ "${pinned}" != "true" ]]; then
        echo "FAIL: Expected pad ${index} to be pinned, but is_pinned=${pinned}" >&2
        return 1
    fi
}

# Assert pad is NOT pinned
# Usage: assert_pad_not_pinned <index> [scope]
assert_pad_not_pinned() {
    local index="$1"
    local scope="${2:-}"
    local pinned
    pinned=$(get_pad_is_pinned "${index}" "${scope}")
    if [[ "${pinned}" != "false" ]]; then
        echo "FAIL: Expected pad ${index} to NOT be pinned, but is_pinned=${pinned}" >&2
        return 1
    fi
}

# Assert pad has a specific tag
# Usage: assert_pad_has_tag <index> <tag> [scope]
assert_pad_has_tag() {
    local index="$1"
    local tag="$2"
    local scope="${3:-}"
    local tags
    tags=$(get_pad_tags "${index}" "${scope}")
    if [[ ! " ${tags} " =~ " ${tag} " ]]; then
        echo "FAIL: Expected pad ${index} to have tag '${tag}', but tags are: ${tags}" >&2
        return 1
    fi
}

# Assert pad has all specified tags
# Usage: assert_pad_has_tags <index> <scope> <tag1> [tag2...]
assert_pad_has_tags() {
    local index="$1"
    local scope="$2"
    shift 2
    local tag
    for tag in "$@"; do
        assert_pad_has_tag "${index}" "${tag}" "${scope}"
    done
}

# Assert pad count matches expected
# Usage: assert_pad_count <expected_count> [scope]
assert_pad_count() {
    local expected="$1"
    local scope="${2:-}"
    local actual
    actual=$(count_pads "${scope}")
    if [[ "${actual}" -ne "${expected}" ]]; then
        echo "FAIL: Expected ${expected} pads, got ${actual}" >&2
        return 1
    fi
}

# Assert command succeeds
# Usage: assert_success
# (Use after 'run' command in bats)
assert_success() {
    if [[ "${status}" -ne 0 ]]; then
        echo "FAIL: Command failed with status ${status}" >&2
        echo "Output: ${output}" >&2
        return 1
    fi
}

# Assert command fails
# Usage: assert_failure
assert_failure() {
    if [[ "${status}" -eq 0 ]]; then
        echo "FAIL: Expected command to fail, but it succeeded" >&2
        echo "Output: ${output}" >&2
        return 1
    fi
}

# Assert output contains string
# Usage: assert_output_contains <substring>
assert_output_contains() {
    local substring="$1"
    if [[ "${output}" != *"${substring}"* ]]; then
        echo "FAIL: Expected output to contain '${substring}'" >&2
        echo "Actual output: ${output}" >&2
        return 1
    fi
}

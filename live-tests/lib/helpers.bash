#!/usr/bin/env bash
# =============================================================================
# PADZ TEST HELPERS
# =============================================================================
#
# PURPOSE
# -------
# Provides utility functions for writing clean, focused bats tests.
# Uses JSON output (--output json) for reliable data extraction.
#
# DESIGN PRINCIPLES
# -----------------
# 1. Tests should assert on specific data, not string matching
# 2. JSON parsing via jq is more reliable than grep/awk
# 3. Small, composable functions that do one thing well
# 4. Helpers should be silent unless there's an error
#
# USAGE
# -----
# Load this in your .bats file:
#   load '../lib/helpers.bash'
#
# Then use functions like:
#   title=$(get_pad_title 1)
#   assert_pad_exists "Meeting Notes"
#
# AVAILABLE FUNCTIONS
# -------------------
# Pad Data Extraction:
#   get_pad_json <index> [scope]      - Get full JSON for a pad
#   get_pad_title <index> [scope]     - Get pad title
#   get_pad_status <index> [scope]    - Get pad status (Planned/Done)
#   get_pad_tags <index> [scope]      - Get pad tags as space-separated list
#   get_pad_id <index> [scope]        - Get pad UUID
#   get_pad_is_pinned <index> [scope] - Check if pad is pinned (true/false)
#   get_pad_is_deleted <index> [scope]- Check if deleted (true/false)
#
# Listing & Search:
#   list_pads [scope]                 - List all pads (JSON)
#   search_pads <term> [scope]        - Search pads by term
#   count_pads [scope]                - Count active pads
#
# Scope helpers:
#   Use "global" or "project" as scope, defaults to current directory
#
# =============================================================================

# Run padz with optional scope
# Usage: _padz [scope] <args...>
_padz() {
    local scope=""
    local args=()

    # Check if first arg is a scope indicator
    if [[ "$1" == "global" ]]; then
        scope="-g"
        shift
    elif [[ "$1" == "project" ]]; then
        # Ensure we're in project directory
        cd "${PROJECT_A}" 2>/dev/null || true
        shift
    fi

    "${PADZ_BIN}" ${scope} "$@"
}

# Get JSON output for all pads
# Usage: list_pads [scope]
list_pads() {
    local scope="${1:-}"
    _padz ${scope} list --output json 2>/dev/null
}

# Get JSON output for deleted pads
# Usage: list_deleted_pads [scope]
list_deleted_pads() {
    local scope="${1:-}"
    _padz ${scope} list --deleted --output json 2>/dev/null
}

# Search pads and return JSON
# Usage: search_pads <term> [scope]
search_pads() {
    local term="$1"
    local scope="${2:-}"
    _padz ${scope} search "${term}" --output json 2>/dev/null
}

# Count active pads
# Usage: count_pads [scope]
count_pads() {
    local scope="${1:-}"
    list_pads "${scope}" | jq '.pads | length'
}

# Get full JSON for a specific pad by index
# Usage: get_pad_json <index> [scope]
get_pad_json() {
    local index="$1"
    local scope="${2:-}"
    # Use first() to ensure we only get one result (in case of duplicates)
    list_pads "${scope}" | jq --arg idx "${index}" 'first(.pads[] | select(.index.value == ($idx | tonumber)))'
}

# Get pad title by index
# Usage: get_pad_title <index> [scope]
get_pad_title() {
    local index="$1"
    local scope="${2:-}"
    get_pad_json "${index}" "${scope}" | jq -r '.pad.metadata.title'
}

# Get pad status by index
# Usage: get_pad_status <index> [scope]
get_pad_status() {
    local index="$1"
    local scope="${2:-}"
    get_pad_json "${index}" "${scope}" | jq -r '.pad.metadata.status'
}

# Get pad tags as space-separated list
# Usage: get_pad_tags <index> [scope]
get_pad_tags() {
    local index="$1"
    local scope="${2:-}"
    get_pad_json "${index}" "${scope}" | jq -r '.pad.metadata.tags | join(" ")'
}

# Get pad UUID
# Usage: get_pad_id <index> [scope]
get_pad_id() {
    local index="$1"
    local scope="${2:-}"
    get_pad_json "${index}" "${scope}" | jq -r '.pad.metadata.id'
}

# Check if pad is pinned
# Usage: get_pad_is_pinned <index> [scope]
get_pad_is_pinned() {
    local index="$1"
    local scope="${2:-}"
    get_pad_json "${index}" "${scope}" | jq -r '.pad.metadata.is_pinned'
}

# Check if pad is deleted
# Usage: get_pad_is_deleted <index> [scope]
get_pad_is_deleted() {
    local index="$1"
    local scope="${2:-}"
    list_deleted_pads "${scope}" | jq --arg idx "${index}" '.pads[] | select(.index.value == ($idx | tonumber)) | .pad.metadata.is_deleted' | head -1
}

# Get pad content (body text)
# Usage: get_pad_content <index> [scope]
get_pad_content() {
    local index="$1"
    local scope="${2:-}"
    get_pad_json "${index}" "${scope}" | jq -r '.pad.content'
}

# Find pad index by title (exact match)
# Returns empty if not found, fails if multiple matches
# Usage: find_pad_by_title <title> [scope]
find_pad_by_title() {
    local title="$1"
    local scope="${2:-}"
    local result
    # Use jq array to get all matches, then check count
    result=$(list_pads "${scope}" | jq -r --arg t "${title}" '[.pads[] | select(.pad.metadata.title == $t) | .index.value] | if length > 1 then "MULTIPLE" elif length == 0 then "" else .[0] | tostring end')
    if [[ "${result}" == "MULTIPLE" ]]; then
        echo "ERROR: Multiple pads found with title '${title}'" >&2
        return 1
    fi
    echo "${result}"
}

# Check if a pad with given title exists
# Usage: pad_exists <title> [scope]
pad_exists() {
    local title="$1"
    local scope="${2:-}"
    local index
    index=$(find_pad_by_title "${title}" "${scope}")
    [[ -n "${index}" ]]
}

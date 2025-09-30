#!/usr/bin/env bash

# Assertion helpers for padz E2E tests

# Assert that a scratch is pinned
assert_scratch_is_pinned() {
    local scope="$1"
    local scratch_id="$2"
    
    local scratch_json
    scratch_json=$(get_scratch_by_index "${scratch_id}")
    
    local is_pinned
    is_pinned=$(echo "${scratch_json}" | jq -r '.is_pinned // false')
    
    if [[ "${is_pinned}" != "true" ]]; then
        echo "Expected scratch ${scratch_id} to be pinned, but it's not" >&2
        return 1
    fi
}

# Assert that a scratch is not pinned
assert_scratch_is_not_pinned() {
    local scope="$1"
    local scratch_id="$2"
    
    local scratch_json
    scratch_json=$(get_scratch_by_index "${scratch_id}")
    
    local is_pinned
    is_pinned=$(echo "${scratch_json}" | jq -r '.is_pinned // false')
    
    if [[ "${is_pinned}" == "true" ]]; then
        echo "Expected scratch ${scratch_id} to not be pinned, but it is" >&2
        return 1
    fi
}

# Assert that a scratch is active (not deleted)
assert_scratch_is_active() {
    local scope="$1"
    local scratch_id="$2"
    
    local scratch_json
    scratch_json=$(get_scratch_by_index "${scratch_id}")
    
    if [[ -z "${scratch_json}" || "${scratch_json}" == "null" ]]; then
        echo "Expected scratch ${scratch_id} to be active, but it's missing" >&2
        return 1
    fi
    
    local is_deleted
    is_deleted=$(echo "${scratch_json}" | jq -r '.is_deleted // false')
    
    if [[ "${is_deleted}" == "true" ]]; then
        echo "Expected scratch ${scratch_id} to be active, but it's deleted" >&2
        return 1
    fi
}

# Assert that a scratch is deleted
assert_scratch_is_deleted() {
    local scope="$1" 
    local scratch_id="$2"
    
    # Check deleted items with --include-deleted flag
    if [[ "${scope}" == "global" ]]; then
        run_padz list --global --include-deleted
    else
        run_padz list --include-deleted
    fi
    
    if [[ "${status}" -ne 0 ]]; then
        echo "Failed to get list with deleted items" >&2
        return 1
    fi
    
    local scratch_json
    scratch_json=$(echo "${output}" | jq -r ".[$((scratch_id-1))]")
    
    if [[ -z "${scratch_json}" || "${scratch_json}" == "null" ]]; then
        echo "Expected scratch ${scratch_id} to exist as deleted, but it's missing" >&2
        return 1
    fi
    
    local is_deleted
    is_deleted=$(echo "${scratch_json}" | jq -r '.is_deleted // false')
    
    if [[ "${is_deleted}" != "true" ]]; then
        echo "Expected scratch ${scratch_id} to be deleted, but it's not" >&2
        return 1
    fi
}

# Assert that a scratch is missing (hard deleted)
assert_scratch_is_missing() {
    local scope="$1"
    local scratch_id="$2"
    
    # Check with --include-deleted to ensure it's completely gone
    if [[ "${scope}" == "global" ]]; then
        run_padz list --global --include-deleted
    else
        run_padz list --include-deleted
    fi
    
    if [[ "${status}" -ne 0 ]]; then
        echo "Failed to get list with deleted items" >&2
        return 1
    fi
    
    local scratch_count
    scratch_count=$(echo "${output}" | jq -r 'length')
    
    if [[ "${scratch_id}" -le "${scratch_count}" ]]; then
        local scratch_json
        scratch_json=$(echo "${output}" | jq -r ".[$((scratch_id-1))]")
        if [[ -n "${scratch_json}" && "${scratch_json}" != "null" ]]; then
            echo "Expected scratch ${scratch_id} to be missing, but it still exists" >&2
            return 1
        fi
    fi
}

# Assert scratch has specific title
assert_scratch_title() {
    local scope="$1"
    local scratch_id="$2"
    local expected_title="$3"
    
    local scratch_json
    scratch_json=$(get_scratch_by_index "${scratch_id}")
    
    local actual_title
    actual_title=$(echo "${scratch_json}" | jq -r '.title')
    
    if [[ "${actual_title}" != "${expected_title}" ]]; then
        echo "Expected scratch ${scratch_id} title to be '${expected_title}', but got '${actual_title}'" >&2
        return 1
    fi
}

# Assert scratch has specific content
assert_scratch_content() {
    local scope="$1"
    local scratch_id="$2" 
    local expected_content="$3"
    
    local scratch_json
    scratch_json=$(get_scratch_by_index "${scratch_id}")
    
    local actual_content
    actual_content=$(echo "${scratch_json}" | jq -r '.content')
    
    if [[ "${actual_content}" != "${expected_content}" ]]; then
        echo "Expected scratch ${scratch_id} content to be '${expected_content}', but got '${actual_content}'" >&2
        return 1
    fi
}

# Assert list includes specific scratch IDs
assert_list_includes() {
    local scope="$1"
    local use_all="$2"
    local pinned="$3"
    shift 3
    local expected_ids=("$@")
    
    # Build command flags
    local flags=()
    if [[ "${scope}" == "global" ]]; then
        flags+=(--global)
    fi
    if [[ "${use_all}" == "true" ]]; then
        flags+=(--include-deleted)
    fi
    
    run_padz list "${flags[@]}"
    
    if [[ "${status}" -ne 0 ]]; then
        echo "Failed to get list" >&2
        return 1
    fi
    
    for expected_id in "${expected_ids[@]}"; do
        local scratch_json
        scratch_json=$(echo "${output}" | jq -r ".[$((expected_id-1))]")
        
        if [[ -z "${scratch_json}" || "${scratch_json}" == "null" ]]; then
            echo "Expected list to include scratch ${expected_id}, but it's missing" >&2
            return 1
        fi
        
        # If checking only pinned, verify it's pinned
        if [[ "${pinned}" == "true" ]]; then
            local is_pinned
            is_pinned=$(echo "${scratch_json}" | jq -r '.is_pinned // false')
            if [[ "${is_pinned}" != "true" ]]; then
                echo "Expected scratch ${expected_id} to be pinned in pinned list" >&2
                return 1
            fi
        fi
    done
}

# Assert command output is valid JSON
assert_valid_json() {
    echo "${output}" | jq . > /dev/null 2>&1
    local result=$?
    if [[ "${result}" -ne 0 ]]; then
        echo "Expected valid JSON output, but got:" >&2
        echo "${output}" >&2
        return 1
    fi
}

# Assert command succeeded
assert_success() {
    if [[ "${status}" -ne 0 ]]; then
        echo "Expected command to succeed, but it failed with status ${status}" >&2
        echo "Output: ${output}" >&2
        return 1
    fi
}

# Assert command failed
assert_failure() {
    if [[ "${status}" -eq 0 ]]; then
        echo "Expected command to fail, but it succeeded" >&2
        echo "Output: ${output}" >&2
        return 1
    fi
}
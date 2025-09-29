#!/usr/bin/env bash

# Test helper functions for padz E2E tests

# Set up each test with a clean environment
setup_test() {
    # Create a temporary test environment
    TEST_ENV_DIR="$(mktemp -d)"
    export TEST_ENV_DIR
    
    # Create isolated directories
    export TEST_HOME="${TEST_ENV_DIR}/home"
    export TEST_XDG_DATA_HOME="${TEST_ENV_DIR}/data" 
    export TEST_PROJECT_DIR="${TEST_ENV_DIR}/project"
    
    mkdir -p "${TEST_HOME}" "${TEST_XDG_DATA_HOME}" "${TEST_PROJECT_DIR}"
    
    # Change to project directory
    cd "${TEST_PROJECT_DIR}"
    
    # Build fresh padz binary if needed
    if [[ ! -f "${PADZ_BIN}" ]]; then
        echo "Building padz binary..." >&2
        (cd "${PROJECT_ROOT}" && go build -o "${PADZ_BIN}" ./cmd/padz)
    fi
}

# Clean up test environment
teardown_test() {
    if [[ -n "${TEST_ENV_DIR}" && -d "${TEST_ENV_DIR}" ]]; then
        rm -rf "${TEST_ENV_DIR}"
    fi
}

# Run padz command in isolated environment
run_padz() {
    HOME="${TEST_HOME}" \
    XDG_DATA_HOME="${TEST_XDG_DATA_HOME}" \
    PADZ_FORMAT=json \
    run "${PADZ_BIN}" "$@"
}

# Run padz command with specific format
run_padz_format() {
    local format="$1"
    shift
    HOME="${TEST_HOME}" \
    XDG_DATA_HOME="${TEST_XDG_DATA_HOME}" \
    PADZ_FORMAT="${format}" \
    run "${PADZ_BIN}" "$@"
}

# Run padz command expecting success
run_padz_success() {
    run_padz "$@"
    if [[ "${status}" -ne 0 ]]; then
        echo "Command failed: padz $*" >&2
        echo "Status: ${status}" >&2
        echo "Output: ${output}" >&2
        return 1
    fi
}

# Parse JSON output and extract field
json_get() {
    local json="$1"
    local field="$2"
    echo "${json}" | jq -r "${field}"
}

# Get scratch count from list output
get_scratch_count() {
    run_padz list
    if [[ "${status}" -eq 0 ]]; then
        echo "${output}" | jq -r 'length'
    else
        echo "0"
    fi
}

# Get scratch by index from list
get_scratch_by_index() {
    local index="$1"
    run_padz list
    if [[ "${status}" -eq 0 ]]; then
        echo "${output}" | jq -r ".[$((index-1))]"
    fi
}

# Check if scratch has specific property value
scratch_has_property() {
    local scratch_json="$1"
    local property="$2"
    local expected_value="$3"
    
    local actual_value
    actual_value=$(echo "${scratch_json}" | jq -r ".${property}")
    [[ "${actual_value}" == "${expected_value}" ]]
}
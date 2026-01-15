#!/usr/bin/env bash
# =============================================================================
# PADZ TEST BACKDOORS
# =============================================================================
#
# PURPOSE
# -------
# Provides low-level functions to manipulate the padz data store directly,
# bypassing the normal API. Use these to set up edge cases like:
#   - Orphan files (content without metadata)
#   - Zombie entries (metadata without content)
#   - Corrupted state that doctor should fix
#
# WARNING
# -------
# These functions directly manipulate the filesystem. They're for testing
# recovery/doctor scenarios, not normal operations.
#
# FILE FORMAT
# -----------
# Content files: pad-{uuid}.txt (not .md!)
# Index file: data.json (HashMap<Uuid, Metadata>, not array)
#
# USAGE
# -----
# Load in your .bats file:
#   load '../lib/backdoors.bash'
#
# Example: Test that doctor recovers orphan files
#   @test "doctor recovers orphan" {
#       backdoor_remove_metadata "$(get_pad_id 1 global)" global
#       run padz -g doctor
#       assert_success
#       assert_output_contains "recovered"
#   }
#
# =============================================================================

# Get the data directory for a scope
# Usage: _get_data_dir [scope]
_get_data_dir() {
    local scope="${1:-}"
    if [[ "${scope}" == "global" ]]; then
        echo "${PADZ_GLOBAL_DATA}"
    else
        # Project scope - data is in .padz under project root
        echo "${PROJECT_A}/.padz"
    fi
}

# Get path to content file for a pad
# Note: Files are named pad-{uuid}.txt
# Usage: backdoor_get_content_path <uuid> [scope]
backdoor_get_content_path() {
    local uuid="$1"
    local scope="${2:-}"
    local data_dir
    data_dir=$(_get_data_dir "${scope}")
    echo "${data_dir}/pad-${uuid}.txt"
}

# Get path to data.json
# Usage: backdoor_get_index_path [scope]
backdoor_get_index_path() {
    local scope="${1:-}"
    local data_dir
    data_dir=$(_get_data_dir "${scope}")
    echo "${data_dir}/data.json"
}

# Remove content file only (creates zombie metadata)
# Usage: backdoor_remove_content <uuid> [scope]
backdoor_remove_content() {
    local uuid="$1"
    local scope="${2:-}"
    local content_path
    content_path=$(backdoor_get_content_path "${uuid}" "${scope}")
    rm -f "${content_path}"
}

# Remove metadata entry only (creates orphan file)
# Note: data.json is HashMap<Uuid, Metadata>, so we delete by key
# Usage: backdoor_remove_metadata <uuid> [scope]
backdoor_remove_metadata() {
    local uuid="$1"
    local scope="${2:-}"
    local index_path
    index_path=$(backdoor_get_index_path "${scope}")

    if [[ -f "${index_path}" ]]; then
        local temp_file="${index_path}.tmp"
        # data.json is {uuid: metadata, ...} so delete by key
        jq --arg id "${uuid}" 'del(.[$id])' "${index_path}" > "${temp_file}"
        mv "${temp_file}" "${index_path}"
    fi
}

# Write invalid JSON to index (for corruption testing)
# Usage: backdoor_corrupt_index [scope]
backdoor_corrupt_index() {
    local scope="${1:-}"
    local index_path
    index_path=$(backdoor_get_index_path "${scope}")
    echo "{ invalid json here" > "${index_path}"
}

# Create an orphan content file (no metadata)
# Usage: backdoor_create_orphan <title> [scope]
# Returns: the UUID of the created orphan
backdoor_create_orphan() {
    local title="$1"
    local scope="${2:-}"
    local data_dir
    data_dir=$(_get_data_dir "${scope}")

    # Generate a UUID (lowercase)
    local uuid
    uuid=$(uuidgen | tr '[:upper:]' '[:lower:]')

    # Create content file with correct naming convention
    mkdir -p "${data_dir}"
    echo -e "${title}\n\nOrphan content body" > "${data_dir}/pad-${uuid}.txt"

    echo "${uuid}"
}

# List all content files in data directory
# Usage: backdoor_list_content_files [scope]
backdoor_list_content_files() {
    local scope="${1:-}"
    local data_dir
    data_dir=$(_get_data_dir "${scope}")
    ls "${data_dir}"/pad-*.txt 2>/dev/null | xargs -n1 basename | sed 's/^pad-//; s/\.txt$//' || true
}

# Check if content file exists for UUID
# Usage: backdoor_content_exists <uuid> [scope]
backdoor_content_exists() {
    local uuid="$1"
    local scope="${2:-}"
    local content_path
    content_path=$(backdoor_get_content_path "${uuid}" "${scope}")
    [[ -f "${content_path}" ]]
}

# Check if metadata exists for UUID
# Note: data.json is {uuid: metadata, ...}
# Usage: backdoor_metadata_exists <uuid> [scope]
backdoor_metadata_exists() {
    local uuid="$1"
    local scope="${2:-}"
    local index_path
    index_path=$(backdoor_get_index_path "${scope}")

    if [[ ! -f "${index_path}" ]]; then
        return 1
    fi

    # Check if key exists in the object
    local has_key
    has_key=$(jq --arg id "${uuid}" 'has($id)' "${index_path}")
    [[ "${has_key}" == "true" ]]
}

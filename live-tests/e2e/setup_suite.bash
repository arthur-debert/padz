#!/usr/bin/env bash

# This file is sourced by BATS before running the test suite
# It sets up the environment for all tests

setup_suite() {
    # Get the directory of this script
    E2E_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    LIVE_TESTS_DIR="$(cd "${E2E_DIR}/.." && pwd)"
    PROJECT_ROOT="$(cd "${LIVE_TESTS_DIR}/.." && pwd)"

    # Export important paths
    export E2E_DIR
    export LIVE_TESTS_DIR  
    export PROJECT_ROOT

    # Ensure we use JSON format for all tests
    export PADZ_FORMAT=json

    # Set up paths for the padz binary
    export PADZ_BIN="${PROJECT_ROOT}/bin/padz"

    echo "🧪 E2E Test Suite Setup"
    echo "📁 Project Root: ${PROJECT_ROOT}"
    echo "📁 Live Tests Dir: ${LIVE_TESTS_DIR}"
    echo "📁 E2E Dir: ${E2E_DIR}"
    echo "🔧 PADZ_FORMAT: ${PADZ_FORMAT}"
    echo "🎯 PADZ_BIN: ${PADZ_BIN}"
}
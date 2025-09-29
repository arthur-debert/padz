#!/usr/bin/env bash

set -euo pipefail

# Get the directory of this script
E2E_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${E2E_DIR}/../.." && pwd)"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🧪 Padz E2E Test Suite${NC}"
echo -e "${BLUE}📁 Project Root: ${PROJECT_ROOT}${NC}"
echo -e "${BLUE}📁 E2E Directory: ${E2E_DIR}${NC}"
echo ""

# Build padz binary
echo -e "${YELLOW}🔨 Building padz binary...${NC}"
cd "${PROJECT_ROOT}"
mkdir -p bin
go build -o bin/padz ./cmd/padz
echo -e "${GREEN}✅ Binary built successfully${NC}"
echo ""

# Default output format
OUTPUT_FORMAT="tap"
OUTPUT_FILE=""
TEST_FILES=()

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --junit)
            OUTPUT_FORMAT="junit"
            shift
            ;;
        --tap)
            OUTPUT_FORMAT="tap"
            shift
            ;;
        --output|-o)
            OUTPUT_FILE="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS] [TEST_FILES...]"
            echo ""
            echo "Options:"
            echo "  --junit         Output in JUnit XML format"
            echo "  --tap          Output in TAP format (default)"
            echo "  --output, -o   Output file (default: stdout)"
            echo "  --help, -h     Show this help"
            echo ""
            echo "Examples:"
            echo "  $0                              # Run all tests with TAP output"
            echo "  $0 --junit -o results.xml      # Run all tests with JUnit output to file"
            echo "  $0 01-create.bats              # Run specific test file"
            exit 0
            ;;
        *)
            # Assume it's a test file
            TEST_FILES+=("$1")
            shift
            ;;
    esac
done

# If no test files specified, find all .bats files
if [[ ${#TEST_FILES[@]} -eq 0 ]]; then
    readarray -t TEST_FILES < <(find "${E2E_DIR}" -name "*.bats" | sort)
else
    # Convert relative paths to absolute paths
    for i in "${!TEST_FILES[@]}"; do
        if [[ ! "${TEST_FILES[i]}" =~ ^/ ]]; then
            TEST_FILES[i]="${E2E_DIR}/${TEST_FILES[i]}"
        fi
    done
fi

# Check if any test files found
if [[ ${#TEST_FILES[@]} -eq 0 ]]; then
    echo -e "${RED}❌ No test files found${NC}"
    exit 1
fi

echo -e "${YELLOW}🎯 Running ${#TEST_FILES[@]} test file(s)...${NC}"
for file in "${TEST_FILES[@]}"; do
    echo -e "${BLUE}   - $(basename "${file}")${NC}"
done
echo ""

# Build BATS command
BATS_CMD=(bats)

# Add output format
case "${OUTPUT_FORMAT}" in
    junit)
        BATS_CMD+=(--formatter junit)
        ;;
    tap)
        BATS_CMD+=(--formatter tap)
        ;;
esac

# Add output file if specified
if [[ -n "${OUTPUT_FILE}" ]]; then
    BATS_CMD+=(--output "${OUTPUT_FILE}")
fi

# Add test files
BATS_CMD+=("${TEST_FILES[@]}")

# Run tests
echo -e "${YELLOW}🚀 Executing tests...${NC}"
echo -e "${BLUE}Command: ${BATS_CMD[*]}${NC}"
echo ""

# Change to E2E directory to run tests
cd "${E2E_DIR}"

if "${BATS_CMD[@]}"; then
    echo ""
    echo -e "${GREEN}✅ All tests passed!${NC}"
    exit 0
else
    echo ""
    echo -e "${RED}❌ Some tests failed${NC}"
    exit 1
fi
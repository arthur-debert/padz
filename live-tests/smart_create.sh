#!/bin/bash
set -e

echo "--- Starting Smart Create Tests ---"

echo "1. Testing Piped Input..."
echo -e "Piped Title\n\nPiped Body" | padz create

echo "Listing pads to verify Piped Input..."
padz list | grep "Piped Title" || exit 1
padz view 1 | grep "Piped Body" || exit 1

echo "PASS: Piped input test"

echo "2. Testing Clipboard..."

mkdir -p ./bin

# Create mock pbpaste using checking single line writes
echo '#!/bin/sh' > ./bin/pbpaste
echo 'echo "Clipboard Title"' >> ./bin/pbpaste
echo 'echo ""' >> ./bin/pbpaste
echo 'echo "Clipboard Body"' >> ./bin/pbpaste
chmod +x ./bin/pbpaste

# Create fake editor to avoid blocking
echo '#!/bin/sh' > ./bin/fake-editor
chmod +x ./bin/fake-editor

# Add to PATH and set EDITOR
export PATH="$(pwd)/bin:$PATH"
export EDITOR="fake-editor"

echo "Running padz create (from clipboard)..."
# Must redirect stdin from /dev/null to prevent padz from reading the rest of this script
# because the test runner feeds the script via stdin/redirection.
padz create </dev/null

echo "Listing pads to verify Clipboard Input..."
padz list | grep "Clipboard Title" || exit 1
# Assuming newest is 1
padz view 1 | grep "Clipboard Body" || exit 1

echo "PASS: Clipboard test"
echo "--- All Tests Passed ---"

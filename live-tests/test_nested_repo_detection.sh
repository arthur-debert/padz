#!/usr/bin/env zsh
# Test nested repo detection: child repo should use parent's .padz

set -e

echo "=== Testing Nested Repo Detection ==="

# The live-tests/run script creates a git repo in the temp dir.
# We need to run `padz init` to create the .padz directory.

echo ""
echo "Step 0: Initialize padz in parent repo"
# Manually create .padz since the test dir is inside the real padz repo
# and `padz init` would find the parent's .padz
mkdir -p .padz
echo "Initialized .padz in parent repo"

echo ""
echo "Step 1: Create a pad in parent repo"
padz n --no-editor "Parent Pad"
padz ls
echo "Created pad in parent repo"

echo ""
echo "Step 2: Create child repo (git only, no .padz)"
mkdir -p child-repo
cd child-repo
git init --quiet
echo "Created child-repo with .git but no .padz"

echo ""
echo "Step 3: Verify from child repo we can see parent's pads"
padz ls

PAD_COUNT=$(padz ls 2>/dev/null | grep -c "Parent Pad" || echo 0)
[[ "$PAD_COUNT" -eq "1" ]] && echo "SUCCESS: Child repo sees parent's pad" || { echo "FAILURE: Child repo cannot see parent's pad (count=$PAD_COUNT)"; exit 1; }

echo ""
echo "Step 4: Create a pad from child repo (should go to parent's .padz)"
padz n --no-editor "Child Pad"
padz ls

PAD_COUNT=$(padz ls 2>/dev/null | grep -c "Pad" || echo 0)
[[ "$PAD_COUNT" -eq "2" ]] && echo "SUCCESS: Pads created from child go to parent's .padz" || { echo "FAILURE: Expected 2 pads, got $PAD_COUNT"; exit 1; }

echo ""
echo "Step 5: Verify pads are stored in parent's .padz, not child"
[[ ! -d ".padz" ]] && echo "SUCCESS: No .padz in child repo" || { echo "FAILURE: .padz directory was created in child repo"; exit 1; }

cd ..
[[ -d ".padz" ]] && echo "SUCCESS: .padz exists in parent repo" || { echo "FAILURE: .padz not found in parent repo"; exit 1; }

echo ""
echo "Step 6: Test deeply nested repos"
mkdir -p child-repo/grandchild-repo
cd child-repo/grandchild-repo
git init --quiet
echo "Created grandchild-repo with .git"

padz n --no-editor "Grandchild Pad"
PAD_COUNT=$(padz ls 2>/dev/null | grep -c "Pad" || echo 0)
[[ "$PAD_COUNT" -eq "3" ]] && echo "SUCCESS: Deeply nested repos also use ancestor's .padz" || { echo "FAILURE: Expected 3 pads from grandchild, got $PAD_COUNT"; exit 1; }

cd ../..

echo ""
echo "Step 7: Test child repo with its own .padz takes precedence"
mkdir -p independent-child
cd independent-child
git init --quiet
mkdir -p .padz

padz n --no-editor "Independent Pad"
INDEPENDENT_COUNT=$(padz ls 2>/dev/null | grep -c "Independent Pad" || echo 0)
TOTAL_COUNT=$(padz ls 2>/dev/null | grep -c "Pad" || echo 0)

[[ "$INDEPENDENT_COUNT" -eq "1" && "$TOTAL_COUNT" -eq "1" ]] && echo "SUCCESS: Child with own .padz is independent" || { echo "FAILURE: Expected only 1 independent pad, got total=$TOTAL_COUNT independent=$INDEPENDENT_COUNT"; exit 1; }

cd ..

echo ""
echo "=== All nested repo detection tests PASSED ==="

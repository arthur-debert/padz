#!/bin/bash
set -e

echo "=== Testing Complex Consecutive Deletions Issue ==="

# Create more scratches to test edge cases
echo "Creating 5 scratches..."
for i in {1..5}; do
    echo "Content $i" | padz create "Test $i"
done

echo ""
echo "Initial list:"
padz list

echo ""
echo "=== Testing consecutive deletions by index ==="

echo ""
echo "Deleting scratch 3 (middle item)..."
padz delete 3
echo "List after deleting 3:"
padz list

echo ""
echo "Deleting scratch 2 (should now be what was originally scratch 2)..."
set +e
padz delete 2
result1=$?
set -e
echo "Result: $result1"
echo "List after deleting 2:"
padz list

echo ""
echo "Deleting scratch 1 (newest remaining)..."
set +e
padz delete 1
result2=$?
set -e
echo "Result: $result2"
echo "List after deleting 1:"
padz list

echo ""
echo "=== Testing deletions by UUID ==="

# Get UUIDs from the list
echo ""
echo "Getting UUIDs for remaining scratches..."
padz list --output json | jq -r '.[].id' > /tmp/uuids.txt

echo "UUIDs found:"
cat /tmp/uuids.txt

echo ""
echo "Trying to delete by UUID..."
set +e
while read uuid; do
    echo "Deleting UUID: $uuid"
    padz delete "$uuid"
    echo "Result: $?"
    echo "Remaining scratches:"
    padz list
    echo ""
done < /tmp/uuids.txt
set -e

echo ""
echo "Final list:"
padz list

echo ""
echo "=== Testing mixed ID formats ==="

# Create a few more scratches
echo "Creating more scratches for mixed ID test..."
echo "Mixed content 1" | padz create "Mixed Test 1"
echo "Mixed content 2" | padz create "Mixed Test 2"

echo ""
echo "List with new scratches:"
padz list

# Try to get both numeric and UUID IDs
echo ""
echo "Getting mixed ID formats..."
padz list --output json | jq -r '.[] | {index: (.id | split("-")[0]), id: .id, title: .title}' | head -2

echo ""
echo "Attempting rapid consecutive deletions..."
for i in {1..2}; do
    echo "Rapid delete attempt $i..."
    set +e
    padz delete $i
    echo "Result: $?"
    set -e
    sleep 0.1  # Small delay to see if timing matters
done

echo ""
echo "Final state:"
padz list
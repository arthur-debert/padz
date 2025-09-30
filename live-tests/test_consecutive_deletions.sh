#!/bin/bash
set -e

echo "=== Testing Consecutive Deletions Issue ==="

echo "Creating first scratch..."
echo "Content 1" | padz create "Test 1"

echo "Creating second scratch..."
echo "Content 2" | padz create "Test 2" 

echo "Creating third scratch..."
echo "Content 3" | padz create "Test 3"

echo ""
echo "Initial list:"
padz list

echo ""
echo "Deleting scratch 1..."
padz delete 1
echo "Result: $?"

echo ""
echo "List after first delete:"
padz list

echo ""
echo "Deleting scratch 2..."
set +e  # Allow this to fail
padz delete 2
delete_result=$?
set -e
echo "Result: $delete_result"

echo ""
echo "List after second delete attempt:"
padz list

if [ $delete_result -ne 0 ]; then
    echo ""
    echo "ERROR: Second deletion failed! This indicates the consecutive deletion bug."
    echo "Let's try deleting by the new index:"
    set +e
    padz delete 1  # After first deletion, what was #2 becomes #1
    padz delete 1  # What was #3 becomes #1
    set -e
fi

echo ""
echo "Final list:"
padz list
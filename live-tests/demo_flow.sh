# Demo Flow: Create, Pin, Delete, List

echo "--- Creating 3 pads ---"
padz n "First Note" "Content of first note"
padz n "Second Note" "Content of second note"
padz n "Third Note" "Content of third note"

echo "\n--- Listing all pads (Creation Order) ---"
padz ls

echo "\n--- Pinning the Third Note (Index 3) ---"
padz p 3

echo "\n--- Listing pads (Pinned should be first) ---"
padz ls

echo "\n--- Deleting the First Note (Index 1) ---"
# Note: After pinning 3, the list order might be: p1(Third), 1(First), 2(Second).
# We need to be careful about indexes.
# The user uses DISPLAY indexes.
# If I pin 3, it becomes p1. 
# The remaining are 1(First) and 2(Second).
# So I delete 1.
padz rm 1

echo "\n--- Listing pads (First note gone, Third pinned, Second remains) ---"
padz ls

echo "\n--- Listing ALL pads (including deleted) ---"
padz ls --deleted

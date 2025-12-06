# Demo Flow: Create, Pin, Delete, List

echo "--- Creating 3 pads ---"
padz n --no-editor "First Note" "Content of first note"
padz n --no-editor "Second Note" "Content of second note"
padz n --no-editor "Third Note" "Content of third note"

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

echo "\n--- Purging the deleted note ---"
echo "Y" | padz purge

echo "\n--- Listing deleted pads (Should be empty) ---"
padz ls --deleted

echo "\n--- Creating a file for Import test ---"
echo "Imported Title\nImported Content" > import_test.txt

echo "\n--- Importing the file ---"
padz import import_test.txt

echo "\n--- Listing pads (Should include Imported Title) ---"
padz ls

echo "\n--- Exporting pads ---"
padz export

echo "\n--- Verifying Export ---"
ls -lh padz-*.tar.gz
# Verify archive content contains the imported pad (filename matches sanitized title)
tar -tzf padz-*.tar.gz | grep "Imported Title"

echo "\n--- Doctor Test: Creating inconsistency ---"
# Create consistency by writing a file manually that isn't in DB
echo "Orphan Pad\nOrphan Content" > .padz/pad-aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.txt

echo "\n--- Running Doctor ---"
padz doctor

echo "\n--- Listing pads (Should include Orphan Pad) ---"
padz ls

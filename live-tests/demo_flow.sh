#!/usr/bin/env zsh
# shellcheck shell=bash
# Demo Flow: Create, Pin, Delete, List

echo "--- Creating 3 pads ---"
padz n --no-editor "First Note" "Content of first note"
padz n --no-editor "Second Note" "Content of second note"
padz n --no-editor "Third Note" "Content of third note"

printf '\n--- Listing all pads (Creation Order) ---\n'
padz ls

printf '\n--- Pinning the Third Note (Index 3) ---\n'
padz p 3

printf '\n--- Listing pads (Pinned should be first) ---\n'
padz ls

printf '\n--- Deleting the First Note (Index 1) ---\n'
# Note: After pinning 3, the list order might be: p1(Third), 1(First), 2(Second).
# We need to be careful about indexes.
# The user uses DISPLAY indexes.
# If I pin 3, it becomes p1.
# The remaining are 1(First) and 2(Second).
# So I delete 1.
padz rm 1

printf '\n--- Listing pads (First note gone, Third pinned, Second remains) ---\n'
padz ls

printf '\n--- Listing ALL pads (including deleted) ---\n'
padz ls --deleted

printf '\n--- Purging the deleted note ---\n'
echo "Y" | padz purge

printf '\n--- Listing deleted pads (Should be empty) ---\n'
padz ls --deleted

printf '\n--- Creating a file for Import test ---\n'
printf 'Imported Title\nImported Content\n' > import_test.txt

printf '\n--- Importing the file ---\n'
padz import import_test.txt

printf '\n--- Listing pads (Should include Imported Title) ---\n'
padz ls

printf '\n--- Exporting pads ---\n'
padz export

printf '\n--- Verifying Export ---\n'
ls -lh padz-*.tar.gz
# Verify archive content contains the imported pad (filename matches sanitized title)
tar -tzf padz-*.tar.gz | grep "Imported Title"

printf '\n--- Doctor Test: Creating inconsistency ---\n'
# Create consistency by writing a file manually that isn't in DB
printf 'Orphan Pad\nOrphan Content\n' > .padz/pad-aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.txt

printf '\n--- Running Doctor ---\n'
padz doctor

printf '\n--- Listing pads (Should include Orphan Pad) ---\n'
padz ls

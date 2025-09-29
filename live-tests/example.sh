#!/bin/zsh

# Create some scratches
echo -e "Groceries:\n- Milk\n- Bread\n- Eggs" | padz new "Shopping"
padz list

echo "\nCreating project-specific scratch..."
echo -e "- Setup development environment\n- Write unit tests\n- Update documentation" | padz new "Project TODO"

echo "\nListing all scratches:"
padz list

echo "\nViewing the shopping list:"
padz view 1

echo "\nPinning the shopping list:"
padz pin 1
padz list

# this shell exports the function export_history, which will export, sans
# line numbers, the history of commands run in this shell, useful if you
# want to save the commands you ran to a file as a script to replay later

#!/bin/zsh
# Example demonstrating naked execution features

echo "=== Naked Execution Demo ==="
echo ""

echo "1. List todos (no command needed):"
too

echo -e "\n2. Add a todo without 'add' command:"
too Buy groceries

echo -e "\n3. Add another todo:"
too Pack for trip

echo -e "\n4. Add sub-todos using --to flag:"
too --to 1 Milk
too --to 1 Bread
too --to 2 Camera
too --to 2 Passport

echo -e "\n5. Current list:"
too

echo -e "\n6. Complete a todo (still needs command):"
too complete 1.1

echo -e "\n7. Final list:"
too

echo -e "\nNote: naked execution makes 'too' even faster to use!"
echo "- 'too' alone lists todos"
echo "- 'too <text>' adds a new todo"
echo "- Other commands still work normally"
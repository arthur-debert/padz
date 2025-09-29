#!/bin/zsh

echo "Testing padz isolation..."
echo

echo "1. Testing global list (should be empty):"
padz list --global

echo
echo "2. Testing project list (should be empty):"
padz list

echo
echo "3. Creating a test scratch:"
echo "This is a test scratch" | padz new "Test Scratch"

echo
echo "4. Listing after creation:"
padz list

echo
echo "5. Checking environment variables:"
echo "HOME: $HOME"
echo "XDG_DATA_HOME: $XDG_DATA_HOME"
echo "PWD: $PWD"
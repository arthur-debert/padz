#!/bin/zsh

echo "Testing global vs project scratches..."
echo

echo "1. Creating global scratch:"
echo "Global scratch content" | padz new --global "Global Scratch"

echo
echo "2. Creating project scratch:"
echo "Project scratch content" | padz new "Project Scratch"

echo
echo "3. Listing global scratches:"
padz list --global

echo
echo "4. Listing project scratches:"
padz list

echo
echo "5. Creating a git repo and testing project detection:"
git init
padz list
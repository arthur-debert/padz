#!/usr/bin/env zsh
# shellcheck shell=bash
# List pads from both global and project scopes
# Run after base-fixture.sh to see the fixture data

echo "=== Global Pads ==="
padz -g list

echo ""
echo "=== Project Pads (project-a) ==="
cd projects/project-a || exit
padz list
cd ../.. || exit

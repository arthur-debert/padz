#!/bin/bash

# Commit the test fixes
git add -A
git commit -m "fix: ensure naked int invocation only accepts positive integers

- Update shouldRunViewOrOpen to reject zero and negative numbers
- Fix test expectations to match the routing logic
- Ensure padz 0 and padz -1 don't trigger view command"
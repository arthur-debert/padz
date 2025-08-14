#!/bin/bash

# Create PR for naked int invocation feature
gh pr create --title "feat: add naked int invocation for view/open shortcut" --body "$(cat <<'EOF'
## Summary
- Added support for `padz <int>` as a shortcut for `padz view <int>`
- Made the command (view/open) configurable via a constant in pkg/config
- Enhanced the naked invocation routing logic to detect integer arguments

## Changes
- Added `NakedIntCommand` constant in `pkg/config/config.go` to choose between "view" or "open"
- Implemented `shouldRunViewOrOpen()` function to detect single integer arguments
- Updated `Execute()` routing logic to handle the new pattern
- Added comprehensive unit tests for all routing functions

## Usage
Now users can quickly view scratches by index:
```bash
# Before
padz view 2

# After (also works)
padz 2
```

## Test Plan
- [x] Added unit tests for `shouldRunViewOrOpen()`, `shouldRunLs()`, and `shouldRunCreate()`
- [x] Manually tested `padz <int>` invocation
- [x] Verified other naked invocations still work (`padz`, `padz -s term`, `padz "text"`)
- [x] All existing tests pass

🤖 Generated with [Claude Code](https://claude.ai/code)
EOF
)"
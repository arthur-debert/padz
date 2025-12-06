# Basic Padz Verification

# Create a pad
padz n "Test Pad"

# List pads (should show 1 pad)
padz ls

# Create another pad
padz n "Second Pad"

# Verify pinned basics
padz p 2
padz ls

# Verify delete
padz rm 1
padz ls
padz ls --deleted

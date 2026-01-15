# =============================================================================
# Base Fixture - Creates a diverse set of pads for testing
# =============================================================================
#
# This fixture creates pads in both global and project scopes with various
# attributes to enable comprehensive testing:
#
#   - Global scope: pads accessible from anywhere (no git context needed)
#   - Project scope: pads in project-a (requires being in that git repo)
#   - Nested pads: children inside parents
#   - Tagged pads: with unique and shared tags
#   - Pinned pads: protected from deletion
#   - Completed pads: marked as done
#   - Deleted pads: soft-deleted, visible with --deleted
#
# Title convention: "<Scope> pad: <description>" for easy debugging
# =============================================================================

# -----------------------------------------------------------------------------
# GLOBAL SCOPE PADS
# -----------------------------------------------------------------------------
# Create from workspace root (no git repo = global scope)

# Simple global pads
padz -g create --no-editor "Global pad: Meeting Notes"
padz -g create --no-editor "Global pad: Quick Reference"
padz -g create --no-editor "Global pad: API Documentation"

# Global pad that will be nested into
padz -g create --no-editor "Global pad: Projects Overview"

# Create a nested global pad (child of "Projects Overview" which is index 4)
padz -g create --no-editor --inside 4 "Global pad: Backend Tasks"

# Create tags first, then apply them
padz -g tags create work
padz -g tags create important
padz -g tags create reference

# Tag some global pads (using display indexes)
# "Meeting Notes" = 1, "Quick Reference" = 2, "API Documentation" = 3
# Note: Use -- to separate -t options from positional INDEXES argument
padz -g add-tag -t work -t important -- 1
padz -g add-tag -t reference -- 2 3
padz -g add-tag -t work -- 3

# Pin a global pad
padz -g pin 2

# Complete a global pad
padz -g complete 1

# Delete a global pad (soft delete)
padz -g delete 3

# Show global pads
echo ""
echo "=== Global Pads ==="
padz -g list
echo ""
echo "=== Global Pads (including deleted) ==="
padz -g list --deleted

# -----------------------------------------------------------------------------
# PROJECT SCOPE PADS
# -----------------------------------------------------------------------------
# Create from inside project-a (git repo = project scope)

cd projects/project-a

# Simple project pads
padz create --no-editor "Project pad: Feature Implementation"
padz create --no-editor "Project pad: Bug Tracker"
padz create --no-editor "Project pad: Test Coverage Report"

# Project pad that will be nested into
padz create --no-editor "Project pad: Sprint Backlog"

# Create nested project pads
padz create --no-editor --inside 4 "Project pad: Sprint Item Alpha"
padz create --no-editor --inside 4 "Project pad: Sprint Item Beta"

# Create project tags first
padz tags create feature
padz tags create priority
padz tags create bug
padz tags create testing

# Tag some project pads
padz add-tag -t feature -t priority -- 1
padz add-tag -t bug -t priority -- 2
padz add-tag -t testing -- 3

# Pin a project pad
padz pin 1

# Complete a project pad
padz complete 3

# Delete a project pad (soft delete)
padz delete 2

# Show project pads
echo ""
echo "=== Project Pads ==="
padz list
echo ""
echo "=== Project Pads (including deleted) ==="
padz list --deleted

# Return to workspace
cd ../..

echo ""
echo "=== Fixture Complete ==="
echo "Global pads: 5 total (1 deleted, 1 pinned, 1 completed, 1 nested)"
echo "Project pads: 6 total (1 deleted, 1 pinned, 1 completed, 2 nested)"

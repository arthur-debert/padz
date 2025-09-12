#!/usr/bin/env bash

set -uo pipefail

# Bug report script - documents all discovered issues
echo "=== PADZ BUG REPORT ==="
echo "Generated: $(date)"
echo "Environment: $HOME"
echo ""

echo "=== BUG #1: CRITICAL - Global Create Command Broken ==="
echo "Severity: CRITICAL"
echo "Command: padz --global create 'content'"
echo "Expected: Creates global pad, shows 'Created pad global-X'"
echo "Actual: Shows list of global pads instead"
echo "Root Cause: Command resolution logic in root.go line 214"
echo "Code Location: cmd/padz/cli/root.go:214"
echo ""
echo "Demonstration:"
echo "$ padz --global create 'Test content'"
padz --global create 'Test content' || true
echo ""
echo "Workaround: cd \$HOME && padz create 'content'"
cd $HOME
padz create 'Workaround content'
echo ""

echo "=== BUG #2: MEDIUM - List Ordering Issue ==="
echo "Severity: MEDIUM"
echo "Issue: List shows pads in reverse order (newest ID shown first)"
echo "Expected: Consistent ordering based on creation time or ID"
echo "Demonstration:"
cd ../projectfoo
padz create 'First pad'
padz create 'Second pad'  
padz create 'Third pad'
echo "Created 3 pads in order. List shows:"
padz list
echo ""

echo "=== BUG #3: LOW - Title Display Logic ==="
echo "Severity: LOW"
echo "Issue: Pads without explicit title show '(untitled)' even when content exists"
echo "Expected: Could use first line of content as title if no title set"
echo "Demonstration:"
padz create 'This could be the title'
padz create -t 'Explicit Title' 'This has both title and content'
echo "List shows:"
padz list | tail -5
echo ""

echo "=== BUG #4: MEDIUM - Command Resolution Over-Aggressive ==="
echo "Severity: MEDIUM" 
echo "Issue: Any unknown flag triggers list command (line 214 root.go)"
echo "Example: padz --unknown-flag should show error, not list"
echo "Demonstration:"
echo "$ padz --nonexistent-flag"
padz --nonexistent-flag 2>/dev/null || echo "Shows list instead of error"
echo ""

echo "=== SUMMARY ==="
echo "Total Bugs Found: 4"
echo "Critical: 1 (Global create broken)"
echo "Medium: 2 (List ordering, command resolution)"  
echo "Low: 1 (Title display logic)"
echo ""
echo "Impact Assessment:"
echo "- Global scope functionality severely impacted"
echo "- Command-line UX issues with flag handling"
echo "- Minor display/ordering inconsistencies"
echo ""
echo "Recommended Priority:"
echo "1. Fix global create bug (CRITICAL)"
echo "2. Fix command resolution over-aggressiveness"
echo "3. Fix list ordering consistency"
echo "4. Improve title display logic"

if [[ "${OUTPUT_FORMAT:-text}" == "json" ]]; then
    cat <<EOF

{
    "bug_report": {
        "generated": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
        "total_bugs": 4,
        "bugs": [
            {
                "id": 1,
                "severity": "CRITICAL",
                "title": "Global create command broken",
                "command": "padz --global create 'content'",
                "expected": "Creates global pad",
                "actual": "Shows list of global pads",
                "root_cause": "Command resolution logic in root.go:214",
                "location": "cmd/padz/cli/root.go:214",
                "workaround": "cd \\$HOME && padz create 'content'"
            },
            {
                "id": 2,
                "severity": "MEDIUM", 
                "title": "List ordering inconsistency",
                "issue": "List shows pads in reverse creation order",
                "location": "cmd/padz/cli/list.go"
            },
            {
                "id": 3,
                "severity": "LOW",
                "title": "Title display logic could be improved", 
                "issue": "Shows (untitled) even when content exists"
            },
            {
                "id": 4,
                "severity": "MEDIUM",
                "title": "Command resolution over-aggressive",
                "issue": "Unknown flags trigger list instead of error",
                "location": "cmd/padz/cli/root.go:214"
            }
        ],
        "priority_order": [
            "Fix global create bug (CRITICAL)",
            "Fix command resolution over-aggressiveness", 
            "Fix list ordering consistency",
            "Improve title display logic"
        ]
    }
}
EOF
fi
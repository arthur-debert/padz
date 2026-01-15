# Padz Live Test Shell Configuration

# Ensure PADZ_GLOBAL_DATA is exported (should be inherited, but make sure)
export PADZ_GLOBAL_DATA

# Vi mode
bindkey -v

# Custom prompt showing we're in test mode
PS1='[padz-test] %~ $ '

# Alias padz to the dev binary
alias padz='__PADZ_BIN__'

# Convenience aliases
alias pa='padz'
alias proj='cd __WORKSPACE__/projects/project-a'
alias ws='cd __WORKSPACE__'

# Helper to show current scope
scope() {
    if [[ -d .git ]] || git rev-parse --git-dir &>/dev/null 2>&1; then
        echo "project scope (in git repo)"
    else
        echo "global scope (no git repo)"
    fi
}

# Helper to show environment
env_info() {
    echo "Workspace:        __WORKSPACE__"
    echo "PADZ_GLOBAL_DATA: ${PADZ_GLOBAL_DATA}"
    echo "Binary:           __PADZ_BIN__"
    echo "Current dir:      $(pwd)"
    echo "Scope:            $(scope)"
}

# Export history without line numbers (useful for capturing commands)
export_history() {
    fc -ln 1 2>/dev/null || echo "# No history available"
}

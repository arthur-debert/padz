# Set vi mode
bindkey -v

# Set custom prompt
PS1="[too-test] $ "

# Create a wrapper function for too that always uses the correct data path and format
function too() {
    command too --data-path="${TODO_DB_PATH}" --format="${TOO_FORMAT}" "$@"
}

# Create an alias for raw too command (without automatic flags)
alias too-raw="command too"
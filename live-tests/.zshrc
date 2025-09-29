# Set vi mode
bindkey -v

# Set custom prompt
PS1="[padz-test] $ "

# Create a wrapper function for padz to ensure we're using the right binary
function padz() {
    "${PADZ_BIN:-padz}" "$@"
}
#!/bin/sh
# Install padz — a curl-piped installer for the padz CLI.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/arthur-debert/padz/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/arthur-debert/padz/main/install.sh | VERSION=v1.4.1 sh
#   curl -fsSL https://raw.githubusercontent.com/arthur-debert/padz/main/install.sh | PREFIX=/usr/local sh
#
# The script is reusable across cargo projects — change the four ─── REUSE ───
# values below and it works for any crate whose release workflow follows the
# same `<bin>-<target>.tar.gz` naming convention.

set -eu

# ─── REUSE: values to edit when adopting this script for another project ───
BIN_NAME=${BIN_NAME:-padz}
REPO=${REPO:-arthur-debert/padz}
# Prefix where the binary goes. "$HOME/.local" needs no sudo; "/usr/local"
# gets you a system-wide install if you pipe with sudo.
PREFIX=${PREFIX:-$HOME/.local}
# Pinned version, or "latest" to resolve from the GitHub API.
VERSION=${VERSION:-latest}
# ────────────────────────────────────────────────────────────────────────────

info()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m==>\033[0m %s\n' "$*" >&2; }
fatal() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || fatal "required command not found: $1"
}

need uname
need tar
need mkdir
# curl OR wget is fine — checked in download().

detect_target() {
    os=$(uname -s)
    arch=$(uname -m)
    case "$os" in
        Darwin)
            case "$arch" in
                arm64|aarch64) echo "aarch64-apple-darwin" ;;
                x86_64)        echo "x86_64-apple-darwin" ;;
                *) fatal "unsupported macOS architecture: $arch" ;;
            esac
            ;;
        Linux)
            case "$arch" in
                x86_64|amd64)  echo "x86_64-linux-gnu" ;;
                aarch64|arm64) echo "aarch64-linux-gnu" ;;
                *) fatal "unsupported Linux architecture: $arch" ;;
            esac
            ;;
        *)
            fatal "unsupported OS: $os (this installer supports macOS and Linux)"
            ;;
    esac
}

download() {
    url="$1"
    dest="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL -o "$dest" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$dest" "$url"
    else
        fatal "need either curl or wget"
    fi
}

resolve_version() {
    if [ "$VERSION" != "latest" ]; then
        echo "$VERSION"
        return
    fi
    # GitHub's /releases/latest 302-redirects to the newest published (non-draft)
    # release. Following the redirect without downloading tells us the tag.
    need_curl=
    command -v curl >/dev/null 2>&1 || need_curl="1"
    if [ -n "$need_curl" ]; then
        # Fallback: parse the API JSON. jq would be cleaner but we avoid it
        # to keep the installer's dependency set minimal.
        download "https://api.github.com/repos/${REPO}/releases/latest" /tmp/padz-latest.json
        tag=$(sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' /tmp/padz-latest.json | head -1)
        rm -f /tmp/padz-latest.json
    else
        tag=$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
            "https://github.com/${REPO}/releases/latest" \
            | sed 's|.*/tag/||')
    fi
    [ -n "$tag" ] || fatal "could not resolve latest release tag"
    echo "$tag"
}

main() {
    target=$(detect_target)
    tag=$(resolve_version)
    info "installing $BIN_NAME $tag for $target"

    archive="${BIN_NAME}-${target}.tar.gz"
    url="https://github.com/${REPO}/releases/download/${tag}/${archive}"

    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT

    info "downloading $url"
    download "$url" "${tmp}/${archive}"

    info "extracting"
    ( cd "$tmp" && tar -xzf "$archive" )

    # The release tarball contains a single top-level directory with the
    # binary inside: $BIN_NAME-$target/$BIN_NAME. Locate it either way.
    bin_src=""
    if [ -x "${tmp}/${BIN_NAME}" ]; then
        bin_src="${tmp}/${BIN_NAME}"
    else
        bin_src=$(find "$tmp" -maxdepth 3 -type f -name "$BIN_NAME" -perm -u+x | head -1)
    fi
    [ -n "$bin_src" ] || fatal "did not find $BIN_NAME binary inside $archive"

    bin_dir="${PREFIX}/bin"
    mkdir -p "$bin_dir"
    install -m 0755 "$bin_src" "${bin_dir}/${BIN_NAME}"
    info "installed ${bin_dir}/${BIN_NAME}"

    # Install shell completions if the binary supports it. padz's CLI is
    # `<bin> completion install`; for other projects this may need tweaking
    # or can simply be removed.
    if "${bin_dir}/${BIN_NAME}" completion install 2>/dev/null; then
        :  # success message already printed by padz
    else
        warn "could not auto-install shell completions; run \`${BIN_NAME} completion install\` manually if desired"
    fi

    # PATH hint if the install dir isn't already on $PATH.
    case ":$PATH:" in
        *":$bin_dir:"*) ;;
        *)
            warn "$bin_dir is not on your \$PATH. Add this to your shell profile:"
            # shellcheck disable=SC2016  # literal $PATH is intended — user pastes this line into rc file
            printf '\n    export PATH="%s:$PATH"\n\n' "$bin_dir" >&2
            ;;
    esac

    info "done. try: $BIN_NAME --version"
}

main "$@"

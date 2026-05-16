#!/usr/bin/env bash
# Project-local dev/CI environment setup.
#
# Installs OS-level tools that aren't shipped by the Claude Code on the web
# base image but are required by this repo's checks. Safe to invoke from a
# SessionStart hook — each step is idempotent.
#
# Background: bats e2e tests (`live-tests/run-tests`) call `uuidgen` from
# `live-tests/lib/backdoors.bash` to fabricate orphan content files. The
# base cloud image ships `libuuid1` and `uuid-dev` but not the
# `uuid-runtime` package that provides the `uuidgen` binary, so without
# this step the doctor.bats orphan-recovery tests fail.

set -euo pipefail

log() { printf '[setup-dev-env] %s\n' "$*"; }

ensure_uuidgen() {
    if command -v uuidgen >/dev/null 2>&1; then
        return 0
    fi
    log "installing uuid-runtime (provides uuidgen, needed by live-tests/lib/backdoors.bash)"
    if [[ $EUID -eq 0 ]]; then
        apt-get update -qq
        apt-get install -y --no-install-recommends uuid-runtime
    else
        sudo apt-get update -qq
        sudo apt-get install -y --no-install-recommends uuid-runtime
    fi
}

ensure_uuidgen

log "ok"

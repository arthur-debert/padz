Cargo Release Pipeline:
Tutorial and reference for reusing padz's distribution workflow

    The padz repo ships a parameterized release pipeline that turns a
    `git tag vX.Y.Z` push into four distribution channels:

    - `cargo install <crate>` (crates.io)
    - `brew install <tap>/<formula>` (Homebrew tap)
    - `curl -fsSL .../install.sh | sh` (prebuilt tarball to `~/.local/bin`)
    - `sudo apt install ./<pkg>.deb` (Debian/Ubuntu)

    Everything is driven by the `env:` block at the top of
    `.github/workflows/release.yml` plus a small set of per-project files.
    Copy the pieces you want, edit the values, and a tag push does the rest.

    This document is split into two halves. Part one is a tutorial for
    adopting the pipeline in a new cargo project. Part two is a reference
    for each file and every configuration knob.


1. Tutorial: adopting the pipeline for another project

    This walks through copying the pipeline into a fresh cargo project
    called `mytool` with a single binary crate. The process takes under
    an hour if you already have a crates.io account and a Homebrew tap.

    1.1. Prerequisites

        Before starting, make sure you have:

        - A cargo project with at least one binary crate
        - A `CHANGELOG.md` at the repo root using `## [X.Y.Z]` section
          headers (the workflow extracts release notes from these)
        - A `LICENSE` file at the repo root
        - `package.description`, `package.license`, and either
          `package.homepage` or `package.repository` set in the binary
          crate's Cargo.toml — the Homebrew job refuses to render a
          formula with any of these empty
        - A crates.io API token (used as the `CRATES_IO_KEY` secret)
        - Optional: a Homebrew tap repository (format
          `<owner>/homebrew-<name>`) with a PAT scoped `contents:write`
          to that repo (used as the `HOMEBREW_TAP_TOKEN` secret)

    1.2. Copy the files

        From the padz repo, copy these paths into the new project,
        preserving structure:

        Files to copy:

            .github/workflows/release.yml
            .github/homebrew-formula.rb.tmpl
            .github/render-homebrew-formula.py
            install.sh

        :: files ::

        If you want `.deb` support, also copy the maintainer scripts
        (adjust the path to match your crate layout):

        Debian files:

            crates/<crate>/debian/postinst
            crates/<crate>/debian/prerm

        :: files ::

    1.3. Edit the release workflow

        Open `.github/workflows/release.yml` and change the `env:`
        block at the top. Every project-specific value lives here —
        no other part of the workflow should need editing.

        For `mytool`:

            env:
              CARGO_TERM_COLOR: always
              BIN_NAME: mytool
              CRATE_NAME: mytool
              EXTRA_CRATES: ""
              PUBLISH_WAIT_SECONDS: "30"
              HOMEBREW_TAP: myorg/homebrew-tap
              HOMEBREW_FORMULA: mytool
              ENABLE_DEB: "true"

        :: yaml ::

        If `mytool` is a single-crate project, leave `EXTRA_CRATES`
        empty. If it depends on a sibling workspace crate `mytool-core`
        that also needs to be published, set `EXTRA_CRATES: "mytool-core"`
        (space-separated, in dependency order — siblings that depend on
        nothing first, top-level crate last).

    1.4. Edit the installer script

        Open `install.sh` and change the four values in the REUSE block:

            BIN_NAME=${BIN_NAME:-mytool}
            REPO=${REPO:-myorg/mytool}
            PREFIX=${PREFIX:-$HOME/.local}
            VERSION=${VERSION:-latest}

        :: shell ::

        `PREFIX` and `VERSION` default to sensible values and should not
        need editing — users can override them with env vars when piping
        to `sh`. Only `BIN_NAME` and `REPO` are mandatory.

        The tail of the script runs `<BIN_NAME> completion install` after
        placing the binary. If your CLI doesn't have that subcommand,
        delete the block around the `completion install` call — it fails
        softly today but it's dead code for projects without it.

    1.5. Add the Debian metadata (optional)

        If `ENABLE_DEB: "true"`, add a `[package.metadata.deb]` stanza
        to the binary crate's Cargo.toml. Paths are relative to the
        crate directory (where the Cargo.toml lives).

        Sample stanza:

            [package.metadata.deb]
            maintainer = "Your Name <you@example.com>"
            copyright = "2026, Your Name <you@example.com>"
            license-file = ["../../LICENSE", "0"]
            extended-description = """
            Longer description of mytool. Shows up in apt-cache show
            and on package listing pages.
            """
            section = "utility"
            priority = "optional"
            depends = "$auto"
            assets = [
                ["target/release/mytool", "usr/bin/", "755"],
                ["../../README.md", "usr/share/doc/mytool/README.md", "644"],
                ["../../CHANGELOG.md", "usr/share/doc/mytool/CHANGELOG.md", "644"],
            ]
            maintainer-scripts = "debian/"

            [package.metadata.deb.variants.amd64]
            [package.metadata.deb.variants.arm64]

        :: toml ::

        Key points:

        - The `target/release/mytool` asset path is correct even when
          CI cross-compiles. cargo-deb automatically rewrites
          `target/release/` to `target/<triple>/release/` when `--target`
          is passed (as the workflow does), so you don't need a second
          asset block per architecture.
        - The empty `[package.metadata.deb.variants.amd64]` /
          `[.arm64]` tables tell cargo-deb which architectures are
          valid — without them, cross-compiled builds fail architecture
          detection.
        - `maintainer-scripts = "debian/"` expects `postinst` and
          `prerm` files in a sibling `debian/` directory. The padz
          scripts generate shell completions at install time and remove
          them at uninstall; adapt or delete for other CLIs.

    1.6. Configure secrets in GitHub

        Under repo Settings → Secrets and variables → Actions, add:

        - `CRATES_IO_KEY` — token from https://crates.io/me (Account
          Settings → API Tokens)
        - `HOMEBREW_TAP_TOKEN` — fine-grained PAT with `contents:write`
          on the tap repo (not the source repo). Only needed if you
          want brew distribution; the workflow skips cleanly when
          absent.

        You can verify a secret exists without printing it:

            gh secret list

        :: shell ::

        And set one from the shell (handy to script):

            gh secret set CRATES_IO_KEY --body "$CARGO_REGISTRY_TOKEN"

        :: shell ::

    1.7. First release

        Bump the workspace version, commit, tag, push:

            cargo install cargo-edit   # once, if not installed
            cargo set-version --workspace 0.1.0
            git add -u && git commit -m "chore: 0.1.0"
            git tag v0.1.0
            git push origin main v0.1.0

        :: shell ::

        Watch the workflow in the Actions tab. On first run, expect to
        hit one of these issues — most are one-line fixes caught by the
        workflow's own validation:

        - "Missing description/license/homepage" — add the field to
          Cargo.toml, re-tag.
        - `cargo publish` fails with "crate name is already taken" — pick
          a different name or add `publish = false` and remove the
          publish step.
        - Homebrew job shows "skip=true" — `HOMEBREW_TAP_TOKEN` not set.
          Fine if brew is optional; otherwise add the secret and re-run
          that job via `gh run rerun --job <id>`.

    1.8. Validate the four channels

        After the release lands, verify each channel works:

            # cargo
            cargo install mytool

            # homebrew
            brew install myorg/tap/mytool

            # install.sh (in a temp HOME so completions go there)
            curl -fsSL https://raw.githubusercontent.com/myorg/mytool/main/install.sh | sh

            # .deb (in a Docker container to avoid polluting host)
            docker run --rm -it ubuntu:24.04 bash -c '
              apt-get update && apt-get install -y curl ca-certificates
              curl -fsSLO https://github.com/myorg/mytool/releases/download/v0.1.0/mytool_0.1.0-1_amd64.deb
              apt install -y ./mytool_0.1.0-1_amd64.deb
              mytool --version
            '

        :: shell ::


2. Reference

    2.1. File layout

        | Path                                       | Purpose |
        | `.github/workflows/release.yml`            | Tag-triggered release workflow |
        | `.github/homebrew-formula.rb.tmpl`         | Ruby formula template with `{{PLACEHOLDER}}` tokens |
        | `.github/render-homebrew-formula.py`       | Template renderer with Ruby-safe escaping |
        | `install.sh`                               | curl-piped installer for the release tarball |
        | `crates/<crate>/Cargo.toml`                | Hosts `[package.metadata.deb]` |
        | `crates/<crate>/debian/postinst`           | Installs shell completions post-apt-install |
        | `crates/<crate>/debian/prerm`              | Cleans up completions pre-apt-remove |

        :: table ::

    2.2. Workflow environment variables

        All knobs live in the `env:` block of `release.yml`. Nothing
        outside that block should change when adopting the pipeline.

        | Variable               | Required | Default | Meaning |
        | `BIN_NAME`             | yes      | —       | Binary name. Drives tarball filenames, deb names, install paths. |
        | `CRATE_NAME`           | yes      | —       | Top-level crate to publish last. Usually equal to `BIN_NAME`. |
        | `EXTRA_CRATES`         | no       | ""      | Space-separated sibling crates to publish first, in dep order. |
        | `PUBLISH_WAIT_SECONDS` | no       | 30      | Delay after each publish so crates.io index catches up. |
        | `HOMEBREW_TAP`         | no       | ""      | `<owner>/<tap-repo>` for brew formula push. Blank disables. |
        | `HOMEBREW_FORMULA`     | no       | —       | Formula filename without `.rb`. Usually equals `BIN_NAME`. |
        | `ENABLE_DEB`           | no       | "false" | Set to `"true"` to build and attach `.deb` packages. |

        :: table align=llll ::

    2.3. Required secrets

        | Secret                | Required | Scope | Purpose |
        | `CRATES_IO_KEY`       | yes      | Repo  | `cargo publish` auth. |
        | `HOMEBREW_TAP_TOKEN`  | no       | Tap repo contents:write | Push formula to tap repo. Job skips if missing. |

        :: table ::

    2.4. Job graph

        The workflow has six jobs that run in this dependency order:

        - `prepare` — extracts version from tag, extracts release notes
          from CHANGELOG.md, uploads notes as an artifact.
        - `publish` — depends on `prepare`. Runs `cargo set-version
          --workspace <VERSION>`, commits the bump to the default
          branch, publishes each crate in dep order.
        - `release` — depends on `prepare`. Creates a *draft* GitHub
          Release with the extracted notes.
        - `build-binaries` — depends on `prepare`. Matrix job building
          for four targets: `aarch64-apple-darwin`, `x86_64-apple-darwin`,
          `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`.
          Linux targets also produce `.deb` when `ENABLE_DEB=true`.
        - `finalize` — depends on `prepare`, `release`, `build-binaries`,
          `publish`. Downloads all artifacts, attaches them to the
          release, flips draft → published.
        - `homebrew-formula` — depends on `prepare`, `finalize`. Guards
          on `HOMEBREW_TAP_TOKEN`: renders the formula from the template,
          commits it to `<HOMEBREW_TAP>/Formula/<HOMEBREW_FORMULA>.rb`.
          All steps are gated on the guard output, so the job is safe
          to keep even before the tap is set up.

    2.5. How the version bump works

        The workflow uses `cargo set-version --workspace <VERSION>` from
        the cargo-edit crate. This single command:

        - Rewrites `version` in every workspace member's Cargo.toml
        - Rewrites every intra-workspace dependency pin (`path = "..."
          version = "..."`) so the published artifacts pin the exact
          sibling version that's being released together
        - Updates `Cargo.lock` to match

        A naive `sed` approach misses the intra-workspace pin rewrite,
        which causes `cargo publish` to fail on sibling crates with
        "version mismatch between path and registry dep". The workflow
        also runs `cargo set-version` in the `build-binaries` matrix so
        the compiled binary reports the release version even before the
        `publish` job's bump commit lands.

    2.6. Handling `.tar.gz` layout

        The release tarball contains a single top-level directory named
        `<BIN_NAME>-<target>` with the binary inside. Example:

        Archive layout:

            padz-x86_64-linux-gnu.tar.gz
            └── padz-x86_64-linux-gnu/
                ├── padz
                ├── README.md
                ├── CHANGELOG.md
                └── LICENSE

        :: tree ::

        The installer's `find "$tmp" -maxdepth 3 -type f -name ...`
        tolerates either this layout or a flat archive, so changing
        the workflow's packaging step (to, e.g., flatten the archive)
        doesn't break installers already out in the wild.

    2.7. Homebrew formula rendering

        The formula is a Ruby template rendered by
        `.github/render-homebrew-formula.py` at workflow time. The
        Python renderer substitutes `{{PLACEHOLDER}}` tokens with env
        vars and applies Ruby-safe escaping to values that are
        interpolated into double-quoted strings (`DESC`, `LICENSE`,
        `HOMEPAGE`). Pass-through keys (URLs, SHAs, class name) are
        written literally because their character set is known safe.

        Why not `sed`? Ruby double-quoted strings have four concerning
        sequences: `\`, `"`, `#{` (interpolation), and newlines. A sed
        pipeline that escapes all of these without corrupting valid
        input is brittle; the Python renderer handles them via a
        single `ruby_double_quoted_escape()` function with a small
        suite of asserts.

        The workflow also validates formula syntax with `ruby -c` before
        pushing, so a bad render fails the job instead of committing a
        broken formula to the tap.

    2.8. Debian completion handling

        `postinst` and `prerm` handle shell completions at install and
        removal time, respectively. They invoke the freshly-installed
        binary with its `completion --shell X print` subcommand rather
        than shipping static completion scripts.

        Why invoke the binary? clap_complete's generated completions
        embed the *absolute path* to the binary (e.g., `/usr/bin/padz`).
        For a cross-compiled artifact built on an amd64 runner targeting
        arm64, the build environment doesn't know the install path —
        generating at install time is the only way to get a correct
        registration line.

        If your CLI doesn't have a `completion --shell X print`
        subcommand, either delete the maintainer scripts (the `.deb`
        will still install fine, just without completions) or adapt
        them to your CLI's equivalent.

    2.9. Tolerated failure modes

        Several conditions are tolerated gracefully rather than failing
        the job:

        - `cargo publish` returns "already published" — the workflow
          checks crates.io and treats it as success. Re-tagging a
          release after a partial failure is safe.
        - Empty commit on version bump — if the workspace is already at
          the target version (local cargo-release did the bump), the
          commit step exits 0 without pushing.
        - `HOMEBREW_TAP_TOKEN` missing — the whole `homebrew-formula`
          job's steps skip. Brew distribution becomes opt-in.
        - Formula unchanged in tap — if rendering produces the exact
          bytes already committed, the push step exits 0. Useful for
          re-runs.

    2.10. Reusability checklist

        When copying the pipeline to a new project, change exactly these
        places and nothing else:

        - The `env:` block in `.github/workflows/release.yml`
        - The REUSE block in `install.sh`
        - The `[package.metadata.deb]` stanza in the crate's Cargo.toml
          (if using deb)
        - `debian/postinst` and `debian/prerm` references to the binary
          name (two or three lines each)

        Everything else — the job graph, the template, the renderer,
        the matrix, the artifact naming — is project-agnostic and
        should be copied verbatim. If you find yourself needing to edit
        a step body to adopt the pipeline, that's a sign the workflow
        should be further parameterized; open an issue on padz.


3. See also

    - padz source: [https://github.com/arthur-debert/padz]
    - cargo-edit (for `cargo set-version`): [https://github.com/killercup/cargo-edit]
    - cargo-deb: [https://github.com/kornelski/cargo-deb]
    - Homebrew formula cookbook: [https://docs.brew.sh/Formula-Cookbook]
    - clap_complete's `CompleteEnv`: [https://docs.rs/clap_complete/latest/clap_complete/env/index.html]

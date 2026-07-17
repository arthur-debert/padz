- Build the Standout-shaped test pyramid. The `padz` crate now has a library
  target (the binary is a shim over `cli::run()`), so the CLI's own seams —
  the clap command tree, the typed handlers, the app builder — are testable in
  process instead of only through a spawned binary. Test coverage is now layered
  by the smallest seam that can observe each behavior: direct `padzapp` tests for
  domain behavior, direct typed-handler tests for adapter mapping, serial
  `standout-test` `TestHarness` tests for Clap-through-render integration, and
  subprocess E2E only for boundaries a harness cannot model (each retained E2E
  file documents the boundary it protects). The create/edit input-precedence and
  terminal-width suites moved down from subprocess to the harness, which closed a
  real gap: a spawned test process has no pty, so its stdin could never *be* a
  terminal and the editor arm of the input chain was untestable — the harness
  injects the reader and now covers both arms. No user-visible behavior changes.

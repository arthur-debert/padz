- Deepen the CLI/core ownership split for `view` and pin-state notices. `view`
  structured output now keeps `title` and `content` unindented, exposes the
  requested `nesting` mode, and leaves four-space human indentation to the
  MiniJinja template. Viewing multiple selected roots now performs one clipboard
  write in display order, separated by `---`; child rows remain excluded from the
  `view` clipboard payload, as before. Repeating `pin` or `unpin` now exposes an
  additive `notices` array with a stable semantic `kind` and canonical display
  path; the former English entry is no longer duplicated in `messages`, while the
  human sentence remains unchanged through the modification template.

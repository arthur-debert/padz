- Tag listing now exposes an ordered `tags` array with an explicit
  `empty`/`listed` status. Tag registry mutations expose `created`/`deleted`/`renamed`
  actions with their relevant names and affected-pad counts; per-pad assignment and
  removal expose requested tag arrays, modified-pad counts, and distinct
  `all_already_present`/`none_present` no-ops. These dedicated structured schemas
  replace the former prose-only `messages` and generic modification results. Human
  wording, ordering, brackets, pluralization, pad rows, and semantic `info`/`success`
  styling remain unchanged through CLI-owned MiniJinja templates.

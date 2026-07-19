- Clone and migrate now expose one semantic cross-store transfer report in
  structured output. The report identifies operation, direction, resolved peer
  store, requested selection, copied pad IDs/count, and explicit empty,
  full-success, partial-success, or no-copy status. Ordered typed diagnostics
  distinguish source-read and destination-write copy failures, parent orphaning,
  destination bucket enumeration failures, tag-registry merge failures, and
  migrate source-delete failures. This intentionally replaces the former
  prose-only `messages` schema; human wording, ordering, and styles remain
  compatible through the CLI-owned transfer template.

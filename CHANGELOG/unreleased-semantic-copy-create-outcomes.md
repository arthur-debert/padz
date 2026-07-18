- `copy` structured output now reports CLI-owned `root_pad_count` and ordered
  `titles` facts instead of a prose-only `messages` result. Nested descendants
  remain in the single ordered clipboard payload but do not increase the selected
  root count. Human wording and clipboard bytes remain compatible.
- Empty piped-input and editor creates now return `{ "kind": "aborted",
  "reason": "empty_content" }` in structured modes instead of an empty generic
  modification plus an English warning message. Human `Aborted: empty content`
  wording and successful exit behavior remain unchanged; no pad, editor fallback,
  or clipboard write occurs for an empty pipe.

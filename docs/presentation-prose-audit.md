# Reusable result presentation-prose audit

This is the closing STD03-WS10 inventory after the command-family semantic
outcome migrations. The reusable `padzapp` interface reports user data, typed
outcomes, and typed diagnostics. The `padz` CLI owns wording, pluralization,
ordering, glyphs, indentation, semantic style tags, and blank-line layout.

## Result inventory

The repository-wide search found no live `CmdMessage` producer. The remaining
`CmdMessage`, `MessageLevel`, `messages`, and `trailing_messages` surfaces were
empty pass-through plumbing, so they were removed from `CmdResult`, CLI result
DTOs, render views, templates, and exports. The unused `MessagesResult`, core
`ModificationResult`, and `messages.jinja` adapters were removed by the same
deletion test: deleting them removed interface complexity without moving any
behavior into callers.

Generic pad modifications now carry `ModificationActionResult`, affected pads,
typed notices, typed outcomes, and request facts. The action is a stable semantic
token (`pin`, `delete`, `update`, and so on); `modification_result.jinja` owns the
human past-tense verb and pluralization. Transfer write failures retain only the
causal store diagnostic in `detail`; the already-typed pad id/category and the
template supply the sentence around it.

There are no layout-only empty strings in reusable results. Empty strings found
under `padzapp/src` are content values, parser buffers, or test data. Template
layout continues to use MiniJinja newline operations in `padz`.

## Retained domain-diagnostic text

The following authored text is intentionally retained. These are failed-operation
diagnostics, not successful command-result presentation. Replacing the causal
detail with a success/no-op enum would discard information needed to correct the
request or diagnose storage and decoding failures.

| Source | Diagnostic content | Why it remains in the reusable module |
| --- | --- | --- |
| `commands/helpers/selector_resolve.rs`, `commands/get/selector_filter.rs`, `error.rs`, `index.rs` | Invalid, missing, range, UUID, and ambiguous-title errors | Selector validity and candidate identity are domain rules. `AmbiguousTitle` already exposes typed term/count/candidates; `Display` is the diagnostic fallback for non-structured error transport. |
| `commands/init.rs`, `init.rs`, `store/fs_backend.rs`, `commands/mod.rs` | Missing/unusable stores, invalid links, migration failures, and unavailable scopes | These explain why the storage interface cannot satisfy an operation and include causal paths or underlying I/O errors. They are not layout or success prose. |
| `commands/purge.rs`, `commands/move_pads.rs`, `commands/create.rs`, `commands/update.rs` | Destructive-operation safety requirements and invalid mutation preconditions | Counts, selectors, and required flags are needed to make a refused mutation actionable. No command result exists on these error paths. |
| `commands/tags.rs`, `commands/tagging.rs`, `tags/validation.rs` | Tag-name validation and missing/duplicate tag errors | The invalid value and violated domain invariant are the diagnostic payload. Successful/no-op tag results are typed separately. |
| `commands/io/import.rs`, `commands/io/export.rs`, `commands/metadata_apply.rs` | Decode, archive-entry, serialization, metadata, and registry-merge causes | Status/reason/category fields are typed; retained strings are lower-level parser, I/O, or store causes that cannot be enumerated without losing useful detail. |
| `commands/transfer.rs`, `api/transfer.rs` | Store/path preconditions and causal partial-failure details | Transfer status, operation, direction, pad id, bucket, and failure category are typed. `detail` contains only the underlying store/link cause; the CLI template owns surrounding prose. |

User-authored titles/content, filesystem paths, selector strings, format names,
and suggested artifact filenames are data, not authored presentation sentences.

## Mechanical verification

The closing search is:

```text
rg -n 'CmdMessage|MessageLevel|trailing_messages|messages\.jinja' \
  crates/padzapp/src crates/padz/src
```

It returns no matches. `crates/padzapp/tests/presentation_contract.rs` keeps that
generic-interface absence and a typed core no-op under test. The direct-handler
fixture pins the structured action token and top-level keys; the serial
`TestHarness` proof pins the compatible human sentence at the rendering seam.

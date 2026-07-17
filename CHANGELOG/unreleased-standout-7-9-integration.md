- Upgrade the coherent Standout dependency family from 7.7.0 to 7.9.0. Naked
  `padz` now uses Standout's invocation-aware default resolver (terminal stdin
  lists; redirected stdin, including an empty pipe, creates) without local argv
  injection. Exports now return owned bytes and semantic report facts from
  `padzapp`; Standout selects and writes the suggested or explicit destination,
  then renders the receipt-aware success report. Metadata warnings remain
  machine-readable and artifact bytes/formats are unchanged. Structured export
  reports now use Standout's `{ report, receipt }` envelope rather than the old
  prose `messages` result. Owned-byte exports keep source data and the compressed
  artifact plus encoder buffers live at peak, while API/handler handoff moves the
  byte vector without another copy.

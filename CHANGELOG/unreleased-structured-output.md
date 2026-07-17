- Fix `--output yaml`, `--output xml`, and `--output csv`, which silently rendered
  human terminal output — ANSI escapes, glyphs, and width-truncated titles — instead
  of machine-readable data. All three were accepted as valid flag values and then fell
  back to automatic rendering, so scripts and agents received text that no parser could
  read, with a success exit code. The output mode is now read via Standout's own
  `App::extract_output_mode` rather than a local copy of its mode list, so `json`,
  `yaml`, `xml`, and `csv` each select the requested serialization. `path` and `uuid`
  are included, and structured output is invariant across terminal width and color
  settings. CSV flattens a whole result into one dotted-path row and is lossy for
  nested data by design — use JSON or YAML for nested reads.

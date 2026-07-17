- Upgrade the Standout framework from 6.2 to the 7.6 generation
  (`standout`/`standout-macros`/`standout-dispatch` 7.6.4). Handler failures now
  use Standout's native `RunResult::Error` variant instead of being detected by
  an `"Error:"` string prefix on rendered output. User-visible behavior is
  otherwise unchanged, except that `<command> --help` / `-h` is now rendered by
  Standout (matching `padz help <command>`, which already was) and no longer
  lists the global `-g`/`-v`/`--data`/`--output` flags; those flags are still
  accepted.

- Releases now run entirely on shipit's per-stage release caller
  (`gh workflow run shipit-release.yml`): the caller gained independently
  re-runnable `build`/`sign`/`publish` stage dispatches, and the legacy
  `release.yml` (rust-cli.yml@v3) workflow was removed.

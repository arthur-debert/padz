# padz — agent orientation

This repo's quality gate, PR/dev flow, and release pipeline are managed by
**shipit** (arthur-debert/shipit). The dev-cycle playbook lives in the
shipit-managed block:

@AGENTS.md

## Releasing

Releases run through shipit's composed pipeline via the local caller
`.github/workflows/shipit-release.yml`:

```sh
gh workflow run shipit-release.yml -f version=X.Y.Z -f stage=full
```

`stage` can also dispatch one re-runnable block (`prepare`, `build`, `sign`,
`publish`) — see the caller's header comment. Changelog entries go in
`CHANGELOG/unreleased-<slug>.md` fragments.

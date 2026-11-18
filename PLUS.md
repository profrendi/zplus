# ZFPlus

[ZapFast](https://github.com/crmne/zapfast) with the Enhancements of the
WA Web Plus extension. See `plus/README-ZFPlus.txt` for the features.

- `src/plus.rs`, `src/ui/plus_new_chat.rs` and the `ZFPlus:` comments
  across `src/` are the Enhancements.
- `plus/UPSTREAM` is the ZapFast release this build is on, `plus/REVISION`
  the Plus revision on top of it; the version is
  `major.minor.(patch * 100 + revision)` (`plus/set-version.sh`).
- `plus-sync` (daily) merges each new ZapFast release and calls
  `plus-release`, which builds the Windows installer and portable zip,
  signs `checksums.txt` with the `ZAPFAST_PLUS_SIGNING_KEY` secret and
  publishes a release. Installed and portable copies update from these
  releases (`src/updates.rs`).
- To ship a change to the Enhancements: commit it, add 1 to
  `plus/REVISION`, push to `main`.

## Staying mergeable with ZapFast

Every ZapFast release and every night's ZapFast `main` is merged into
ZFPlus. A line both sides changed stops that merge (ZapFast v0.21.3 did:
ZFPlus had made its own `archive::Write` for what ZapFast then shipped as
`archive::Batch`). So:

- Put Enhancements in files of their own (`src/plus*.rs`,
  `src/ui/plus_*.rs`, `src/backend/worker/plus_pages.rs`) and call them
  from ZapFast's code with as few lines as possible, each marked `ZFPlus:`.
- Never rework ZapFast's own internals (the archive, the worker's event
  loop, transactions, the network, its constants) for speed or tidiness.
  When ZapFast needs a fix or a speed-up, report it upstream and wait for
  their version; where something cannot wait, keep the change small and
  replace it with theirs at the next merge.
- Leave ZapFast's lines as they are, and add ours at least one unchanged
  line away from lines ZapFast is likely to touch.
- When a merge conflicts, take ZapFast's version of their code, then put
  the Enhancements back on top of it; never drop an Enhancement or weaken a
  test to get through.
- Follow `AGENTS.md`: no automation of any kind (no bulk, scheduled or
  automatic sending), nothing logged that is private, every platform kept
  compiling, and ZFPlus kept light (nothing running for a switch that is
  off).

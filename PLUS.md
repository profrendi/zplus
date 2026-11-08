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

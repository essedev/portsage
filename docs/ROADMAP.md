# Roadmap

What is still open. Shipped work lives in [CHANGELOG.md](../CHANGELOG.md), which carries the same detail with dates attached; this file was an archive of nine closed milestones and has been pruned back to the things that have not happened yet.

## Multi-host, Phase 4

Phases 1-3 shipped in v0.11.0 and were validated live against the `forge` dev box. The full plan and the design decisions behind it are in [multi-host-evolution.md](multi-host-evolution.md).

- [ ] Project migration between backends.
- [ ] Health dashboard in Settings.
- [ ] CLI `portsage backends list / add / remove` (today backends are Mac-UI only).
- [ ] Tailscale host auto-discovery via `tailscale status --json`.
- [ ] Server-pushed `StateChanged` events. Deferred on purpose: the 60s periodic sync plus a post-mutation sync covers the lag for a dev-server use case.

## Feature parity

Design notes for the open items are in [feature-proposals.md](feature-proposals.md).

- [ ] Project tags and colours, to recognise projects at a glance.
- [ ] System notifications: macOS alerts for collisions and zombie ports.
- [ ] i18n with a language switcher (i18next, English + Italian, persisted in the DB). The UI is English-only today while the README and ARCHITECTURE ship in both languages.

## Platform

- [ ] Universal macOS binary (arm64 + x86_64). Releases are arm64-only.
- [ ] macOS DMG build in CI. `server-build.yml` produces the Linux tarball on a tag, but the DMG still has to be built on a Mac by hand, which makes every release a manual step.
- [ ] Windows support: Unix socket to named pipe, scanner via netstat or the Win32 API, OS-specific commands.
- [ ] Auto-update via the Tauri updater. Today an upgrade goes through `brew upgrade --cask portsage` or `portsage self-update`.

## Ruled out

Kept here because they come up again otherwise.

- **Recycling the ranges of released projects.** `compute_next_range` always walks forward from `MAX(range_end)`. Numbers are not scarce (27 projects reach 4340 of 65535) and reusing a range would point an old `.env`, or a compose file on a backup, at another project's service. This is also what makes restoring a deleted project with its original range safe.
- **Import ports from docker-compose.yml.** Covered by the MCP tools: an agent reads the compose file and registers the ports.
- **Light theme.** The dark theme is the app's identity.

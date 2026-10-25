# Release checklist

Bloom ships auto-updates to every install, so a bad release reaches users
silently. Run this checklist for every release, every time.

## Owner agreement (24 October 2026)

The agent manages pushes, PR review/merges and Releases under standing owner
authorization; no per-release approval is needed. The agent completes technical
checks and artifact verification. The owner performs the physical Windows tests
below after publication. Record those as pending until the owner reports results;
they do not block a technically validated release intended for testing. Do not
replace the owner's installation or control their desktop automatically.

## 1. Before tagging (on `main`)

- [ ] CI green on `main` (Build, CodeQL) and `bun run build` passes locally.
- [ ] In `src-tauri`: `cargo check --locked`, `cargo clippy --locked --all-targets`, `cargo test --locked`.
- [ ] Prepare the affected native test cases for the owner (or record a requested
      development smoke test separately). Physical checks assigned to the owner:
  - [ ] notch modes (fixed / smart / peek) and hover expansion
  - [ ] dock modes, pinned apps, Start menu
  - [ ] desktop volume overlay and per-app mixer
  - [ ] Settings → About: "Check for Updates" reaches the network without erroring
  - [ ] tray → Quit restores the native taskbar (and no crash flag is left behind)
- [ ] Updater channel health: `bun scripts/verify-updater.mjs v<current>` passes.
      This fetches `latest.json` and every installer the way the app does, within
      the app's own `CHECK_TIMEOUT_SECS` budget (`src-tauri/src/updater.rs`). If it
      fails on the release machine, users on similar networks cannot update —
      fix the manifest/network path before tagging.
- [ ] For network/timeout changes, verify automated failure/timeout handling and
      explicitly list any degraded-connection test still pending with the owner.
      A fast channel check alone does not prove slow-route recovery (SYN retries
      alone can take ~21 s).

## 2. Tag and publish

- [ ] Commit coherent versions with a Spanish Summary and brief body, push main,
      verify CI/CodeQL on that revision, then send a new version tag. Do not move
      published tags. The existing `bun run release` script does not provide the
      required commit body or wait for checks before pushing its tag.
- [ ] Watch the Release workflow to completion; do not walk away from a red run.

## 3. After the workflow

- [ ] `bun scripts/verify-updater.mjs vX.Y.Z` passes.
- [ ] **Owner: N‑1 install smoke test**: on a machine still running the previous
      version, click Settings → About → Check for Updates. It must find the new
      version. Auto-install waits ~24 h by design (`MIN_AUTO_INSTALL_AGE_SECS`),
      but the badge/label must appear immediately.
- [ ] **Owner: fresh install**: download `bloom_X.Y.Z_x64-setup.exe`, install, confirm it
      launches and About shows the new version.
- [ ] **Owner:** quit from the tray on the fresh install; confirm the taskbar returns.
- [ ] Record workarounds, known issues and the owner's test cases in Release
      notes and the delivery log. The desktop app has no remote announcement feed.

## 4. If something is broken

- [ ] Rollback lever: edit the GitHub release and mark it **pre-release**.
      `releases/latest` then points at the previous stable release and update
      checks fall back to it.
- [ ] The 24 h age gate before auto-install gives a window to pull a bad
      release before it rolls out widely.
- [ ] Publish rollback or recovery instructions in the Release/issue with direct
      installer links; the removed announcement feed cannot notify this build.

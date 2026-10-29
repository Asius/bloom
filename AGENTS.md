# AGENTS.md

Windows-only Tauri v2 application: Rust backend + React 19/TypeScript frontend. Since the owner's 28 October 2026 direction, Bloom adds an interactive island to Windows; it does not replace or hide its taskbar or system tray. See DESKTOP_BASELINE.md and ISLAND_MIGRATION.md. Earlier dock/tray requirements are historical and must not be reintroduced.

## Commands

Use Bun 1.4.0 (portable official copy one directory above if absent from PATH), Rust stable and Windows MSVC/SDK.

- bun install --frozen-lockfile; do not create package-lock.json.
- bun run tauri dev: complete app, strict Vite port 1420; no separate dev server.
- bun run test: frontend behavior tests in scripts/.
- bun run build: TypeScript and production frontend.
- In src-tauri: cargo check --locked, cargo clippy --locked --all-targets, cargo test --locked.
- bun run bump <version>: keep package.json, tauri.conf.json, Cargo.toml and Cargo.lock coherent.
- Follow RELEASE_CHECKLIST.md for all publication checks. The automatic release helper does not provide the required Spanish commit body or wait for checks; use explicit release steps.

## Architecture and scope

Three WebView entries: main/index.html (island), overlay/overlay.html (volume and startup/update states), settings/settings.html. Configuration, Vite input and capabilities must agree.

commands.rs exposes only island/settings/audio/file APIs. services.rs handles island input/overlap, media/audio, capture exclusion and display/preferences. window_region.rs bounds native input to visible content; preserve click-through outside that region. The settings window never takes island scale; text scale is independent.

island_settings.rs migrates retired preferences and preserves one original backup. legacy_taskbar.rs restores only an existing old crash marker at startup; it never hides Windows or starts a watchdog. Do not add Explorer integration, live tray inventory, window thumbnails, launcher catalogs, bottom AppBars or Win-key overrides. The ordinary Bloom icon in Windows' notification area offers settings, diagnostics, restart and quit.

Preserve current clock/calendar, music/visualizer, file shelf, volume and island modes. Do not add AI/services from Coucou, notifications, hardware controls or a new layout without choosing scope/design with the owner. Removed timer/radios/battery/brightness/DND/remote promotions stay removed. Preserve original GPL attribution and the fork's signed update channel.

Settings live in app_config_dir/settings.json, with bloom-prefixed string values. Persist through atomic file/cache functions; listen to settings-changed and settings-external-changed. Retired preference keys cannot reactivate removed features. Preserve a damaged settings file instead of overwriting it.

Windows bindings use windows 0.61 with explicit features. windows-core remains a direct dependency required by the shelf COM implementation macro. There is no native observer DLL or C++ build step.

## Git, releases and verification

Work on main following Asius/bloom by default. Every commit needs a short conventional Summary and a brief Spanish body explaining result, purpose, verification and limits. Report hash, Summary and description. Stage only delivery files.

The owner authorizes normal pushes, reviewing/updating/merging PRs, tags and Releases permanently. Never move published tags or log signing secrets. Review dependency updates individually. Synchronize all four versions; verify local checks plus CI/CodeQL on the exact revision before tagging; wait for Release completion and verify latest manifest, full downloads, SHA-256 and updater signatures.

Since 3.11.1, publish one Windows x64 NSIS setup.exe only (plus its updater signature and latest.json). Keep bundle.targets=["nsis"] and the Release workflow's --bundles nsis argument. Do not delete or retag historical releases. GitHub's generated source archives are not additional installers.

Island view changes use a keyed immediate replacement, with container springs and entrance fades. Do not restore independent exit-presence queues for music and tool panels. Geometry and JSX share islandView(); media snapshot equality includes seek permission and position timestamp. Visualizer is shared by both players without importing App. Browser regression uses scripts/preview-island.mjs in target/ui-preview; never ship IPC mocks or infer native Windows behavior from this fixture.

For the NSIS-only channel, updater 2.12.0 is pinned with rustls-tls and system-proxy, without its optional zip feature. Shelf previews use the reviewed zip 9.0.5. Do not re-enable legacy archive updater formats or leave an affected second zip version in the lockfile; review future updater/platform upgrades separately. See ISLAND_STABILITY.md.

The agent owns technical checks; the owner tests installed behavior. Publication, pure tests and browser IPC mocks do not prove native Windows behavior. List pending owner cases per release and keep roadmap status honest. Do not replace the installed app or control the owner's desktop as an automatic publication step.

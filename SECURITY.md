# Security Policy

## Supported versions

Only the latest release of Bloom receives security fixes.

| Version        | Supported |
| -------------- | --------- |
| Latest release | ✅        |
| Older releases | ❌        |

## Reporting a vulnerability

Please do not report security vulnerabilities in public issues or discussions.

Report privately through GitHub's private vulnerability reporting:

https://github.com/Asius/bloom/security/advisories/new

Include as much of the following as you can:

- A description of the issue and its impact
- Steps to reproduce, or a proof of concept
- Bloom version (Settings → About) and Windows version
- Any suggested fix

This is a personal fork maintained by @Asius. Response times depend on maintainer availability. Confirmed fixes will be published in a release, with credit unless you prefer to stay anonymous.

## Scope

Bloom runs as a desktop app with system-level access (window management, media controls, autostart). Reports about privilege escalation, arbitrary code execution, unsafe IPC between the webview and the Rust core, and supply-chain issues are especially appreciated.

## Credential transport review — 2026-11-03

The shared GitHub/Google credential HTTP client now rejects non-HTTPS requests and disables redirects. A regression test verifies HTTP rejection before connecting. OAuth's local browser callback uses its separate loopback listener.

CodeQL alert #1 (`rust/cleartext-transmission`) flags the public repository URL built from `account.login`. Inspection found only the public handle in that fixed HTTPS API URL; access tokens are bearer headers, not URL parameters. The owner explicitly approved dismissal; GitHub confirmed `dismissed` / `false positive` on 2026-11-03. No alert suppression was added to code or CI.

## Safe harbor

Good-faith security research on Bloom is welcome. Do not access, modify, or delete other people's data, and do not disrupt the project or its users. If you are unsure whether something is in scope, report it privately first.

## Reviewed dependency alerts — 2026-11-03

Bloom ships Windows x64 NSIS only. The following alerts describe real upstream bugs; their affected code is not used by this product. They remain in Cargo.lock because Cargo records dependencies for other targets and build tools. They are not patched by this review.

- [GHSA-wrw7-89jp-8q8g / RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html): glib 0.18.5 is brought by GTK/WebKit dependencies for Linux. `cargo tree --locked --target x86_64-pc-windows-msvc -i glib@0.18.5` has no matching dependency; `--target all` shows the Linux path. The fixed glib >=0.20 is not compatible with the GTK 0.18 dependency family. No Linux build is distributed.
- [GHSA-cq8v-f236-94qc / RUSTSEC-2026-0097](https://rustsec.org/advisories/RUSTSEC-2026-0097.html): rand 0.7.3 enters through the build path `phf_generator 0.8 -> phf_codegen 0.8 -> selectors 0.24 -> kuchikiki -> tauri-utils`. Its `log` feature is disabled. The advisory requires logging and a logger re-entering the thread RNG, so this configuration does not reach the faulty code. Other locked versions, rand 0.8.8 and 0.9.4, are in the patched ranges. There is no published patched 0.7 version; replacing it with 0.8 would break the transitive API contract.

Run `./scripts/verify-advisory-scope.ps1` to repeat the Windows dependency and feature checks. CI runs this after cargo check. It fails if affected glib starts compiling for Windows or an affected rand enables logging. Reopen/reassess these alerts if Windows targets, dependency features or the supported platforms change. This check addresses these two advisories; it is not a general vulnerability scanner.

GitHub alerts [#1](https://github.com/Asius/bloom/security/dependabot/1) and [#2](https://github.com/Asius/bloom/security/dependabot/2) were dismissed as `not_used` on 2026-11-03 with these findings. They were not marked as patched; the final API check found no open Dependabot alerts.

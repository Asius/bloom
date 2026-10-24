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

## Safe harbor

Good-faith security research on Bloom is welcome. Do not access, modify, or delete other people's data, and do not disrupt the project or its users. If you are unsure whether something is in scope, report it privately first.

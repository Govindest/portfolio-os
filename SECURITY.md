# Security policy

## Reporting a vulnerability

Please **do not** open a public issue for security problems. Instead, use GitHub's private vulnerability reporting ("Report a vulnerability" under the Security tab) on this repository. You should receive an acknowledgment within a few days.

Include reproduction steps and impact. Credentials handling, order submission, and the audit trail are the highest-sensitivity areas.

## Scope

PortfolioOS is a local desktop app. Especially interesting reports:

- Any way brokerage credentials reach SQLite, log files, exports, the frontend after storage, or the network beyond the intended brokerage host
- Any way an order can be submitted without explicit user approval, or while emergency stop / read-only mode is active
- Duplicate-order bugs (idempotency bypass)
- YAML strategy import leading to anything other than validated data (it must never execute)
- SQL injection through user-supplied symbols, names, or CSV imports

## Design guarantees we intend to keep

- Secrets only in the OS credential store (Keychain / Credential Manager / Secret Service)
- Paper-trading endpoints only in version 1; no live-money mode exists
- All network calls go only to the brokerage hosts the user configured
- No telemetry

## Supported versions

Only the latest release receives security fixes.

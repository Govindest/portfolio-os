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

## Security assumptions

- The machine running PortfolioOS is trusted: anyone with your OS user session can use the app and, through your OS credential store, the brokerage keys it holds. PortfolioOS adds no second password layer.
- The OS credential store (Keychain / Credential Manager / Secret Service) provides at-rest protection for secrets; the SQLite database is not encrypted and contains no secrets, but does contain your portfolio data.
- TLS certificate validation is enforced for all brokerage traffic, with one deliberate exception: the Interactive Brokers Client Portal Gateway on `https://localhost:5000` uses a self-signed certificate, so verification is disabled for that localhost-only connection.
- The frontend runs in the system webview under a restrictive CSP (`default-src 'self'`) with only the minimal `core:default` Tauri capability; it can only reach the backend through the typed command list, none of which return stored secrets.
- macOS builds are currently unsigned and not notarized; Gatekeeper will warn on first launch.

## Design guarantees we intend to keep

- Secrets only in the OS credential store (Keychain / Credential Manager / Secret Service)
- Paper-trading endpoints only in version 1; no live-money mode exists
- All network calls go only to the brokerage hosts the user configured
- No telemetry

## Supported versions

Only the latest release receives security fixes.

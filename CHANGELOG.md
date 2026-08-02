# Changelog

All notable changes to PortfolioOS are documented here. This project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-08-02

First public release. Paper trading only; there is no live-money mode.

### Added

- **Local-first desktop app** (Tauri 2 + React 19 + TypeScript) with a Rust backend, SQLite persistence, and versioned migrations. No server, no account, no telemetry.
- **Broker adapter system** with a shared `BrokerAdapter` trait:
  - Demo Mode — deterministic simulated portfolio, quotes, orders, fills, cancellation, and reset.
  - Alpaca Paper — account, positions, orders, market clock, IEX quotes, market-order submission and cancellation, fractional shares, fill reconciliation (official paper API only).
  - Tradier Sandbox (Beta) — balances, positions, delayed quotes, order preview and sandbox submission.
  - Interactive Brokers (Experimental, read-only) — local Client Portal Gateway detection, balances, positions.
  - Manual portfolios — manual entry, CSV import, trade-list CSV export.
- **Credential security** — secrets stored only in the OS credential store (Keychain / Credential Manager / Secret Service), masked in the UI, and deleted on disconnect.
- **Strategies as data** — equal weight, custom allocation, and an educational momentum example; declarative YAML with a JSON Schema, import/export, and stored version history.
- **Rebalancing engine** — drift threshold, minimum order value, per-order and per-batch caps, position-weight caps, fractional rounding, open-order exclusion, cash constraints, and a plain-English explanation for every proposed order.
- **Safety controls** — mandatory typed batch approval, emergency stop, read-only mode, allow-sells and fractional toggles, market-hours and stale-price warnings, cancel-all-open-orders.
- **Crash-safe idempotent submission** — deterministic client order ids, intent persisted before the network call, duplicate submissions returned rather than re-traded.
- **Complete local audit trail** and read-only background synchronization (manual / 5 min / 15 min / hourly).
- **Documentation** — architecture, broker adapter guide, strategy schema, development, security, contributing, release checklist, and disclaimer.

### Known limitations

- Alpaca Paper is implemented and tested against canned HTTP responses, but has not been verified against the live Alpaca API by a maintainer.
- Tradier and IBKR adapters are code-reviewed only; IBKR cannot place orders.
- Market orders only — limit orders are not implemented.
- macOS bundles are unsigned and not notarized.
- No `tauri-driver` UI end-to-end suite yet.

[0.1.0]: https://github.com/Govindest/portfolio-os/releases/tag/v0.1.0

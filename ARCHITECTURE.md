# Architecture

PortfolioOS is a Tauri 2 desktop application: a React/TypeScript frontend rendered in the system webview, and a Rust backend that owns all data, credentials, and brokerage communication.

```
┌─────────────────────────────────────────────────────────────┐
│ Frontend (React + TypeScript + Tailwind + shadcn-style UI)  │
│  pages/ · components/ · TanStack Query · React Hook Form    │
└──────────────────────┬──────────────────────────────────────┘
                       │ Tauri IPC (typed commands, serde JSON)
┌──────────────────────┴──────────────────────────────────────┐
│ commands.rs — thin wrappers, no business logic              │
├─────────────────────────────────────────────────────────────┤
│ service.rs — AppService: every user-facing operation        │
│   connections · sync · dashboard · strategies · rebalance   │
│   order submission · audit · exports · demo reset           │
├──────────────┬───────────────┬──────────────┬───────────────┤
│ rebalance.rs │ strategy.rs   │ audit.rs     │ sync.rs       │
│ pure engine  │ YAML schema   │ audit trail  │ background    │
├──────────────┴───────┬───────┴──────────────┴───────────────┤
│ broker/ — BrokerAdapter trait                               │
│   mock.rs · alpaca.rs · tradier.rs · ibkr.rs                │
├──────────────────────┼──────────────────────────────────────┤
│ db.rs (SQLite, WAL,  │ credentials.rs (OS keychain via      │
│ versioned migrations)│ `keyring`; MemoryStore for tests)    │
└──────────────────────┴──────────────────────────────────────┘
```

## Key decisions

**Service layer over command logic.** `AppService` contains every operation and takes `Db` + `CredentialStore` by injection, so the complete product flow (connect → sync → strategy → rebalance → approve → fills → audit) runs headlessly in `tests/e2e.rs` against a real SQLite file and the deterministic mock broker. Tauri commands in `commands.rs` are one-liners.

**Pure rebalancing engine.** `rebalance.rs` is a pure function of `(strategy, positions/quotes/cash, safety settings) → plan`. Every rule — drift threshold, min/max order value, batch cap, position-weight cap, fractional rounding, open-order exclusion, cash constraints, sell ordering — is unit-tested without any broker.

**Broker adapters are the only place broker JSON exists.** The `BrokerAdapter` trait (see [BROKER_ADAPTERS.md](BROKER_ADAPTERS.md)) normalizes accounts, positions, orders, quotes, and errors into shared types. Adapters accept injected base URLs so tests run against canned local HTTP responses.

**Idempotent order submission.** Client order ids are deterministic per rebalance-run + side + symbol (`pos-<run8>-buy-AAPL`). Intent is written to `submitted_orders` *before* the network call, so a crash mid-submission cannot lose in-flight orders, and re-approving a submitted run returns the existing orders instead of re-trading. The mock broker also deduplicates by client id, mirroring Alpaca's behavior.

**Credentials never touch the database.** `credentials.rs` defines a `CredentialStore` trait with a `keyring`-backed production implementation (Keychain / Credential Manager / Secret Service) and an in-memory test double. SQLite stores only a display mask (`PKAB••••••••`). Disconnect deletes the OS entry first.

**Errors are typed and user-first.** `BrokerError` has one variant per failure category (authentication, rate limit, market closed, insufficient buying power, …), each carrying a `user_message` written for non-technical users plus optional `technical` detail the UI shows in an expandable section.

**SQLite with append-only migrations.** `db.rs` holds an ordered migration list applied transactionally under `PRAGMA user_version`, with foreign keys enforced and WAL enabled. All timestamps are UTC.

**Background sync is read-only by construction.** The sync loop (`sync.rs`) can only call read endpoints and the reconcile path; the submit path exists solely behind explicit user approval, the emergency-stop check, and the read-only-mode check.

**Browser dev mock.** `npm run dev` in a plain browser installs `devMock.ts` (via `@tauri-apps/api/mocks`) so the UI is fully explorable without the Rust backend. It is dev-only and excluded from desktop builds.

## Data model

Sixteen tables: `app_settings`, `onboarding_state`, `brokerage_connections`, `brokerage_accounts`, `account_snapshots`, `positions`, `position_snapshots`, `strategies`, `strategy_versions`, `strategy_assets`, `rebalance_runs`, `proposed_orders`, `submitted_orders`, `order_events`, `synchronization_runs`, `audit_events`. Strategy edits append to `strategy_versions`; nothing user-visible is hard-deleted (strategies archive, connections cascade their operational data but audit rows persist).

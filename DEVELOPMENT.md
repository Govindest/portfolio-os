# Development

## Prerequisites

- Node.js ≥ 20 and npm
- Rust ≥ 1.77 via [rustup](https://rustup.rs)
- Tauri platform prerequisites: <https://tauri.app/start/prerequisites/>
  - macOS: Xcode Command Line Tools
  - Linux: `webkit2gtk`, `libayatana-appindicator`, and the Secret Service DBus API (e.g. gnome-keyring) for credential storage
  - Windows: WebView2 (preinstalled on Windows 11) and MSVC build tools

## Everyday commands

```bash
npm install          # once
npm run tauri dev    # full desktop app with hot reload
npm run dev          # frontend only in a browser — uses a simulated backend (devMock)
npm test             # Vitest frontend tests
npm run test:rust    # cargo test: unit tests + service-level end-to-end tests
npm run lint         # ESLint (strict TS, no `any`)
npm run typecheck    # tsc --noEmit
npm run format       # Prettier
npm run tauri build  # production bundles into src-tauri/target/release/bundle/
```

Rust-side quality gates:

```bash
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Project layout

```
src/                  React frontend
  pages/              One file per sidebar destination + Onboarding
  components/         ConnectWizard, Layout, shared pieces, ui/ primitives
  lib/api.ts          Typed IPC bridge (all backend access goes through here)
  lib/devMock.ts      Browser-only simulated backend for `npm run dev`
  test/               Vitest suites
src-tauri/
  src/                Rust backend (see ARCHITECTURE.md)
  tests/e2e.rs        Headless end-to-end flows over the service layer
docs/                 JSON Schema + screenshots
```

## Testing philosophy

- The rebalancing engine and YAML validation are pure and exhaustively unit-tested.
- `tests/e2e.rs` drives the real service layer (real SQLite, mock broker, in-memory credential store) through the complete user journey, including crash-safe idempotent submission, emergency stop, disconnect-removes-credentials, and restart persistence.
- Broker HTTP behavior is tested against canned localhost responses (`one_shot_http`); no test needs real credentials or network access.
- Frontend tests mock the IPC layer with `@tauri-apps/api/mocks`.

## Where local data lives

- Database: `<app-data>/portfolio-os.sqlite3` (e.g. `~/Library/Application Support/com.portfolio-os.app/` on macOS)
- Demo broker state: `<app-data>/mock_broker_state.json`
- Credentials: OS credential store under service `com.portfolio-os.credentials`, one entry per connection id

Deleting the app-data directory resets the app; credentials must be removed via Disconnect (or your OS keychain UI).

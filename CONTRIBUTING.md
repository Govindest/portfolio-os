# Contributing to PortfolioOS

Thanks for helping! PortfolioOS aims to be the most trustworthy way to run a rules-based portfolio locally. That shapes what we accept.

## Ground rules

- **Official brokerage APIs only.** PRs adding scraping or unofficial integrations are declined.
- **No silent trading.** Any code path that could submit an order must sit behind explicit user approval, the emergency-stop check, and read-only mode.
- **No secrets outside the OS credential store.** Never write credentials to SQLite, files, logs, or the frontend.
- **Honest UI.** No placeholder buttons, no fake success states, no capability claims an adapter can't back.

## Workflow

1. Fork and branch from `main`.
2. `npm install`, then make your change.
3. Quality gates must pass locally:
   ```bash
   npm run lint && npm run typecheck && npm test && npm run test:rust
   cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings
   ```
4. Add tests for behavior you add or change. Rebalancing rules and broker error mapping especially.
5. Open a PR using the template. Keep PRs focused; separate refactors from features.

## Good first contributions

- Additional rebalance-engine edge-case tests
- Accessibility improvements (focus order, screen-reader labels)
- New broker adapters per [BROKER_ADAPTERS.md](BROKER_ADAPTERS.md)
- Documentation fixes

## Code style

- TypeScript: strict, no `any`, Prettier-formatted.
- Rust: `cargo fmt`, clippy-clean, typed errors (`BrokerError`), no `unwrap()` on external data.
- User-facing copy: plain language, no unexplained jargon; error messages say what happened *and* what to do next.

## Conduct

Be kind. See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

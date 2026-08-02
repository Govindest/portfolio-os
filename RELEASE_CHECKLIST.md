# Release checklist

Before tagging a release:

## Quality gates

- [ ] `npm run lint` clean
- [ ] `npm run typecheck` clean
- [ ] `npm test` — all frontend tests pass
- [ ] `npm run test:rust` — all Rust unit + e2e tests pass
- [ ] `cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings`
- [ ] `npm run tauri build` succeeds on macOS, Windows, and Linux (CI matrix)

## Manual smoke test (fresh app-data directory)

- [ ] First launch shows onboarding; demo mode reaches a populated dashboard in under two minutes
- [ ] Alpaca paper connect: bad key shows the friendly error; good key connects, syncs, and masks the credential
- [ ] Create equal-weight and custom strategies; import/export YAML round-trips
- [ ] Rebalance preview shows explanations; typed SUBMIT confirmation required; fills reconcile
- [ ] Emergency stop blocks submission and can cancel open orders
- [ ] Disconnect removes the credential from the OS credential store (verify in Keychain)
- [ ] Restart the app: connections, strategies, orders, and audit history persist

## Security review

- [ ] `git grep` for key-like strings (`PK[A-Z0-9]{10,}`, `sk-`, `Bearer `) returns nothing real
- [ ] No credentials in SQLite: inspect `portfolio-os.sqlite3` after connecting a broker
- [ ] Dependency audit: `npm audit` and `cargo audit` reviewed

## Publishing

- [ ] Bump version in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` (keep in sync)
- [ ] Update README roadmap/status if capabilities changed
- [ ] Tag `vX.Y.Z`; the release workflow builds and attaches installers
- [ ] Write release notes: features, fixes, known limitations

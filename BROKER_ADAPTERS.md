# Broker adapters

Every brokerage integration implements the `BrokerAdapter` trait in `src-tauri/src/broker/mod.rs`:

```rust
#[async_trait]
pub trait BrokerAdapter: Send + Sync {
    async fn test_connection(&self) -> Result<ConnectionStatus, BrokerError>;
    async fn get_accounts(&self) -> Result<Vec<BrokerAccount>, BrokerError>;
    async fn get_account_summary(&self, account_id: &str) -> Result<AccountSummary, BrokerError>;
    async fn get_positions(&self, account_id: &str) -> Result<Vec<Position>, BrokerError>;
    async fn get_orders(&self, account_id: &str) -> Result<Vec<BrokerOrder>, BrokerError>;
    async fn get_market_clock(&self) -> Result<MarketClock, BrokerError>;
    async fn get_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, BrokerError>;
    async fn validate_orders(&self, account_id: &str, orders: &[ProposedOrder])
        -> Result<Vec<ValidatedOrder>, BrokerError>;
    async fn submit_orders(&self, account_id: &str, orders: &[ValidatedOrder])
        -> Result<Vec<SubmittedOrder>, BrokerError>;
    async fn cancel_order(&self, account_id: &str, order_id: &str) -> Result<(), BrokerError>;
    fn supports_fractional(&self) -> bool { false }
    fn supports_trading(&self) -> bool { true }
}
```

Rules for adapters:

1. **Official APIs only.** No scraping, no reverse-engineered endpoints, no unofficial auth flows. If a brokerage has no official trading API, users get the manual-portfolio path instead.
2. **Broker JSON stays inside the adapter.** Only the shared types above cross the boundary.
3. **Map errors to `BrokerError` categories** with a message an ordinary user can act on; put raw status codes in the `technical` field. Never include credentials or full URLs with query strings.
4. **Inject base URLs** (`with_bases`/`with_base` constructors) so tests can point the adapter at a local canned HTTP server. No test may require real credentials.
5. **Be honest about capability.** If trading isn't implemented, return `supports_trading() == false` and a clear error — the UI labels the integration and disables submission. Never stub a fake success.
6. **Respect idempotency.** `submit_orders` receives a `client_order_id` per order; pass it to the brokerage where supported (Alpaca `client_order_id`, Tradier `tag`).

## Current adapters

| Adapter | File | Notes |
| --- | --- | --- |
| Mock | `broker/mock.rs` | Deterministic prices/positions; orders fill 3 s after submission; state persists to `mock_broker_state.json`; dedupes by client order id. |
| Alpaca | `broker/alpaca.rs` | `paper-api.alpaca.markets` + `data.alpaca.markets` (IEX feed). Paper only; market day orders; fractional supported. |
| Tradier | `broker/tradier.rs` | `sandbox.tradier.com/v1`. Preview via `preview=true`; whole shares; delayed sandbox quotes flagged stale. Beta. |
| IBKR | `broker/ibkr.rs` | Local Client Portal Gateway (`https://localhost:5000/v1/api`), self-signed cert accepted for localhost only. Read-only: `supports_trading() == false`. |

## Adding an adapter

1. Create `src-tauri/src/broker/<name>.rs` implementing the trait against the broker's official documentation.
2. Add the variant to `BrokerKind` and wire it in `AppService::build_adapter` / `preview_connection`.
3. Add a broker card + guided setup steps in `src/components/ConnectWizard.tsx`.
4. Add tests using the canned-HTTP pattern in `src-tauri/tests/e2e.rs` (`one_shot_http`).
5. Label anything incomplete honestly (Beta / Experimental) in the wizard and the connection card.

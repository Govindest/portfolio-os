//! Fully functional demo broker. Deterministic balances, positions, quotes,
//! order acceptance and fills. State persists to a JSON file inside the app
//! data directory so demo mode survives restarts; `reset` restores the seed.

use super::*;
use crate::errors::BrokerError;
use chrono::{Duration, Utc};
use std::path::PathBuf;
use std::sync::Mutex;

pub const MOCK_ACCOUNT_ID: &str = "DEMO-000001";
/// Seconds a mock order stays pending before it deterministically fills.
const FILL_DELAY_SECS: i64 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MockState {
    cash: f64,
    positions: Vec<MockPosition>,
    orders: Vec<MockOrder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MockPosition {
    symbol: String,
    quantity: f64,
    cost_basis: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MockOrder {
    id: String,
    client_order_id: String,
    symbol: String,
    side: OrderSide,
    quantity: f64,
    filled_quantity: f64,
    avg_fill_price: Option<f64>,
    status: OrderStatus,
    submitted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// Deterministic demo price table. Stable so automated tests never flake.
fn price_of(symbol: &str) -> Option<f64> {
    Some(match symbol {
        "AAPL" => 227.52,
        "MSFT" => 415.30,
        "NVDA" => 131.26,
        "GOOGL" => 178.35,
        "AMZN" => 197.93,
        "META" => 585.25,
        "TSLA" => 248.98,
        "VOO" => 512.58,
        "VTI" => 279.11,
        "BND" => 73.57,
        "QQQ" => 483.85,
        "SCHD" => 27.94,
        "VXUS" => 61.12,
        "JPM" => 224.79,
        "V" => 311.04,
        "JNJ" => 155.21,
        "UNH" => 524.66,
        "XOM" => 117.96,
        _ => return None,
    })
}

fn company_name(symbol: &str) -> Option<String> {
    let n = match symbol {
        "AAPL" => "Apple Inc.",
        "MSFT" => "Microsoft Corporation",
        "NVDA" => "NVIDIA Corporation",
        "GOOGL" => "Alphabet Inc.",
        "AMZN" => "Amazon.com, Inc.",
        "META" => "Meta Platforms, Inc.",
        "TSLA" => "Tesla, Inc.",
        "VOO" => "Vanguard S&P 500 ETF",
        "VTI" => "Vanguard Total Stock Market ETF",
        "BND" => "Vanguard Total Bond Market ETF",
        "QQQ" => "Invesco QQQ Trust",
        "SCHD" => "Schwab US Dividend Equity ETF",
        "VXUS" => "Vanguard Total International Stock ETF",
        "JPM" => "JPMorgan Chase & Co.",
        "V" => "Visa Inc.",
        "JNJ" => "Johnson & Johnson",
        "UNH" => "UnitedHealth Group",
        "XOM" => "Exxon Mobil Corporation",
        _ => return None,
    };
    Some(n.to_string())
}

fn seed_state() -> MockState {
    MockState {
        cash: 12_450.00,
        positions: vec![
            MockPosition {
                symbol: "AAPL".into(),
                quantity: 42.0,
                cost_basis: 8_105.16,
            },
            MockPosition {
                symbol: "MSFT".into(),
                quantity: 18.0,
                cost_basis: 6_642.00,
            },
            MockPosition {
                symbol: "NVDA".into(),
                quantity: 60.0,
                cost_basis: 5_460.30,
            },
            MockPosition {
                symbol: "VOO".into(),
                quantity: 25.0,
                cost_basis: 11_310.75,
            },
            MockPosition {
                symbol: "BND".into(),
                quantity: 80.0,
                cost_basis: 5_986.40,
            },
            MockPosition {
                symbol: "AMZN".into(),
                quantity: 15.0,
                cost_basis: 2_688.30,
            },
        ],
        orders: seed_orders(),
    }
}

/// A little pre-existing history so the demo Activity view is not empty.
fn seed_orders() -> Vec<MockOrder> {
    let base = Utc::now() - Duration::days(12);
    let mk = |i: i64, symbol: &str, side: OrderSide, qty: f64, price: f64| MockOrder {
        id: format!("demo-hist-{i}"),
        client_order_id: format!("demo-hist-c{i}"),
        symbol: symbol.into(),
        side,
        quantity: qty,
        filled_quantity: qty,
        avg_fill_price: Some(price),
        status: OrderStatus::Filled,
        submitted_at: base + Duration::days(i),
        updated_at: base + Duration::days(i) + Duration::minutes(2),
    };
    vec![
        mk(0, "VOO", OrderSide::Buy, 5.0, 508.11),
        mk(2, "AAPL", OrderSide::Buy, 6.0, 224.90),
        mk(5, "BND", OrderSide::Buy, 20.0, 74.02),
        mk(8, "NVDA", OrderSide::Sell, 10.0, 128.44),
    ]
}

pub struct MockBroker {
    state: Mutex<MockState>,
    state_path: Option<PathBuf>,
}

impl MockBroker {
    /// `state_path: None` keeps everything in memory (tests).
    pub fn new(state_path: Option<PathBuf>) -> Self {
        let state = state_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(seed_state);
        Self {
            state: Mutex::new(state),
            state_path,
        }
    }

    pub fn reset(&self) {
        let mut st = self.state.lock().unwrap();
        *st = seed_state();
        self.persist(&st);
    }

    fn persist(&self, st: &MockState) {
        if let Some(p) = &self.state_path {
            if let Ok(json) = serde_json::to_string_pretty(st) {
                let _ = std::fs::write(p, json);
            }
        }
    }

    /// Advance pending orders past the fill delay into deterministic fills.
    fn settle(&self, st: &mut MockState) {
        let now = Utc::now();
        let mut fills: Vec<(String, OrderSide, f64, f64)> = vec![];
        for o in st.orders.iter_mut() {
            let pending = matches!(o.status, OrderStatus::Pending | OrderStatus::Accepted);
            if pending && (now - o.submitted_at).num_seconds() >= FILL_DELAY_SECS {
                let price = price_of(&o.symbol).unwrap_or(100.0);
                o.status = OrderStatus::Filled;
                o.filled_quantity = o.quantity;
                o.avg_fill_price = Some(price);
                o.updated_at = now;
                fills.push((o.symbol.clone(), o.side, o.quantity, price));
            }
        }
        for (symbol, side, qty, price) in fills {
            let value = qty * price;
            match side {
                OrderSide::Buy => {
                    st.cash -= value;
                    if let Some(p) = st.positions.iter_mut().find(|p| p.symbol == symbol) {
                        p.quantity += qty;
                        p.cost_basis += value;
                    } else {
                        st.positions.push(MockPosition {
                            symbol,
                            quantity: qty,
                            cost_basis: value,
                        });
                    }
                }
                OrderSide::Sell => {
                    st.cash += value;
                    if let Some(p) = st.positions.iter_mut().find(|p| p.symbol == symbol) {
                        let unit_cost = if p.quantity > 0.0 {
                            p.cost_basis / p.quantity
                        } else {
                            0.0
                        };
                        p.quantity -= qty;
                        p.cost_basis -= unit_cost * qty;
                    }
                }
            }
        }
        st.positions.retain(|p| p.quantity > 1e-9);
        self.persist(st);
    }

    fn check_account(account_id: &str) -> Result<(), BrokerError> {
        if account_id == MOCK_ACCOUNT_ID {
            Ok(())
        } else {
            Err(BrokerError::validation("Unknown demo account."))
        }
    }
}

#[async_trait]
impl BrokerAdapter for MockBroker {
    async fn test_connection(&self) -> Result<ConnectionStatus, BrokerError> {
        Ok(ConnectionStatus {
            connected: true,
            paper: true,
            message: "Demo mode is ready. No external account required.".into(),
        })
    }

    async fn get_accounts(&self) -> Result<Vec<BrokerAccount>, BrokerError> {
        Ok(vec![BrokerAccount {
            id: MOCK_ACCOUNT_ID.into(),
            name: "Demo Portfolio".into(),
            paper: true,
            currency: "USD".into(),
        }])
    }

    async fn get_account_summary(&self, account_id: &str) -> Result<AccountSummary, BrokerError> {
        Self::check_account(account_id)?;
        let mut st = self.state.lock().unwrap();
        self.settle(&mut st);
        let positions_value: f64 = st
            .positions
            .iter()
            .map(|p| p.quantity * price_of(&p.symbol).unwrap_or(0.0))
            .sum();
        Ok(AccountSummary {
            account_id: account_id.into(),
            portfolio_value: positions_value + st.cash,
            cash: st.cash,
            buying_power: st.cash,
            currency: "USD".into(),
            daily_change_percent: Some(0.42),
        })
    }

    async fn get_positions(&self, account_id: &str) -> Result<Vec<Position>, BrokerError> {
        Self::check_account(account_id)?;
        let mut st = self.state.lock().unwrap();
        self.settle(&mut st);
        Ok(st
            .positions
            .iter()
            .map(|p| {
                let price = price_of(&p.symbol).unwrap_or(0.0);
                let market_value = p.quantity * price;
                Position {
                    symbol: p.symbol.clone(),
                    name: company_name(&p.symbol),
                    quantity: p.quantity,
                    current_price: price,
                    market_value,
                    cost_basis: Some(p.cost_basis),
                    unrealized_pl: Some(market_value - p.cost_basis),
                }
            })
            .collect())
    }

    async fn get_orders(&self, account_id: &str) -> Result<Vec<BrokerOrder>, BrokerError> {
        Self::check_account(account_id)?;
        let mut st = self.state.lock().unwrap();
        self.settle(&mut st);
        let mut orders: Vec<BrokerOrder> = st
            .orders
            .iter()
            .map(|o| BrokerOrder {
                id: o.id.clone(),
                symbol: o.symbol.clone(),
                side: o.side,
                quantity: o.quantity,
                filled_quantity: o.filled_quantity,
                avg_fill_price: o.avg_fill_price,
                status: o.status,
                submitted_at: o.submitted_at,
                updated_at: o.updated_at,
            })
            .collect();
        orders.sort_by_key(|o| std::cmp::Reverse(o.submitted_at));
        Ok(orders)
    }

    async fn get_market_clock(&self) -> Result<MarketClock, BrokerError> {
        // Demo market is always open so the full flow can be exercised anytime.
        Ok(MarketClock {
            is_open: true,
            next_open: None,
            next_close: None,
        })
    }

    async fn get_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, BrokerError> {
        let now = Utc::now();
        Ok(symbols
            .iter()
            .filter_map(|s| {
                price_of(s).map(|price| Quote {
                    symbol: s.clone(),
                    price,
                    as_of: now,
                    stale: false,
                })
            })
            .collect())
    }

    async fn validate_orders(
        &self,
        account_id: &str,
        orders: &[ProposedOrder],
    ) -> Result<Vec<ValidatedOrder>, BrokerError> {
        Self::check_account(account_id)?;
        let st = self.state.lock().unwrap();
        let mut cash = st.cash;
        Ok(orders
            .iter()
            .map(|o| {
                let mut valid = true;
                let mut msg = None;
                if price_of(&o.symbol).is_none() {
                    valid = false;
                    msg = Some(format!("{} is not available in demo mode.", o.symbol));
                } else if o.quantity <= 0.0 {
                    valid = false;
                    msg = Some("Quantity must be positive.".into());
                } else if o.side == OrderSide::Buy {
                    if o.estimated_value > cash {
                        valid = false;
                        msg = Some("Not enough demo cash for this buy.".into());
                    } else {
                        cash -= o.estimated_value;
                    }
                } else if o.side == OrderSide::Sell {
                    let held = st
                        .positions
                        .iter()
                        .find(|p| p.symbol == o.symbol)
                        .map(|p| p.quantity)
                        .unwrap_or(0.0);
                    if o.quantity > held + 1e-9 {
                        valid = false;
                        msg = Some(format!("You only hold {held} shares of {}.", o.symbol));
                    }
                }
                ValidatedOrder {
                    proposed: o.clone(),
                    valid,
                    validation_message: msg,
                    client_order_id: String::new(), // assigned by the service layer
                }
            })
            .collect())
    }

    async fn submit_orders(
        &self,
        account_id: &str,
        orders: &[ValidatedOrder],
    ) -> Result<Vec<SubmittedOrder>, BrokerError> {
        Self::check_account(account_id)?;
        let mut st = self.state.lock().unwrap();
        let now = Utc::now();
        let mut out = vec![];
        for v in orders {
            // Idempotency: a client_order_id we have already seen is returned, not re-created.
            if let Some(existing) = st
                .orders
                .iter()
                .find(|o| o.client_order_id == v.client_order_id)
            {
                out.push(SubmittedOrder {
                    client_order_id: v.client_order_id.clone(),
                    broker_order_id: Some(existing.id.clone()),
                    symbol: existing.symbol.clone(),
                    side: existing.side,
                    quantity: existing.quantity,
                    status: existing.status,
                    message: Some("Duplicate submission ignored.".into()),
                });
                continue;
            }
            if !v.valid {
                out.push(SubmittedOrder {
                    client_order_id: v.client_order_id.clone(),
                    broker_order_id: None,
                    symbol: v.proposed.symbol.clone(),
                    side: v.proposed.side,
                    quantity: v.proposed.quantity,
                    status: OrderStatus::Rejected,
                    message: v.validation_message.clone(),
                });
                continue;
            }
            let id = format!("demo-{}", uuid::Uuid::new_v4());
            st.orders.push(MockOrder {
                id: id.clone(),
                client_order_id: v.client_order_id.clone(),
                symbol: v.proposed.symbol.clone(),
                side: v.proposed.side,
                quantity: v.proposed.quantity,
                filled_quantity: 0.0,
                avg_fill_price: None,
                status: OrderStatus::Accepted,
                submitted_at: now,
                updated_at: now,
            });
            out.push(SubmittedOrder {
                client_order_id: v.client_order_id.clone(),
                broker_order_id: Some(id),
                symbol: v.proposed.symbol.clone(),
                side: v.proposed.side,
                quantity: v.proposed.quantity,
                status: OrderStatus::Accepted,
                message: None,
            });
        }
        self.persist(&st);
        Ok(out)
    }

    async fn cancel_order(&self, account_id: &str, order_id: &str) -> Result<(), BrokerError> {
        Self::check_account(account_id)?;
        let mut st = self.state.lock().unwrap();
        let Some(o) = st.orders.iter_mut().find(|o| o.id == order_id) else {
            return Err(BrokerError::validation("Order not found."));
        };
        match o.status {
            OrderStatus::Pending | OrderStatus::Accepted => {
                o.status = OrderStatus::Canceled;
                o.updated_at = Utc::now();
                self.persist(&st);
                Ok(())
            }
            _ => Err(BrokerError::validation(
                "This order can no longer be canceled.",
            )),
        }
    }

    fn supports_fractional(&self) -> bool {
        true
    }
}

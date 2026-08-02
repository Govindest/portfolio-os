//! Alpaca paper-trading adapter. Official REST API only:
//! https://docs.alpaca.markets/reference — trading via paper-api.alpaca.markets,
//! quotes via data.alpaca.markets. Live-trading endpoints are intentionally
//! not supported in version 1.

use super::*;
use crate::errors::{BrokerError, ErrorDetail};
use serde_json::json;

const TRADING_BASE: &str = "https://paper-api.alpaca.markets";
const DATA_BASE: &str = "https://data.alpaca.markets";

pub struct AlpacaBroker {
    client: reqwest::Client,
    key: String,
    secret: String,
    trading_base: String,
    data_base: String,
}

impl AlpacaBroker {
    pub fn new(key: String, secret: String) -> Self {
        Self::with_bases(key, secret, TRADING_BASE.into(), DATA_BASE.into())
    }

    /// Base URLs injectable so tests can point at a local mock server.
    pub fn with_bases(
        key: String,
        secret: String,
        trading_base: String,
        data_base: String,
    ) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("http client"),
            key,
            secret,
            trading_base,
            data_base,
        }
    }

    fn req(&self, method: reqwest::Method, url: String) -> reqwest::RequestBuilder {
        self.client
            .request(method, url)
            .header("APCA-API-KEY-ID", &self.key)
            .header("APCA-API-SECRET-KEY", &self.secret)
    }

    async fn handle(resp: reqwest::Response) -> Result<serde_json::Value, BrokerError> {
        let status = resp.status();
        let body: serde_json::Value = resp.json().await.unwrap_or(json!({}));
        if status.is_success() {
            return Ok(body);
        }
        let tech = format!("HTTP {status}: {}", body["message"].as_str().unwrap_or(""));
        Err(match status.as_u16() {
            401 => BrokerError::Authentication(ErrorDetail {
                user_message: "The API key was not accepted. Confirm that you copied your paper-trading credentials from the Alpaca dashboard.".into(),
                technical: Some(tech),
            }),
            403 => {
                let msg = body["message"].as_str().unwrap_or("");
                if msg.contains("buying power") {
                    BrokerError::InsufficientBuyingPower(ErrorDetail {
                        user_message: "The account does not have enough buying power for this order.".into(),
                        technical: Some(tech),
                    })
                } else {
                    BrokerError::Authorization(ErrorDetail {
                        user_message: "PortfolioOS reached Alpaca, but the secret appears incorrect or the key lacks permission.".into(),
                        technical: Some(tech),
                    })
                }
            }
            422 => BrokerError::InvalidRequest(ErrorDetail {
                user_message: "Alpaca rejected the request as invalid.".into(),
                technical: Some(tech),
            }),
            429 => BrokerError::RateLimit(ErrorDetail {
                user_message: "Alpaca is rate limiting requests. Wait a moment and try again.".into(),
                technical: Some(tech),
            }),
            500..=599 => BrokerError::BrokerageUnavailable(ErrorDetail {
                user_message: "The brokerage service is temporarily unavailable. Your credentials were not changed.".into(),
                technical: Some(tech),
            }),
            _ => BrokerError::UnknownResponse(ErrorDetail {
                user_message: "Alpaca returned an unexpected response.".into(),
                technical: Some(tech),
            }),
        })
    }

    fn map_status(s: &str) -> OrderStatus {
        match s {
            "new" | "accepted" | "pending_new" | "accepted_for_bidding" => OrderStatus::Accepted,
            "partially_filled" => OrderStatus::PartiallyFilled,
            "filled" => OrderStatus::Filled,
            "rejected" | "expired" | "stopped" | "suspended" => OrderStatus::Rejected,
            "canceled" | "pending_cancel" | "done_for_day" | "replaced" => OrderStatus::Canceled,
            _ => OrderStatus::Pending,
        }
    }

    fn parse_order(o: &serde_json::Value) -> Option<BrokerOrder> {
        let f = |v: &serde_json::Value| v.as_str().and_then(|s| s.parse::<f64>().ok());
        Some(BrokerOrder {
            id: o["id"].as_str()?.to_string(),
            symbol: o["symbol"].as_str()?.to_string(),
            side: OrderSide::parse(o["side"].as_str()?)?,
            quantity: f(&o["qty"]).or_else(|| f(&o["notional"])).unwrap_or(0.0),
            filled_quantity: f(&o["filled_qty"]).unwrap_or(0.0),
            avg_fill_price: f(&o["filled_avg_price"]),
            status: Self::map_status(o["status"].as_str().unwrap_or("")),
            submitted_at: o["submitted_at"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(Utc::now),
            updated_at: o["updated_at"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(Utc::now),
        })
    }
}

#[async_trait]
impl BrokerAdapter for AlpacaBroker {
    async fn test_connection(&self) -> Result<ConnectionStatus, BrokerError> {
        let url = format!("{}/v2/account", self.trading_base);
        let body = Self::handle(self.req(reqwest::Method::GET, url).send().await?).await?;
        // Paper accounts on paper-api always are paper; double-check the flag if present.
        let live = body["account_number"].as_str().is_some()
            && body["pattern_day_trader"].is_boolean()
            && body["is_paper"] == json!(false);
        if live {
            return Err(BrokerError::Validation(ErrorDetail {
                user_message: "This account is configured for live trading. PortfolioOS version 1 supports paper trading only.".into(),
                technical: None,
            }));
        }
        Ok(ConnectionStatus {
            connected: true,
            paper: true,
            message: "Connected to Alpaca paper trading.".into(),
        })
    }

    async fn get_accounts(&self) -> Result<Vec<BrokerAccount>, BrokerError> {
        let url = format!("{}/v2/account", self.trading_base);
        let body = Self::handle(self.req(reqwest::Method::GET, url).send().await?).await?;
        let id = body["account_number"]
            .as_str()
            .or(body["id"].as_str())
            .unwrap_or("alpaca-paper")
            .to_string();
        Ok(vec![BrokerAccount {
            id,
            name: "Alpaca Paper Account".into(),
            paper: true,
            currency: body["currency"].as_str().unwrap_or("USD").into(),
        }])
    }

    async fn get_account_summary(&self, account_id: &str) -> Result<AccountSummary, BrokerError> {
        let url = format!("{}/v2/account", self.trading_base);
        let body = Self::handle(self.req(reqwest::Method::GET, url).send().await?).await?;
        let f = |k: &str| {
            body[k]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        let equity = f("equity");
        let last_equity = f("last_equity");
        Ok(AccountSummary {
            account_id: account_id.into(),
            portfolio_value: equity,
            cash: f("cash"),
            buying_power: f("buying_power"),
            currency: body["currency"].as_str().unwrap_or("USD").into(),
            daily_change_percent: if last_equity > 0.0 {
                Some((equity - last_equity) / last_equity * 100.0)
            } else {
                None
            },
        })
    }

    async fn get_positions(&self, _account_id: &str) -> Result<Vec<Position>, BrokerError> {
        let url = format!("{}/v2/positions", self.trading_base);
        let body = Self::handle(self.req(reqwest::Method::GET, url).send().await?).await?;
        let arr = body.as_array().cloned().unwrap_or_default();
        Ok(arr
            .iter()
            .filter_map(|p| {
                let f = |k: &str| p[k].as_str().and_then(|s| s.parse::<f64>().ok());
                Some(Position {
                    symbol: p["symbol"].as_str()?.to_string(),
                    name: None,
                    quantity: f("qty")?,
                    current_price: f("current_price").unwrap_or(0.0),
                    market_value: f("market_value").unwrap_or(0.0),
                    cost_basis: f("cost_basis"),
                    unrealized_pl: f("unrealized_pl"),
                })
            })
            .collect())
    }

    async fn get_orders(&self, _account_id: &str) -> Result<Vec<BrokerOrder>, BrokerError> {
        let url = format!(
            "{}/v2/orders?status=all&limit=100&direction=desc",
            self.trading_base
        );
        let body = Self::handle(self.req(reqwest::Method::GET, url).send().await?).await?;
        Ok(body
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(Self::parse_order)
            .collect())
    }

    async fn get_market_clock(&self) -> Result<MarketClock, BrokerError> {
        let url = format!("{}/v2/clock", self.trading_base);
        let body = Self::handle(self.req(reqwest::Method::GET, url).send().await?).await?;
        Ok(MarketClock {
            is_open: body["is_open"].as_bool().unwrap_or(false),
            next_open: body["next_open"].as_str().and_then(|s| s.parse().ok()),
            next_close: body["next_close"].as_str().and_then(|s| s.parse().ok()),
        })
    }

    async fn get_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, BrokerError> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }
        let url = format!(
            "{}/v2/stocks/trades/latest?symbols={}&feed=iex",
            self.data_base,
            symbols.join(",")
        );
        let body = Self::handle(self.req(reqwest::Method::GET, url).send().await?).await?;
        let now = Utc::now();
        let mut out = vec![];
        if let Some(map) = body["trades"].as_object() {
            for (symbol, t) in map {
                let price = t["p"].as_f64().unwrap_or(0.0);
                let as_of: DateTime<Utc> =
                    t["t"].as_str().and_then(|s| s.parse().ok()).unwrap_or(now);
                out.push(Quote {
                    symbol: symbol.clone(),
                    price,
                    as_of,
                    // Older than 15 minutes counts as stale for rebalancing.
                    stale: (now - as_of).num_minutes() > 15,
                });
            }
        }
        Ok(out)
    }

    async fn validate_orders(
        &self,
        account_id: &str,
        orders: &[ProposedOrder],
    ) -> Result<Vec<ValidatedOrder>, BrokerError> {
        // Alpaca has no dry-run endpoint; validate locally against account state.
        let summary = self.get_account_summary(account_id).await?;
        let positions = self.get_positions(account_id).await?;
        let mut cash = summary.buying_power;
        Ok(orders
            .iter()
            .map(|o| {
                let mut valid = true;
                let mut msg = None;
                if o.quantity <= 0.0 {
                    valid = false;
                    msg = Some("Quantity must be positive.".into());
                } else if o.side == OrderSide::Buy {
                    if o.estimated_value > cash {
                        valid = false;
                        msg = Some("Estimated cost exceeds available buying power.".into());
                    } else {
                        cash -= o.estimated_value;
                    }
                } else {
                    let held = positions
                        .iter()
                        .find(|p| p.symbol == o.symbol)
                        .map(|p| p.quantity)
                        .unwrap_or(0.0);
                    if o.quantity > held + 1e-9 {
                        valid = false;
                        msg = Some(format!("Position only holds {held} shares."));
                    }
                }
                ValidatedOrder {
                    proposed: o.clone(),
                    valid,
                    validation_message: msg,
                    client_order_id: String::new(),
                }
            })
            .collect())
    }

    async fn submit_orders(
        &self,
        _account_id: &str,
        orders: &[ValidatedOrder],
    ) -> Result<Vec<SubmittedOrder>, BrokerError> {
        let mut out = vec![];
        for v in orders {
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
            let payload = json!({
                "symbol": v.proposed.symbol,
                "qty": format!("{}", v.proposed.quantity),
                "side": v.proposed.side.as_str(),
                "type": "market",
                "time_in_force": "day",
                "client_order_id": v.client_order_id,
            });
            let url = format!("{}/v2/orders", self.trading_base);
            let resp = self
                .req(reqwest::Method::POST, url)
                .json(&payload)
                .send()
                .await?;
            match Self::handle(resp).await {
                Ok(body) => out.push(SubmittedOrder {
                    client_order_id: v.client_order_id.clone(),
                    broker_order_id: body["id"].as_str().map(String::from),
                    symbol: v.proposed.symbol.clone(),
                    side: v.proposed.side,
                    quantity: v.proposed.quantity,
                    status: Self::map_status(body["status"].as_str().unwrap_or("")),
                    message: None,
                }),
                Err(e) => out.push(SubmittedOrder {
                    client_order_id: v.client_order_id.clone(),
                    broker_order_id: None,
                    symbol: v.proposed.symbol.clone(),
                    side: v.proposed.side,
                    quantity: v.proposed.quantity,
                    status: OrderStatus::Rejected,
                    message: Some(e.user_message()),
                }),
            }
        }
        Ok(out)
    }

    async fn cancel_order(&self, _account_id: &str, order_id: &str) -> Result<(), BrokerError> {
        let url = format!("{}/v2/orders/{order_id}", self.trading_base);
        let resp = self.req(reqwest::Method::DELETE, url).send().await?;
        if resp.status().is_success() || resp.status().as_u16() == 204 {
            Ok(())
        } else {
            Self::handle(resp).await.map(|_| ())
        }
    }

    fn supports_fractional(&self) -> bool {
        true
    }
}

//! Tradier sandbox adapter (Beta). Official sandbox API only:
//! https://documentation.tradier.com — base sandbox.tradier.com/v1 with a
//! user-supplied sandbox access token. Labeled Beta in the UI: sandbox quotes
//! are 15-minute delayed and some accounts lack order support.

use super::*;
use crate::errors::{BrokerError, ErrorDetail};
use serde_json::json;

const SANDBOX_BASE: &str = "https://sandbox.tradier.com/v1";

pub struct TradierBroker {
    client: reqwest::Client,
    token: String,
    base: String,
}

impl TradierBroker {
    pub fn new(token: String) -> Self {
        Self::with_base(token, SANDBOX_BASE.into())
    }

    pub fn with_base(token: String, base: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("http client"),
            token,
            base,
        }
    }

    fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, format!("{}{}", self.base, path))
            .bearer_auth(&self.token)
            .header("Accept", "application/json")
    }

    async fn handle(resp: reqwest::Response) -> Result<serde_json::Value, BrokerError> {
        let status = resp.status();
        let body: serde_json::Value = resp.json().await.unwrap_or(json!({}));
        if status.is_success() {
            return Ok(body);
        }
        let tech = format!("HTTP {status}");
        Err(match status.as_u16() {
            401 => BrokerError::Authentication(ErrorDetail {
                user_message: "Tradier did not accept the access token. Confirm you copied your sandbox token.".into(),
                technical: Some(tech),
            }),
            429 => BrokerError::RateLimit(ErrorDetail {
                user_message: "Tradier is rate limiting requests. Wait a moment and try again.".into(),
                technical: Some(tech),
            }),
            500..=599 => BrokerError::BrokerageUnavailable(ErrorDetail {
                user_message: "Tradier is temporarily unavailable. Your credentials were not changed.".into(),
                technical: Some(tech),
            }),
            _ => BrokerError::UnknownResponse(ErrorDetail {
                user_message: "Tradier returned an unexpected response.".into(),
                technical: Some(tech),
            }),
        })
    }

    fn map_status(s: &str) -> OrderStatus {
        match s {
            "open" | "pending" | "submitted" | "calculated" | "accepted" => OrderStatus::Accepted,
            "partially_filled" => OrderStatus::PartiallyFilled,
            "filled" => OrderStatus::Filled,
            "rejected" | "error" | "expired" => OrderStatus::Rejected,
            "canceled" => OrderStatus::Canceled,
            _ => OrderStatus::Pending,
        }
    }
}

#[async_trait]
impl BrokerAdapter for TradierBroker {
    async fn test_connection(&self) -> Result<ConnectionStatus, BrokerError> {
        let body = Self::handle(
            self.req(reqwest::Method::GET, "/user/profile")
                .send()
                .await?,
        )
        .await?;
        if body["profile"].is_object() {
            Ok(ConnectionStatus {
                connected: true,
                paper: true,
                message: "Connected to Tradier sandbox.".into(),
            })
        } else {
            Err(BrokerError::UnknownResponse(ErrorDetail {
                user_message: "Tradier returned an unexpected profile response.".into(),
                technical: Some(body.to_string().chars().take(300).collect()),
            }))
        }
    }

    async fn get_accounts(&self) -> Result<Vec<BrokerAccount>, BrokerError> {
        let body = Self::handle(
            self.req(reqwest::Method::GET, "/user/profile")
                .send()
                .await?,
        )
        .await?;
        let accounts = &body["profile"]["account"];
        let list: Vec<serde_json::Value> = if accounts.is_array() {
            accounts.as_array().unwrap().clone()
        } else if accounts.is_object() {
            vec![accounts.clone()]
        } else {
            vec![]
        };
        Ok(list
            .iter()
            .filter_map(|a| {
                Some(BrokerAccount {
                    id: a["account_number"].as_str()?.to_string(),
                    name: format!("Tradier {}", a["type"].as_str().unwrap_or("account")),
                    paper: true,
                    currency: "USD".into(),
                })
            })
            .collect())
    }

    async fn get_account_summary(&self, account_id: &str) -> Result<AccountSummary, BrokerError> {
        let path = format!("/accounts/{account_id}/balances");
        let body = Self::handle(self.req(reqwest::Method::GET, &path).send().await?).await?;
        let b = &body["balances"];
        let f = |v: &serde_json::Value| v.as_f64().unwrap_or(0.0);
        let cash = f(&b["total_cash"]);
        Ok(AccountSummary {
            account_id: account_id.into(),
            portfolio_value: f(&b["total_equity"]),
            cash,
            buying_power: b["margin"]["stock_buying_power"]
                .as_f64()
                .or(b["cash"]["cash_available"].as_f64())
                .unwrap_or(cash),
            currency: "USD".into(),
            daily_change_percent: None,
        })
    }

    async fn get_positions(&self, account_id: &str) -> Result<Vec<Position>, BrokerError> {
        let path = format!("/accounts/{account_id}/positions");
        let body = Self::handle(self.req(reqwest::Method::GET, &path).send().await?).await?;
        let positions = &body["positions"]["position"];
        let list: Vec<serde_json::Value> = if positions.is_array() {
            positions.as_array().unwrap().clone()
        } else if positions.is_object() {
            vec![positions.clone()]
        } else {
            vec![]
        };
        // Tradier positions carry cost basis but not price; fetch quotes for value.
        let symbols: Vec<String> = list
            .iter()
            .filter_map(|p| p["symbol"].as_str().map(String::from))
            .collect();
        let quotes = self.get_quotes(&symbols).await.unwrap_or_default();
        Ok(list
            .iter()
            .filter_map(|p| {
                let symbol = p["symbol"].as_str()?.to_string();
                let quantity = p["quantity"].as_f64()?;
                let price = quotes
                    .iter()
                    .find(|q| q.symbol == symbol)
                    .map(|q| q.price)
                    .unwrap_or(0.0);
                let cost = p["cost_basis"].as_f64();
                let market_value = quantity * price;
                Some(Position {
                    symbol,
                    name: None,
                    quantity,
                    current_price: price,
                    market_value,
                    cost_basis: cost,
                    unrealized_pl: cost.map(|c| market_value - c),
                })
            })
            .collect())
    }

    async fn get_orders(&self, account_id: &str) -> Result<Vec<BrokerOrder>, BrokerError> {
        let path = format!("/accounts/{account_id}/orders");
        let body = Self::handle(self.req(reqwest::Method::GET, &path).send().await?).await?;
        let orders = &body["orders"]["order"];
        let list: Vec<serde_json::Value> = if orders.is_array() {
            orders.as_array().unwrap().clone()
        } else if orders.is_object() {
            vec![orders.clone()]
        } else {
            vec![]
        };
        Ok(list
            .iter()
            .filter_map(|o| {
                Some(BrokerOrder {
                    id: o["id"]
                        .as_i64()
                        .map(|i| i.to_string())
                        .or(o["id"].as_str().map(String::from))?,
                    symbol: o["symbol"].as_str()?.to_string(),
                    side: OrderSide::parse(o["side"].as_str()?)?,
                    quantity: o["quantity"].as_f64().unwrap_or(0.0),
                    filled_quantity: o["exec_quantity"].as_f64().unwrap_or(0.0),
                    avg_fill_price: o["avg_fill_price"].as_f64().filter(|p| *p > 0.0),
                    status: Self::map_status(o["status"].as_str().unwrap_or("")),
                    submitted_at: o["create_date"]
                        .as_str()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or_else(Utc::now),
                    updated_at: o["transaction_date"]
                        .as_str()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or_else(Utc::now),
                })
            })
            .collect())
    }

    async fn get_market_clock(&self) -> Result<MarketClock, BrokerError> {
        let body = Self::handle(
            self.req(reqwest::Method::GET, "/markets/clock")
                .send()
                .await?,
        )
        .await?;
        let state = body["clock"]["state"].as_str().unwrap_or("closed");
        Ok(MarketClock {
            is_open: state == "open",
            next_open: None,
            next_close: None,
        })
    }

    async fn get_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, BrokerError> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }
        let path = format!("/markets/quotes?symbols={}", symbols.join(","));
        let body = Self::handle(self.req(reqwest::Method::GET, &path).send().await?).await?;
        let quotes = &body["quotes"]["quote"];
        let list: Vec<serde_json::Value> = if quotes.is_array() {
            quotes.as_array().unwrap().clone()
        } else if quotes.is_object() {
            vec![quotes.clone()]
        } else {
            vec![]
        };
        let now = Utc::now();
        Ok(list
            .iter()
            .filter_map(|q| {
                Some(Quote {
                    symbol: q["symbol"].as_str()?.to_string(),
                    price: q["last"].as_f64().or(q["prevclose"].as_f64())?,
                    as_of: now,
                    // Sandbox market data is delayed; always flag as potentially stale.
                    stale: true,
                })
            })
            .collect())
    }

    async fn validate_orders(
        &self,
        account_id: &str,
        orders: &[ProposedOrder],
    ) -> Result<Vec<ValidatedOrder>, BrokerError> {
        // Tradier supports order preview: POST with preview=true.
        let mut out = vec![];
        for o in orders {
            let path = format!("/accounts/{account_id}/orders");
            let form = [
                ("class", "equity".to_string()),
                ("symbol", o.symbol.clone()),
                ("side", o.side.as_str().to_string()),
                ("quantity", format!("{}", o.quantity)),
                ("type", "market".to_string()),
                ("duration", "day".to_string()),
                ("preview", "true".to_string()),
            ];
            let resp = self
                .req(reqwest::Method::POST, &path)
                .form(&form)
                .send()
                .await?;
            match Self::handle(resp).await {
                Ok(body) => {
                    let ok = body["order"]["status"].as_str() == Some("ok");
                    out.push(ValidatedOrder {
                        proposed: o.clone(),
                        valid: ok,
                        validation_message: if ok {
                            None
                        } else {
                            Some("Tradier preview rejected this order.".into())
                        },
                        client_order_id: String::new(),
                    });
                }
                Err(e) => out.push(ValidatedOrder {
                    proposed: o.clone(),
                    valid: false,
                    validation_message: Some(e.user_message()),
                    client_order_id: String::new(),
                }),
            }
        }
        Ok(out)
    }

    async fn submit_orders(
        &self,
        account_id: &str,
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
            let path = format!("/accounts/{account_id}/orders");
            let form = [
                ("class", "equity".to_string()),
                ("symbol", v.proposed.symbol.clone()),
                ("side", v.proposed.side.as_str().to_string()),
                ("quantity", format!("{}", v.proposed.quantity)),
                ("type", "market".to_string()),
                ("duration", "day".to_string()),
                (
                    "tag",
                    v.client_order_id
                        .replace('-', "")
                        .chars()
                        .take(255)
                        .collect::<String>(),
                ),
            ];
            let resp = self
                .req(reqwest::Method::POST, &path)
                .form(&form)
                .send()
                .await?;
            match Self::handle(resp).await {
                Ok(body) => out.push(SubmittedOrder {
                    client_order_id: v.client_order_id.clone(),
                    broker_order_id: body["order"]["id"].as_i64().map(|i| i.to_string()),
                    symbol: v.proposed.symbol.clone(),
                    side: v.proposed.side,
                    quantity: v.proposed.quantity,
                    status: OrderStatus::Accepted,
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

    async fn cancel_order(&self, account_id: &str, order_id: &str) -> Result<(), BrokerError> {
        let path = format!("/accounts/{account_id}/orders/{order_id}");
        let resp = self.req(reqwest::Method::DELETE, &path).send().await?;
        Self::handle(resp).await.map(|_| ())
    }

    fn supports_fractional(&self) -> bool {
        // Tradier equity orders are whole-share only.
        false
    }
}

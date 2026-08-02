//! Interactive Brokers adapter (Experimental, read-only).
//!
//! Talks to a locally running Client Portal Gateway
//! (https://www.interactivebrokers.com/campus/ibkr-api-page/cpapi-v1/) on
//! https://localhost:5000. The gateway uses a self-signed certificate, so
//! certificate verification is disabled for this localhost-only connection.
//! Authentication happens inside the gateway's own browser login; PortfolioOS
//! never sees IBKR credentials.
//!
//! Version 1 is read-only: accounts, balances, and positions. Order
//! submission through the gateway is NOT implemented — the trait methods
//! return honest "not supported" errors, and the UI labels the integration
//! Experimental.

use super::*;
use crate::errors::{BrokerError, ErrorDetail};

const DEFAULT_BASE: &str = "https://localhost:5000/v1/api";

pub struct IbkrBroker {
    client: reqwest::Client,
    base: String,
}

impl IbkrBroker {
    pub fn new(gateway_url: Option<String>) -> Self {
        let base = gateway_url.unwrap_or_else(|| DEFAULT_BASE.to_string());
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                // Localhost-only gateway ships a self-signed certificate.
                .danger_accept_invalid_certs(true)
                .build()
                .expect("http client"),
            base,
        }
    }

    fn unsupported() -> BrokerError {
        BrokerError::Validation(ErrorDetail {
            user_message: "The Interactive Brokers integration is read-only in this version. Trading through the Client Portal Gateway is not yet supported.".into(),
            technical: None,
        })
    }
}

#[async_trait]
impl BrokerAdapter for IbkrBroker {
    async fn test_connection(&self) -> Result<ConnectionStatus, BrokerError> {
        let url = format!("{}/iserver/auth/status", self.base);
        let resp = self.client.post(&url).send().await.map_err(|_| {
            BrokerError::Network(ErrorDetail {
                user_message: "No Client Portal Gateway found on this computer. Start the gateway, sign in through its browser window, then try again.".into(),
                technical: Some(format!("POST {url} failed")),
            })
        })?;
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        let authenticated = body["authenticated"].as_bool().unwrap_or(false);
        if authenticated {
            Ok(ConnectionStatus {
                connected: true,
                paper: true,
                message: "Connected to the local IBKR Client Portal Gateway (read-only).".into(),
            })
        } else {
            Err(BrokerError::Authentication(ErrorDetail {
                user_message: "The gateway is running but not signed in. Open the gateway in your browser and complete the IBKR login.".into(),
                technical: None,
            }))
        }
    }

    async fn get_accounts(&self) -> Result<Vec<BrokerAccount>, BrokerError> {
        let url = format!("{}/portfolio/accounts", self.base);
        let body: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        Ok(body
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|a| {
                let id = a["accountId"].as_str().or(a["id"].as_str())?;
                Some(BrokerAccount {
                    id: id.to_string(),
                    name: a["accountTitle"]
                        .as_str()
                        .unwrap_or("IBKR Account")
                        .to_string(),
                    // Paper account ids start with D.
                    paper: id.starts_with('D'),
                    currency: a["currency"].as_str().unwrap_or("USD").into(),
                })
            })
            .collect())
    }

    async fn get_account_summary(&self, account_id: &str) -> Result<AccountSummary, BrokerError> {
        let url = format!("{}/portfolio/{account_id}/ledger", self.base);
        let body: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        let base = &body["BASE"];
        Ok(AccountSummary {
            account_id: account_id.into(),
            portfolio_value: base["netliquidationvalue"].as_f64().unwrap_or(0.0),
            cash: base["cashbalance"].as_f64().unwrap_or(0.0),
            buying_power: base["cashbalance"].as_f64().unwrap_or(0.0),
            currency: "USD".into(),
            daily_change_percent: None,
        })
    }

    async fn get_positions(&self, account_id: &str) -> Result<Vec<Position>, BrokerError> {
        let url = format!("{}/portfolio/{account_id}/positions/0", self.base);
        let body: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        Ok(body
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|p| {
                let quantity = p["position"].as_f64()?;
                if quantity == 0.0 {
                    return None;
                }
                let price = p["mktPrice"].as_f64().unwrap_or(0.0);
                Some(Position {
                    symbol: p["contractDesc"]
                        .as_str()
                        .or(p["ticker"].as_str())?
                        .trim()
                        .to_string(),
                    name: p["name"].as_str().map(String::from),
                    quantity,
                    current_price: price,
                    market_value: p["mktValue"].as_f64().unwrap_or(quantity * price),
                    cost_basis: p["avgCost"].as_f64().map(|c| c * quantity),
                    unrealized_pl: p["unrealizedPnl"].as_f64(),
                })
            })
            .collect())
    }

    async fn get_orders(&self, _account_id: &str) -> Result<Vec<BrokerOrder>, BrokerError> {
        // Read-only v1: order history via gateway is not implemented.
        Ok(vec![])
    }

    async fn get_market_clock(&self) -> Result<MarketClock, BrokerError> {
        // The Client Portal API has no simple market-clock endpoint.
        Ok(MarketClock {
            is_open: false,
            next_open: None,
            next_close: None,
        })
    }

    async fn get_quotes(&self, _symbols: &[String]) -> Result<Vec<Quote>, BrokerError> {
        // Market data snapshots need conid resolution + subscription handshakes;
        // not reliable enough to ship. Positions already carry mktPrice.
        Ok(vec![])
    }

    async fn validate_orders(
        &self,
        _account_id: &str,
        _orders: &[ProposedOrder],
    ) -> Result<Vec<ValidatedOrder>, BrokerError> {
        Err(Self::unsupported())
    }

    async fn submit_orders(
        &self,
        _account_id: &str,
        _orders: &[ValidatedOrder],
    ) -> Result<Vec<SubmittedOrder>, BrokerError> {
        Err(Self::unsupported())
    }

    async fn cancel_order(&self, _account_id: &str, _order_id: &str) -> Result<(), BrokerError> {
        Err(Self::unsupported())
    }

    fn supports_trading(&self) -> bool {
        false
    }
}

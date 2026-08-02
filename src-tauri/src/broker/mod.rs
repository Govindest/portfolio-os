pub mod alpaca;
pub mod ibkr;
pub mod mock;
pub mod tradier;

use crate::errors::BrokerError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BrokerKind {
    Mock,
    Alpaca,
    Tradier,
    Ibkr,
    Manual,
}

impl BrokerKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BrokerKind::Mock => "mock",
            BrokerKind::Alpaca => "alpaca",
            BrokerKind::Tradier => "tradier",
            BrokerKind::Ibkr => "ibkr",
            BrokerKind::Manual => "manual",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "mock" => Some(Self::Mock),
            "alpaca" => Some(Self::Alpaca),
            "tradier" => Some(Self::Tradier),
            "ibkr" => Some(Self::Ibkr),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub paper: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerAccount {
    pub id: String,
    pub name: String,
    pub paper: bool,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSummary {
    pub account_id: String,
    pub portfolio_value: f64,
    pub cash: f64,
    pub buying_power: f64,
    pub currency: String,
    pub daily_change_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub name: Option<String>,
    pub quantity: f64,
    pub current_price: f64,
    pub market_value: f64,
    pub cost_basis: Option<f64>,
    pub unrealized_pl: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "buy" => Some(Self::Buy),
            "sell" => Some(Self::Sell),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Accepted,
    PartiallyFilled,
    Filled,
    Rejected,
    Canceled,
}

impl OrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderStatus::Pending => "pending",
            OrderStatus::Accepted => "accepted",
            OrderStatus::PartiallyFilled => "partially_filled",
            OrderStatus::Filled => "filled",
            OrderStatus::Rejected => "rejected",
            OrderStatus::Canceled => "canceled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerOrder {
    pub id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub filled_quantity: f64,
    pub avg_fill_price: Option<f64>,
    pub status: OrderStatus,
    pub submitted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketClock {
    pub is_open: bool,
    pub next_open: Option<DateTime<Utc>>,
    pub next_close: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub price: f64,
    pub as_of: DateTime<Utc>,
    pub stale: bool,
}

/// An order the rebalancing engine proposes, before broker validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedOrder {
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub estimated_value: f64,
    pub current_weight: f64,
    pub target_weight: f64,
    pub drift: f64,
    pub reason: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedOrder {
    pub proposed: ProposedOrder,
    pub valid: bool,
    pub validation_message: Option<String>,
    /// Client-generated idempotency key, stable per rebalance-run + symbol,
    /// so a retried submission cannot create a duplicate order.
    pub client_order_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmittedOrder {
    pub client_order_id: String,
    pub broker_order_id: Option<String>,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub status: OrderStatus,
    pub message: Option<String>,
}

/// Shared abstraction every brokerage integration implements.
/// Broker-specific payloads never leave the adapter.
#[async_trait]
pub trait BrokerAdapter: Send + Sync {
    async fn test_connection(&self) -> Result<ConnectionStatus, BrokerError>;
    async fn get_accounts(&self) -> Result<Vec<BrokerAccount>, BrokerError>;
    async fn get_account_summary(&self, account_id: &str) -> Result<AccountSummary, BrokerError>;
    async fn get_positions(&self, account_id: &str) -> Result<Vec<Position>, BrokerError>;
    async fn get_orders(&self, account_id: &str) -> Result<Vec<BrokerOrder>, BrokerError>;
    async fn get_market_clock(&self) -> Result<MarketClock, BrokerError>;
    async fn get_quotes(&self, symbols: &[String]) -> Result<Vec<Quote>, BrokerError>;
    async fn validate_orders(
        &self,
        account_id: &str,
        orders: &[ProposedOrder],
    ) -> Result<Vec<ValidatedOrder>, BrokerError>;
    async fn submit_orders(
        &self,
        account_id: &str,
        orders: &[ValidatedOrder],
    ) -> Result<Vec<SubmittedOrder>, BrokerError>;
    async fn cancel_order(&self, account_id: &str, order_id: &str) -> Result<(), BrokerError>;
    /// Whether the broker supports fractional share quantities.
    fn supports_fractional(&self) -> bool {
        false
    }
    /// Whether this adapter can place orders at all (IBKR v1 is read-only).
    fn supports_trading(&self) -> bool {
        true
    }
}

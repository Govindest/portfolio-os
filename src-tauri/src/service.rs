//! Service layer: everything the UI can do, independent of Tauri, so the full
//! product flow is testable headlessly. Tauri commands are thin wrappers.

use crate::audit::{self, AuditRefs};
use crate::broker::{
    alpaca::AlpacaBroker, ibkr::IbkrBroker, mock::MockBroker, mock::MOCK_ACCOUNT_ID,
    tradier::TradierBroker, AccountSummary, BrokerAdapter, BrokerKind, ConnectionStatus, OrderSide,
    Position, Quote,
};
use crate::credentials::{mask, BrokerSecrets, CredentialStore};
use crate::db::Db;
use crate::errors::BrokerError;
use crate::rebalance::{plan_rebalance, RebalanceInput, RebalancePlan, SafetySettings};
use crate::strategy::{self, StrategyDoc};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub struct AppService {
    pub db: Db,
    pub creds: Box<dyn CredentialStore>,
    pub data_dir: PathBuf,
}

// ---------- DTOs ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub safety: SafetySettings,
    /// None = manual sync only.
    pub sync_interval_minutes: Option<u32>,
    pub theme: String,
    pub read_only: bool,
    pub emergency_stop: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            safety: SafetySettings::default(),
            sync_interval_minutes: Some(15),
            theme: "system".into(),
            read_only: false,
            emergency_stop: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionDto {
    pub id: String,
    pub broker_kind: String,
    pub label: String,
    pub status: String,
    pub paper: bool,
    pub default_account_id: Option<String>,
    pub masked_credential: Option<String>,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
    pub accounts: Vec<AccountDto>,
    pub supports_trading: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDto {
    pub id: String,
    pub name: String,
    pub paper: bool,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategySummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub kind: String,
    pub current_version: i64,
    pub asset_count: i64,
    pub updated_at: String,
    pub doc: StrategyDoc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceRunDto {
    pub id: String,
    pub connection_id: String,
    pub account_id: String,
    pub strategy_id: Option<String>,
    pub status: String,
    pub plan: RebalancePlan,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmittedOrderDto {
    pub client_order_id: String,
    pub run_id: Option<String>,
    pub broker_order_id: Option<String>,
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
    pub filled_quantity: f64,
    pub avg_fill_price: Option<f64>,
    pub status: String,
    pub message: Option<String>,
    pub submitted_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardDto {
    pub summary: Option<AccountSummary>,
    pub positions: Vec<Position>,
    pub recent_orders: Vec<SubmittedOrderDto>,
    pub drift_percent: Option<f64>,
    pub active_strategy: Option<StrategySummary>,
    pub history: Vec<HistoryPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryPoint {
    pub taken_at: String,
    pub portfolio_value: f64,
}

impl AppService {
    pub fn new(db: Db, creds: Box<dyn CredentialStore>, data_dir: PathBuf) -> Self {
        Self {
            db,
            creds,
            data_dir,
        }
    }

    // ---------- settings & onboarding ----------

    pub fn get_settings(&self) -> Result<AppSettings, BrokerError> {
        let conn = self.db.conn.lock().unwrap();
        let json: Option<String> = conn
            .query_row("SELECT value FROM app_settings WHERE key='app'", [], |r| {
                r.get(0)
            })
            .ok();
        Ok(json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default())
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), BrokerError> {
        let json = serde_json::to_string(settings).unwrap();
        {
            let conn = self.db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO app_settings (key, value) VALUES ('app', ?1)
                 ON CONFLICT(key) DO UPDATE SET value=?1, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",
                [&json],
            )?;
        }
        audit::record(
            &self.db,
            "settings_updated",
            "Settings were updated.",
            AuditRefs::default(),
            None,
        )
    }

    pub fn set_emergency_stop(&self, enabled: bool) -> Result<AppSettings, BrokerError> {
        let mut s = self.get_settings()?;
        s.emergency_stop = enabled;
        self.save_settings(&s)?;
        audit::record(
            &self.db,
            if enabled {
                "emergency_stop_enabled"
            } else {
                "emergency_stop_disabled"
            },
            if enabled {
                "Emergency stop enabled. New order submission is blocked."
            } else {
                "Emergency stop disabled."
            },
            AuditRefs::default(),
            None,
        )?;
        Ok(s)
    }

    pub fn get_onboarding(&self) -> Result<(bool, String), BrokerError> {
        let conn = self.db.conn.lock().unwrap();
        Ok(conn
            .query_row(
                "SELECT completed, step FROM onboarding_state WHERE id=1",
                [],
                |r| Ok((r.get::<_, i64>(0)? != 0, r.get(1)?)),
            )
            .unwrap_or((false, "welcome".into())))
    }

    pub fn set_onboarding(&self, completed: bool, step: &str) -> Result<(), BrokerError> {
        let conn = self.db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO onboarding_state (id, completed, step) VALUES (1, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET completed=?1, step=?2, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",
            rusqlite::params![completed as i64, step],
        )?;
        Ok(())
    }

    // ---------- connections ----------

    fn adapter_for(&self, connection_id: &str) -> Result<Box<dyn BrokerAdapter>, BrokerError> {
        let (kind, gateway_url): (String, Option<String>) = {
            let conn = self.db.conn.lock().unwrap();
            conn.query_row(
                "SELECT broker_kind, gateway_url FROM brokerage_connections WHERE id=?1",
                [connection_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| BrokerError::validation("This brokerage connection no longer exists."))?
        };
        let kind = BrokerKind::parse(&kind)
            .ok_or_else(|| BrokerError::validation("Unknown broker type."))?;
        self.build_adapter(&kind, connection_id, gateway_url)
    }

    fn build_adapter(
        &self,
        kind: &BrokerKind,
        connection_id: &str,
        gateway_url: Option<String>,
    ) -> Result<Box<dyn BrokerAdapter>, BrokerError> {
        match kind {
            BrokerKind::Mock => Ok(Box::new(MockBroker::new(Some(
                self.data_dir.join("mock_broker_state.json"),
            )))),
            BrokerKind::Alpaca => {
                let s = self.require_secrets(connection_id)?;
                Ok(Box::new(AlpacaBroker::new(s.key, s.secret)))
            }
            BrokerKind::Tradier => {
                let s = self.require_secrets(connection_id)?;
                Ok(Box::new(TradierBroker::new(s.key)))
            }
            BrokerKind::Ibkr => Ok(Box::new(IbkrBroker::new(gateway_url))),
            BrokerKind::Manual => Err(BrokerError::validation(
                "Manual portfolios do not connect to a brokerage.",
            )),
        }
    }

    fn require_secrets(&self, connection_id: &str) -> Result<BrokerSecrets, BrokerError> {
        self.creds.get(connection_id)?.ok_or_else(|| {
            BrokerError::credential_store(format!("No stored credentials for {connection_id}"))
        })
    }

    /// Test credentials for a broker without persisting anything.
    pub async fn preview_connection(
        &self,
        kind: &str,
        secrets: &BrokerSecrets,
        gateway_url: Option<String>,
    ) -> Result<ConnectionStatus, BrokerError> {
        let kind = BrokerKind::parse(kind)
            .ok_or_else(|| BrokerError::validation("Unknown broker type."))?;
        let adapter: Box<dyn BrokerAdapter> = match kind {
            BrokerKind::Mock => Box::new(MockBroker::new(None)),
            BrokerKind::Alpaca => Box::new(AlpacaBroker::new(
                secrets.key.clone(),
                secrets.secret.clone(),
            )),
            BrokerKind::Tradier => Box::new(TradierBroker::new(secrets.key.clone())),
            BrokerKind::Ibkr => Box::new(IbkrBroker::new(gateway_url)),
            BrokerKind::Manual => {
                return Ok(ConnectionStatus {
                    connected: true,
                    paper: true,
                    message: "Manual portfolios are tracked locally.".into(),
                })
            }
        };
        adapter.test_connection().await
    }

    pub async fn create_connection(
        &self,
        kind: &str,
        label: &str,
        secrets: BrokerSecrets,
        gateway_url: Option<String>,
    ) -> Result<ConnectionDto, BrokerError> {
        let broker_kind = BrokerKind::parse(kind)
            .ok_or_else(|| BrokerError::validation("Unknown broker type."))?;
        let id = Uuid::new_v4().to_string();
        let masked = if secrets.key.is_empty() {
            None
        } else {
            Some(mask(&secrets.key))
        };

        // Manual portfolios skip adapters entirely.
        if broker_kind == BrokerKind::Manual {
            let conn = self.db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO brokerage_connections (id, broker_kind, label, paper, default_account_id) VALUES (?1,'manual',?2,1,'manual')",
                rusqlite::params![id, label],
            )?;
            conn.execute(
                "INSERT INTO brokerage_accounts (id, connection_id, name, paper) VALUES ('manual', ?1, ?2, 1)",
                rusqlite::params![id, label],
            )?;
            drop(conn);
            audit::record(
                &self.db,
                "connection_created",
                &format!("Manual portfolio \"{label}\" was created."),
                AuditRefs {
                    connection_id: Some(id.clone()),
                    ..Default::default()
                },
                None,
            )?;
            return self.get_connection(&id);
        }

        if !secrets.key.is_empty() || !secrets.secret.is_empty() {
            self.creds.save(&id, &secrets)?;
        }
        let adapter = self.build_adapter(&broker_kind, &id, gateway_url.clone())?;
        let status = match adapter.test_connection().await {
            Ok(s) => s,
            Err(e) => {
                // Never keep credentials for a failed connection.
                let _ = self.creds.delete(&id);
                return Err(e);
            }
        };
        let accounts = adapter.get_accounts().await?;
        let default_account = accounts.first().map(|a| a.id.clone());
        {
            let mut conn = self.db.conn.lock().unwrap();
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO brokerage_connections (id, broker_kind, label, paper, default_account_id, gateway_url, masked_credential)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![id, broker_kind.as_str(), label, status.paper as i64, default_account, gateway_url, masked],
            )?;
            for a in &accounts {
                tx.execute(
                    "INSERT INTO brokerage_accounts (id, connection_id, name, paper, currency) VALUES (?1,?2,?3,?4,?5)",
                    rusqlite::params![a.id, id, a.name, a.paper as i64, a.currency],
                )?;
            }
            tx.commit()?;
        }
        audit::record(
            &self.db,
            "connection_created",
            &format!("Connected {label}."),
            AuditRefs {
                connection_id: Some(id.clone()),
                ..Default::default()
            },
            None,
        )?;
        if broker_kind == BrokerKind::Mock {
            self.seed_demo_history(&id)?;
        }
        // Initial sync so the dashboard is instantly populated.
        self.sync_connection(&id, "initial").await.ok();
        self.get_connection(&id)
    }

    /// Deterministic 30-day synthetic value history for the demo account.
    fn seed_demo_history(&self, connection_id: &str) -> Result<(), BrokerError> {
        let conn = self.db.conn.lock().unwrap();
        let base = 43_000.0_f64;
        for day in 0..30 {
            // Smooth deterministic wave, no randomness: tests stay stable.
            let t = day as f64;
            let value = base * (1.0 + 0.002 * t + 0.008 * (t * 0.9).sin());
            let taken = (Utc::now() - chrono::Duration::days(30 - day)).to_rfc3339();
            conn.execute(
                "INSERT INTO account_snapshots (connection_id, account_id, portfolio_value, cash, buying_power, taken_at)
                 VALUES (?1, ?2, ?3, 12450.0, 12450.0, ?4)",
                rusqlite::params![connection_id, MOCK_ACCOUNT_ID, value, taken],
            )?;
        }
        Ok(())
    }

    pub fn get_connection(&self, id: &str) -> Result<ConnectionDto, BrokerError> {
        self.list_connections()?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| BrokerError::validation("Connection not found."))
    }

    pub fn list_connections(&self) -> Result<Vec<ConnectionDto>, BrokerError> {
        let conn = self.db.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, broker_kind, label, status, paper, default_account_id, masked_credential, last_sync_at, last_error, created_at
             FROM brokerage_connections ORDER BY created_at",
        )?;
        let mut out: Vec<ConnectionDto> = stmt
            .query_map([], |r| {
                let kind: String = r.get(1)?;
                Ok(ConnectionDto {
                    id: r.get(0)?,
                    supports_trading: kind != "ibkr" && kind != "manual",
                    broker_kind: kind,
                    label: r.get(2)?,
                    status: r.get(3)?,
                    paper: r.get::<_, i64>(4)? != 0,
                    default_account_id: r.get(5)?,
                    masked_credential: r.get(6)?,
                    last_sync_at: r.get(7)?,
                    last_error: r.get(8)?,
                    created_at: r.get(9)?,
                    accounts: vec![],
                })
            })?
            .filter_map(Result::ok)
            .collect();
        let mut acct_stmt = conn.prepare(
            "SELECT id, name, paper, currency FROM brokerage_accounts WHERE connection_id=?1",
        )?;
        for c in out.iter_mut() {
            c.accounts = acct_stmt
                .query_map([&c.id], |r| {
                    Ok(AccountDto {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        paper: r.get::<_, i64>(2)? != 0,
                        currency: r.get(3)?,
                    })
                })?
                .filter_map(Result::ok)
                .collect();
        }
        Ok(out)
    }

    pub async fn test_connection(&self, id: &str) -> Result<ConnectionStatus, BrokerError> {
        let kind: String = {
            let conn = self.db.conn.lock().unwrap();
            conn.query_row(
                "SELECT broker_kind FROM brokerage_connections WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(|_| BrokerError::validation("Connection not found."))?
        };
        if kind == "manual" {
            return Ok(ConnectionStatus {
                connected: true,
                paper: true,
                message: "Manual portfolio is stored locally.".into(),
            });
        }
        self.adapter_for(id)?.test_connection().await
    }

    pub fn disconnect(&self, id: &str) -> Result<(), BrokerError> {
        // Remove credentials from the OS store first — that is the whole point.
        self.creds.delete(id)?;
        let label: String = {
            let conn = self.db.conn.lock().unwrap();
            let label = conn
                .query_row(
                    "SELECT label FROM brokerage_connections WHERE id=?1",
                    [id],
                    |r| r.get(0),
                )
                .unwrap_or_else(|_| "connection".into());
            conn.execute("DELETE FROM brokerage_connections WHERE id=?1", [id])?;
            label
        };
        audit::record(
            &self.db,
            "connection_removed",
            &format!("Disconnected {label} and removed stored credentials."),
            AuditRefs::default(),
            None,
        )
    }

    pub fn set_default_account(
        &self,
        connection_id: &str,
        account_id: &str,
    ) -> Result<(), BrokerError> {
        let conn = self.db.conn.lock().unwrap();
        conn.execute(
            "UPDATE brokerage_connections SET default_account_id=?2 WHERE id=?1",
            rusqlite::params![connection_id, account_id],
        )?;
        Ok(())
    }

    // ---------- sync ----------

    pub async fn sync_connection(&self, id: &str, trigger: &str) -> Result<(), BrokerError> {
        let started = Utc::now().to_rfc3339();
        let result = self.sync_inner(id).await;
        let (success, detail) = match &result {
            Ok(()) => (1, None),
            Err(e) => (0, Some(e.user_message())),
        };
        {
            let conn = self.db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO synchronization_runs (connection_id, trigger, success, detail, started_at) VALUES (?1,?2,?3,?4,?5)",
                rusqlite::params![id, trigger, success, detail, started],
            )?;
            if success == 1 {
                conn.execute(
                    "UPDATE brokerage_connections SET last_sync_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'), last_error=NULL, status='connected' WHERE id=?1",
                    [id],
                )?;
            } else {
                conn.execute(
                    "UPDATE brokerage_connections SET last_error=?2, status='error' WHERE id=?1",
                    rusqlite::params![id, detail],
                )?;
            }
        }
        result
    }

    async fn sync_inner(&self, id: &str) -> Result<(), BrokerError> {
        let kind: String = {
            let conn = self.db.conn.lock().unwrap();
            conn.query_row(
                "SELECT broker_kind FROM brokerage_connections WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(|_| BrokerError::validation("Connection not found."))?
        };
        if kind == "manual" {
            return Ok(()); // nothing remote to sync
        }
        let adapter = self.adapter_for(id)?;
        let accounts = {
            let conn = self.db.conn.lock().unwrap();
            let mut stmt =
                conn.prepare("SELECT id FROM brokerage_accounts WHERE connection_id=?1")?;
            let ids: Vec<String> = stmt
                .query_map([id], |r| r.get(0))?
                .filter_map(Result::ok)
                .collect();
            ids
        };
        for account_id in accounts {
            let summary = adapter.get_account_summary(&account_id).await?;
            let positions = adapter.get_positions(&account_id).await?;
            let orders = adapter.get_orders(&account_id).await.unwrap_or_default();
            let mut conn = self.db.conn.lock().unwrap();
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO account_snapshots (connection_id, account_id, portfolio_value, cash, buying_power, daily_change_percent)
                 VALUES (?1,?2,?3,?4,?5,?6)",
                rusqlite::params![id, account_id, summary.portfolio_value, summary.cash, summary.buying_power, summary.daily_change_percent],
            )?;
            tx.execute(
                "DELETE FROM positions WHERE connection_id=?1 AND account_id=?2",
                rusqlite::params![id, account_id],
            )?;
            for p in &positions {
                tx.execute(
                    "INSERT INTO positions (connection_id, account_id, symbol, name, quantity, current_price, market_value, cost_basis, unrealized_pl)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                    rusqlite::params![id, account_id, p.symbol, p.name, p.quantity, p.current_price, p.market_value, p.cost_basis, p.unrealized_pl],
                )?;
                tx.execute(
                    "INSERT INTO position_snapshots (connection_id, account_id, symbol, quantity, market_value) VALUES (?1,?2,?3,?4,?5)",
                    rusqlite::params![id, account_id, p.symbol, p.quantity, p.market_value],
                )?;
            }
            // Reconcile fills for orders we submitted.
            for o in &orders {
                let changed = tx.execute(
                    "UPDATE submitted_orders
                     SET status=?2, filled_quantity=?3, avg_fill_price=?4, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')
                     WHERE broker_order_id=?1 AND status<>?2",
                    rusqlite::params![o.id, o.status.as_str(), o.filled_quantity, o.avg_fill_price],
                )?;
                if changed > 0 {
                    let client_id: Option<String> = tx
                        .query_row(
                            "SELECT client_order_id FROM submitted_orders WHERE broker_order_id=?1",
                            [&o.id],
                            |r| r.get(0),
                        )
                        .ok();
                    if let Some(cid) = client_id {
                        tx.execute(
                            "INSERT INTO order_events (client_order_id, status, detail) VALUES (?1,?2,?3)",
                            rusqlite::params![
                                cid,
                                o.status.as_str(),
                                format!("Filled {} of {} at {}", o.filled_quantity, o.quantity, o.avg_fill_price.map(|p| format!("${p:.2}")).unwrap_or_else(|| "—".into()))
                            ],
                        )?;
                    }
                }
            }
            tx.commit()?;
        }
        audit::record(
            &self.db,
            "sync_completed",
            "Account data was synchronized.",
            AuditRefs {
                connection_id: Some(id.to_string()),
                ..Default::default()
            },
            None,
        )?;
        Ok(())
    }

    // ---------- portfolio / dashboard ----------

    pub fn stored_positions(
        &self,
        connection_id: &str,
        account_id: &str,
    ) -> Result<Vec<Position>, BrokerError> {
        let conn = self.db.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT symbol, name, quantity, current_price, market_value, cost_basis, unrealized_pl
             FROM positions WHERE connection_id=?1 AND account_id=?2 ORDER BY market_value DESC",
        )?;
        let rows: Vec<_> = stmt
            .query_map(rusqlite::params![connection_id, account_id], |r| {
                Ok(Position {
                    symbol: r.get(0)?,
                    name: r.get(1)?,
                    quantity: r.get(2)?,
                    current_price: r.get(3)?,
                    market_value: r.get(4)?,
                    cost_basis: r.get(5)?,
                    unrealized_pl: r.get(6)?,
                })
            })?
            .filter_map(Result::ok)
            .collect();
        drop(stmt);
        Ok(rows)
    }

    fn latest_snapshot(&self, connection_id: &str, account_id: &str) -> Option<AccountSummary> {
        let conn = self.db.conn.lock().unwrap();
        conn.query_row(
            "SELECT portfolio_value, cash, buying_power, daily_change_percent FROM account_snapshots
             WHERE connection_id=?1 AND account_id=?2 ORDER BY id DESC LIMIT 1",
            rusqlite::params![connection_id, account_id],
            |r| {
                Ok(AccountSummary {
                    account_id: account_id.into(),
                    portfolio_value: r.get(0)?,
                    cash: r.get(1)?,
                    buying_power: r.get(2)?,
                    currency: "USD".into(),
                    daily_change_percent: r.get(3)?,
                })
            },
        )
        .ok()
    }

    pub fn get_dashboard(
        &self,
        connection_id: &str,
        account_id: &str,
    ) -> Result<DashboardDto, BrokerError> {
        let summary = self.latest_snapshot(connection_id, account_id);
        let positions = self.stored_positions(connection_id, account_id)?;
        let recent_orders = self.list_submitted_orders(Some(connection_id), 10)?;
        let active_strategy = self.list_strategies()?.into_iter().next();
        let drift_percent = active_strategy.as_ref().and_then(|s| {
            let total: f64 = positions.iter().map(|p| p.market_value).sum();
            let cash = summary.as_ref().map(|x| x.cash).unwrap_or(0.0);
            let pv = total + cash;
            if pv <= 0.0 {
                return None;
            }
            let mut drift = 0.0;
            for a in &s.doc.assets {
                let held = positions
                    .iter()
                    .find(|p| p.symbol == a.symbol)
                    .map(|p| p.market_value)
                    .unwrap_or(0.0);
                drift += (held / pv - a.target_weight).abs();
            }
            for p in &positions {
                if !s.doc.assets.iter().any(|a| a.symbol == p.symbol) {
                    drift += p.market_value / pv;
                }
            }
            Some(drift / 2.0 * 100.0)
        });
        let history = {
            let conn = self.db.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT taken_at, portfolio_value FROM account_snapshots
                 WHERE connection_id=?1 AND account_id=?2 ORDER BY taken_at ASC",
            )?;
            let rows: Vec<_> = stmt
                .query_map(rusqlite::params![connection_id, account_id], |r| {
                    Ok(HistoryPoint {
                        taken_at: r.get(0)?,
                        portfolio_value: r.get(1)?,
                    })
                })?
                .filter_map(Result::ok)
                .collect();
            drop(stmt);
            rows
        };
        Ok(DashboardDto {
            summary,
            positions,
            recent_orders,
            drift_percent,
            active_strategy,
            history,
        })
    }

    // ---------- manual portfolio ----------

    pub fn upsert_manual_position(
        &self,
        connection_id: &str,
        symbol: &str,
        quantity: f64,
        price: f64,
        cost_basis: Option<f64>,
    ) -> Result<(), BrokerError> {
        let symbol = symbol.trim().to_uppercase();
        if symbol.is_empty() || quantity < 0.0 || price < 0.0 {
            return Err(BrokerError::validation(
                "Enter a symbol, a non-negative quantity, and a non-negative price.",
            ));
        }
        let conn = self.db.conn.lock().unwrap();
        if quantity == 0.0 {
            conn.execute(
                "DELETE FROM positions WHERE connection_id=?1 AND account_id='manual' AND symbol=?2",
                rusqlite::params![connection_id, symbol],
            )?;
        } else {
            conn.execute(
                "INSERT INTO positions (connection_id, account_id, symbol, quantity, current_price, market_value, cost_basis)
                 VALUES (?1, 'manual', ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(connection_id, account_id, symbol)
                 DO UPDATE SET quantity=?3, current_price=?4, market_value=?5, cost_basis=?6, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",
                rusqlite::params![connection_id, symbol, quantity, price, quantity * price, cost_basis],
            )?;
        }
        drop(conn);
        self.refresh_manual_snapshot(connection_id)
    }

    pub fn set_manual_cash(&self, connection_id: &str, cash: f64) -> Result<(), BrokerError> {
        if cash < 0.0 {
            return Err(BrokerError::validation("Cash cannot be negative."));
        }
        {
            let conn = self.db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value=?2",
                rusqlite::params![format!("manual_cash:{connection_id}"), cash.to_string()],
            )?;
        }
        self.refresh_manual_snapshot(connection_id)
    }

    fn manual_cash(&self, connection_id: &str) -> f64 {
        let conn = self.db.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM app_settings WHERE key=?1",
            [format!("manual_cash:{connection_id}")],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0)
    }

    fn refresh_manual_snapshot(&self, connection_id: &str) -> Result<(), BrokerError> {
        let cash = self.manual_cash(connection_id);
        let positions = self.stored_positions(connection_id, "manual")?;
        let value: f64 = positions.iter().map(|p| p.market_value).sum::<f64>() + cash;
        let conn = self.db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO account_snapshots (connection_id, account_id, portfolio_value, cash, buying_power)
             VALUES (?1, 'manual', ?2, ?3, ?3)",
            rusqlite::params![connection_id, value, cash],
        )?;
        Ok(())
    }

    /// CSV import: symbol,quantity,price[,cost_basis]
    pub fn import_positions_csv(
        &self,
        connection_id: &str,
        content: &str,
    ) -> Result<u32, BrokerError> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(content.as_bytes());
        let mut count = 0;
        for rec in reader.records() {
            let rec = rec
                .map_err(|e| BrokerError::validation(&format!("CSV row could not be read: {e}")))?;
            let symbol = rec.get(0).unwrap_or("").trim();
            if symbol.is_empty() {
                continue;
            }
            let quantity: f64 =
                rec.get(1).unwrap_or("").trim().parse().map_err(|_| {
                    BrokerError::validation(&format!("Invalid quantity for {symbol}."))
                })?;
            let price: f64 =
                rec.get(2).unwrap_or("").trim().parse().map_err(|_| {
                    BrokerError::validation(&format!("Invalid price for {symbol}."))
                })?;
            let cost: Option<f64> = rec.get(3).and_then(|c| c.trim().parse().ok());
            self.upsert_manual_position(connection_id, symbol, quantity, price, cost)?;
            count += 1;
        }
        audit::record(
            &self.db,
            "positions_imported",
            &format!("Imported {count} positions from CSV."),
            AuditRefs {
                connection_id: Some(connection_id.into()),
                ..Default::default()
            },
            None,
        )?;
        Ok(count)
    }

    // ---------- strategies ----------

    pub fn save_strategy(
        &self,
        kind: &str,
        doc: &StrategyDoc,
        existing_id: Option<String>,
    ) -> Result<StrategySummary, BrokerError> {
        let mut doc = doc.clone();
        strategy::validate(&mut doc)?;
        let yaml = strategy::to_yaml(&doc)?;
        let id = existing_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        {
            let mut conn = self.db.conn.lock().unwrap();
            let tx = conn.transaction()?;
            let version: i64 = if existing_id.is_some() {
                let v: i64 = tx
                    .query_row(
                        "SELECT current_version FROM strategies WHERE id=?1",
                        [&id],
                        |r| r.get(0),
                    )
                    .map_err(|_| BrokerError::validation("Strategy not found."))?;
                v + 1
            } else {
                1
            };
            tx.execute(
                "INSERT INTO strategies (id, name, description, kind, current_version)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET name=?2, description=?3, current_version=?5, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",
                rusqlite::params![id, doc.name, doc.description, kind, version],
            )?;
            tx.execute(
                "INSERT INTO strategy_versions (strategy_id, version, yaml) VALUES (?1, ?2, ?3)",
                rusqlite::params![id, version, yaml],
            )?;
            tx.execute("DELETE FROM strategy_assets WHERE strategy_id=?1", [&id])?;
            for a in &doc.assets {
                tx.execute(
                    "INSERT INTO strategy_assets (strategy_id, symbol, target_weight) VALUES (?1,?2,?3)",
                    rusqlite::params![id, a.symbol, a.target_weight],
                )?;
            }
            tx.commit()?;
        }
        audit::record(
            &self.db,
            if existing_id.is_some() {
                "strategy_updated"
            } else {
                "strategy_created"
            },
            &format!(
                "Strategy \"{}\" was {}.",
                doc.name,
                if existing_id.is_some() {
                    "updated"
                } else {
                    "created"
                }
            ),
            AuditRefs {
                strategy_id: Some(id.clone()),
                ..Default::default()
            },
            None,
        )?;
        self.get_strategy(&id)
    }

    pub fn get_strategy(&self, id: &str) -> Result<StrategySummary, BrokerError> {
        self.list_strategies()?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| BrokerError::validation("Strategy not found."))
    }

    pub fn list_strategies(&self) -> Result<Vec<StrategySummary>, BrokerError> {
        let conn = self.db.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT s.id, s.name, s.description, s.kind, s.current_version, s.updated_at,
                    (SELECT yaml FROM strategy_versions v WHERE v.strategy_id = s.id AND v.version = s.current_version)
             FROM strategies s WHERE s.archived = 0 ORDER BY s.updated_at DESC",
        )?;
        let rows: Vec<_> = stmt
            .query_map([], |r| {
                let yaml: String = r.get(6)?;
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, String>(5)?,
                    yaml,
                ))
            })?
            .filter_map(Result::ok)
            .filter_map(|(id, name, description, kind, version, updated_at, yaml)| {
                let doc = strategy::parse_yaml(&yaml).ok()?;
                Some(StrategySummary {
                    id,
                    name,
                    description,
                    kind,
                    current_version: version,
                    asset_count: doc.assets.len() as i64,
                    updated_at,
                    doc,
                })
            })
            .collect();
        drop(stmt);
        Ok(rows)
    }

    pub fn delete_strategy(&self, id: &str) -> Result<(), BrokerError> {
        let conn = self.db.conn.lock().unwrap();
        conn.execute("UPDATE strategies SET archived=1 WHERE id=?1", [id])?;
        drop(conn);
        audit::record(
            &self.db,
            "strategy_archived",
            "A strategy was archived.",
            AuditRefs {
                strategy_id: Some(id.into()),
                ..Default::default()
            },
            None,
        )
    }

    pub fn import_strategy_yaml(&self, yaml: &str) -> Result<StrategySummary, BrokerError> {
        let doc = strategy::parse_yaml(yaml)?;
        self.save_strategy("imported", &doc, None)
    }

    pub fn export_strategy_yaml(&self, id: &str) -> Result<String, BrokerError> {
        let s = self.get_strategy(id)?;
        strategy::to_yaml(&s.doc)
    }

    // ---------- rebalancing ----------

    pub async fn run_rebalance(
        &self,
        connection_id: &str,
        account_id: &str,
        strategy_id: &str,
    ) -> Result<RebalanceRunDto, BrokerError> {
        let strategy = self.get_strategy(strategy_id)?;
        let settings = self.get_settings()?;
        let kind: String = {
            let conn = self.db.conn.lock().unwrap();
            conn.query_row(
                "SELECT broker_kind FROM brokerage_connections WHERE id=?1",
                [connection_id],
                |r| r.get(0),
            )
            .map_err(|_| BrokerError::validation("Connection not found."))?
        };

        let positions = self.stored_positions(connection_id, account_id)?;
        let symbols: Vec<String> = strategy
            .doc
            .assets
            .iter()
            .map(|a| a.symbol.clone())
            .chain(positions.iter().map(|p| p.symbol.clone()))
            .collect();

        let (quotes, market_open, fractional, cash, buying_power, open_symbols) = if kind
            == "manual"
        {
            let cash = self.manual_cash(connection_id);
            let quotes: Vec<Quote> = positions
                .iter()
                .map(|p| Quote {
                    symbol: p.symbol.clone(),
                    price: p.current_price,
                    as_of: Utc::now(),
                    stale: true,
                })
                .collect();
            (quotes, true, true, cash, cash, vec![])
        } else {
            let adapter = self.adapter_for(connection_id)?;
            let quotes = adapter.get_quotes(&symbols).await.unwrap_or_default();
            let clock = adapter
                .get_market_clock()
                .await
                .unwrap_or(crate::broker::MarketClock {
                    is_open: false,
                    next_open: None,
                    next_close: None,
                });
            let summary = self
                .latest_snapshot(connection_id, account_id)
                .ok_or_else(|| BrokerError::validation("Sync the account before rebalancing."))?;
            let open_symbols: Vec<String> = {
                let conn = self.db.conn.lock().unwrap();
                let mut stmt = conn.prepare(
                    "SELECT DISTINCT symbol FROM submitted_orders WHERE connection_id=?1 AND account_id=?2 AND status IN ('pending','accepted','partially_filled')",
                )?;
                let rows: Vec<_> = stmt
                    .query_map(rusqlite::params![connection_id, account_id], |r| r.get(0))?
                    .filter_map(Result::ok)
                    .collect();
                drop(stmt);
                rows
            };
            (
                quotes,
                clock.is_open,
                adapter.supports_fractional(),
                summary.cash,
                summary.buying_power,
                open_symbols,
            )
        };

        let input = RebalanceInput {
            positions,
            quotes,
            cash,
            buying_power,
            open_order_symbols: open_symbols,
            market_open,
            broker_supports_fractional: fractional,
        };
        let plan = plan_rebalance(&strategy.doc, &input, &settings.safety);
        let run_id = Uuid::new_v4().to_string();
        {
            let mut conn = self.db.conn.lock().unwrap();
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO rebalance_runs (id, connection_id, account_id, strategy_id, status, summary_json)
                 VALUES (?1,?2,?3,?4,'proposed',?5)",
                rusqlite::params![run_id, connection_id, account_id, strategy_id, serde_json::to_string(&plan).unwrap()],
            )?;
            for o in &plan.orders {
                tx.execute(
                    "INSERT INTO proposed_orders (id, run_id, symbol, side, quantity, estimated_value, current_weight, target_weight, drift, reason, warnings_json)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                    rusqlite::params![
                        Uuid::new_v4().to_string(),
                        run_id,
                        o.symbol,
                        o.side.as_str(),
                        o.quantity,
                        o.estimated_value,
                        o.current_weight,
                        o.target_weight,
                        o.drift,
                        o.reason,
                        serde_json::to_string(&o.warnings).unwrap()
                    ],
                )?;
            }
            tx.commit()?;
        }
        audit::record(
            &self.db,
            "rebalance_calculated",
            &format!(
                "Rebalance for \"{}\" proposed {} orders (buys ${:.2}, sells ${:.2}).",
                strategy.name,
                plan.orders.len(),
                plan.total_buy_value,
                plan.total_sell_value
            ),
            AuditRefs {
                strategy_id: Some(strategy_id.into()),
                connection_id: Some(connection_id.into()),
                account_id: Some(account_id.into()),
                ..Default::default()
            },
            None,
        )?;
        self.get_rebalance_run(&run_id)
    }

    pub fn get_rebalance_run(&self, run_id: &str) -> Result<RebalanceRunDto, BrokerError> {
        let conn = self.db.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, connection_id, account_id, strategy_id, status, summary_json, created_at FROM rebalance_runs WHERE id=?1",
            [run_id],
            |r| {
                let json: String = r.get(5)?;
                Ok(RebalanceRunDto {
                    id: r.get(0)?,
                    connection_id: r.get(1)?,
                    account_id: r.get(2)?,
                    strategy_id: r.get(3)?,
                    status: r.get(4)?,
                    plan: serde_json::from_str(&json).unwrap(),
                    created_at: r.get(6)?,
                })
            },
        )
        .map_err(|_| BrokerError::validation("Rebalance run not found."))
    }

    /// Approve and submit a rebalance run. `acknowledged` must be true —
    /// explicit batch approval is always required in version 1.
    pub async fn approve_and_submit(
        &self,
        run_id: &str,
        acknowledged: bool,
    ) -> Result<Vec<SubmittedOrderDto>, BrokerError> {
        if !acknowledged {
            return Err(BrokerError::validation(
                "Review the orders and confirm before submitting.",
            ));
        }
        let settings = self.get_settings()?;
        if settings.emergency_stop {
            return Err(BrokerError::validation(
                "Emergency stop is active. Disable it in Settings before submitting orders.",
            ));
        }
        if settings.read_only {
            return Err(BrokerError::validation("PortfolioOS is in read-only mode."));
        }
        let run = self.get_rebalance_run(run_id)?;
        if run.status == "submitted" {
            // Duplicate-submission guard: return what was already submitted.
            return self.orders_for_run(run_id);
        }
        if run.status != "proposed" {
            return Err(BrokerError::validation(
                "This rebalance can no longer be submitted.",
            ));
        }
        let kind: String = {
            let conn = self.db.conn.lock().unwrap();
            conn.query_row(
                "SELECT broker_kind FROM brokerage_connections WHERE id=?1",
                [&run.connection_id],
                |r| r.get(0),
            )
            .map_err(|_| BrokerError::validation("Connection not found."))?
        };
        if kind == "manual" {
            return Err(BrokerError::validation(
                "Manual portfolios cannot submit orders. Export the trade list as CSV and place the trades with your broker.",
            ));
        }
        let adapter = self.adapter_for(&run.connection_id)?;
        if !adapter.supports_trading() {
            return Err(BrokerError::validation(
                "This brokerage connection is read-only in this version of PortfolioOS.",
            ));
        }
        let mut validated = adapter
            .validate_orders(&run.account_id, &run.plan.orders)
            .await?;
        // Deterministic client order id per run+symbol: retries cannot duplicate.
        for v in validated.iter_mut() {
            let short: String = run_id.chars().take(8).collect();
            v.client_order_id = format!(
                "pos-{short}-{}-{}",
                v.proposed.side.as_str(),
                v.proposed.symbol
            );
        }
        // Persist intent BEFORE talking to the broker so an app crash between
        // submit and response cannot lose track of in-flight orders.
        {
            let mut conn = self.db.conn.lock().unwrap();
            let tx = conn.transaction()?;
            for v in &validated {
                tx.execute(
                    "INSERT OR IGNORE INTO submitted_orders (client_order_id, run_id, connection_id, account_id, symbol, side, quantity, status)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,'pending')",
                    rusqlite::params![
                        v.client_order_id,
                        run_id,
                        run.connection_id,
                        run.account_id,
                        v.proposed.symbol,
                        v.proposed.side.as_str(),
                        v.proposed.quantity
                    ],
                )?;
            }
            tx.execute(
                "UPDATE rebalance_runs SET status='submitted' WHERE id=?1",
                [run_id],
            )?;
            tx.commit()?;
        }
        audit::record(
            &self.db,
            "orders_approved",
            &format!("User approved a batch of {} orders.", validated.len()),
            AuditRefs {
                connection_id: Some(run.connection_id.clone()),
                account_id: Some(run.account_id.clone()),
                ..Default::default()
            },
            None,
        )?;
        let submitted = adapter.submit_orders(&run.account_id, &validated).await?;
        {
            let conn = self.db.conn.lock().unwrap();
            for s in &submitted {
                conn.execute(
                    "UPDATE submitted_orders SET broker_order_id=?2, status=?3, message=?4, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE client_order_id=?1",
                    rusqlite::params![s.client_order_id, s.broker_order_id, s.status.as_str(), s.message],
                )?;
                conn.execute(
                    "INSERT INTO order_events (client_order_id, status, detail) VALUES (?1,?2,?3)",
                    rusqlite::params![s.client_order_id, s.status.as_str(), s.message],
                )?;
            }
        }
        for s in &submitted {
            audit::record(
                &self.db,
                "order_submitted",
                &format!(
                    "{} {} {} — {}",
                    match s.side {
                        OrderSide::Buy => "Buy",
                        OrderSide::Sell => "Sell",
                    },
                    s.quantity,
                    s.symbol,
                    s.status.as_str()
                ),
                AuditRefs {
                    connection_id: Some(run.connection_id.clone()),
                    account_id: Some(run.account_id.clone()),
                    order_id: s.broker_order_id.clone(),
                    ..Default::default()
                },
                None,
            )?;
        }
        self.orders_for_run(run_id)
    }

    pub fn orders_for_run(&self, run_id: &str) -> Result<Vec<SubmittedOrderDto>, BrokerError> {
        let conn = self.db.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT client_order_id, run_id, broker_order_id, symbol, side, quantity, filled_quantity, avg_fill_price, status, message, submitted_at, updated_at
             FROM submitted_orders WHERE run_id=?1 ORDER BY submitted_at",
        )?;
        let rows: Vec<_> = stmt
            .query_map([run_id], row_to_submitted)?
            .filter_map(Result::ok)
            .collect();
        drop(stmt);
        Ok(rows)
    }

    pub fn list_submitted_orders(
        &self,
        connection_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<SubmittedOrderDto>, BrokerError> {
        let conn = self.db.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT client_order_id, run_id, broker_order_id, symbol, side, quantity, filled_quantity, avg_fill_price, status, message, submitted_at, updated_at
             FROM submitted_orders WHERE (?1 IS NULL OR connection_id=?1) ORDER BY submitted_at DESC LIMIT ?2",
        )?;
        let rows: Vec<_> = stmt
            .query_map(rusqlite::params![connection_id, limit], row_to_submitted)?
            .filter_map(Result::ok)
            .collect();
        drop(stmt);
        Ok(rows)
    }

    pub async fn cancel_order(
        &self,
        connection_id: &str,
        broker_order_id: &str,
    ) -> Result<(), BrokerError> {
        let account_id: String = {
            let conn = self.db.conn.lock().unwrap();
            conn.query_row(
                "SELECT account_id FROM submitted_orders WHERE broker_order_id=?1",
                [broker_order_id],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| "".into())
        };
        let adapter = self.adapter_for(connection_id)?;
        adapter.cancel_order(&account_id, broker_order_id).await?;
        {
            let conn = self.db.conn.lock().unwrap();
            conn.execute(
                "UPDATE submitted_orders SET status='canceled', updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE broker_order_id=?1",
                [broker_order_id],
            )?;
        }
        audit::record(
            &self.db,
            "order_canceled",
            "An open order was canceled.",
            AuditRefs {
                connection_id: Some(connection_id.into()),
                order_id: Some(broker_order_id.into()),
                ..Default::default()
            },
            None,
        )
    }

    /// Cancel every eligible open order on a connection (emergency-stop helper).
    pub async fn cancel_all_open(&self, connection_id: &str) -> Result<u32, BrokerError> {
        let ids: Vec<String> = {
            let conn = self.db.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT broker_order_id FROM submitted_orders
                 WHERE connection_id=?1 AND broker_order_id IS NOT NULL AND status IN ('pending','accepted','partially_filled')",
            )?;
            let rows: Vec<_> = stmt
                .query_map([connection_id], |r| r.get(0))?
                .filter_map(Result::ok)
                .collect();
            drop(stmt);
            rows
        };
        let mut canceled = 0;
        for id in ids {
            if self.cancel_order(connection_id, &id).await.is_ok() {
                canceled += 1;
            }
        }
        Ok(canceled)
    }

    // ---------- demo & exports ----------

    pub fn reset_demo(&self) -> Result<(), BrokerError> {
        MockBroker::new(Some(self.data_dir.join("mock_broker_state.json"))).reset();
        audit::record(
            &self.db,
            "demo_reset",
            "Demo data was reset to its initial state.",
            AuditRefs::default(),
            None,
        )
    }

    pub fn positions_csv(
        &self,
        connection_id: &str,
        account_id: &str,
    ) -> Result<String, BrokerError> {
        let positions = self.stored_positions(connection_id, account_id)?;
        let mut out =
            String::from("symbol,quantity,current_price,market_value,cost_basis,unrealized_pl\n");
        for p in positions {
            out.push_str(&format!(
                "{},{},{},{},{},{}\n",
                p.symbol,
                p.quantity,
                p.current_price,
                p.market_value,
                p.cost_basis.map(|v| v.to_string()).unwrap_or_default(),
                p.unrealized_pl.map(|v| v.to_string()).unwrap_or_default()
            ));
        }
        Ok(out)
    }

    pub fn proposed_orders_csv(&self, run_id: &str) -> Result<String, BrokerError> {
        let run = self.get_rebalance_run(run_id)?;
        let mut out = String::from(
            "symbol,side,quantity,estimated_value,current_weight,target_weight,reason\n",
        );
        for o in run.plan.orders {
            out.push_str(&format!(
                "{},{},{},{:.2},{:.4},{:.4},\"{}\"\n",
                o.symbol,
                o.side.as_str(),
                o.quantity,
                o.estimated_value,
                o.current_weight,
                o.target_weight,
                o.reason.replace('"', "'")
            ));
        }
        Ok(out)
    }

    /// Write export content to ~/Downloads (or data dir fallback); returns path.
    pub fn save_export(&self, filename: &str, content: &str) -> Result<String, BrokerError> {
        // ponytail: filename sanitized to a basename; exports only ever land in Downloads.
        let safe: String = filename
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
            .collect();
        if safe.is_empty() || safe.starts_with('.') {
            return Err(BrokerError::validation("Invalid file name."));
        }
        let dir = dirs_download().unwrap_or_else(|| self.data_dir.clone());
        let path = dir.join(safe);
        std::fs::write(&path, content).map_err(|e| BrokerError::storage(e.to_string()))?;
        Ok(path.to_string_lossy().to_string())
    }

    pub fn recent_syncs(
        &self,
        connection_id: &str,
        limit: u32,
    ) -> Result<Vec<serde_json::Value>, BrokerError> {
        let conn = self.db.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT trigger, success, detail, started_at, finished_at FROM synchronization_runs
             WHERE connection_id=?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows: Vec<_> = stmt
            .query_map(rusqlite::params![connection_id, limit], |r| {
                Ok(serde_json::json!({
                    "trigger": r.get::<_, String>(0)?,
                    "success": r.get::<_, i64>(1)? != 0,
                    "detail": r.get::<_, Option<String>>(2)?,
                    "started_at": r.get::<_, String>(3)?,
                    "finished_at": r.get::<_, String>(4)?,
                }))
            })?
            .filter_map(Result::ok)
            .collect();
        drop(stmt);
        Ok(rows)
    }
}

fn row_to_submitted(r: &rusqlite::Row) -> rusqlite::Result<SubmittedOrderDto> {
    Ok(SubmittedOrderDto {
        client_order_id: r.get(0)?,
        run_id: r.get(1)?,
        broker_order_id: r.get(2)?,
        symbol: r.get(3)?,
        side: r.get(4)?,
        quantity: r.get(5)?,
        filled_quantity: r.get(6)?,
        avg_fill_price: r.get(7)?,
        status: r.get(8)?,
        message: r.get(9)?,
        submitted_at: r.get(10)?,
        updated_at: r.get(11)?,
    })
}

fn dirs_download() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join("Downloads"))
        .filter(|p| p.is_dir())
}

// Momentum demo: bundled deterministic sample data, educational only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MomentumRow {
    pub symbol: String,
    /// Trailing 6-month return from the bundled sample dataset (not live data).
    pub sample_return_percent: f64,
    pub selected: bool,
}

/// Educational example, not investment advice. Ranks a fixed sample universe
/// by bundled historical sample returns and equal-weights the top half.
pub fn momentum_demo_rows() -> Vec<MomentumRow> {
    let data = [
        ("NVDA", 38.2),
        ("META", 24.7),
        ("AAPL", 12.1),
        ("MSFT", 9.8),
        ("GOOGL", 7.4),
        ("AMZN", 6.9),
        ("V", 4.2),
        ("JPM", 3.1),
        ("JNJ", -2.4),
        ("XOM", -5.8),
    ];
    let mut rows: Vec<MomentumRow> = data
        .iter()
        .map(|(s, r)| MomentumRow {
            symbol: s.to_string(),
            sample_return_percent: *r,
            selected: false,
        })
        .collect();
    rows.sort_by(|a, b| {
        b.sample_return_percent
            .partial_cmp(&a.sample_return_percent)
            .unwrap()
    });
    let half = rows.len() / 2;
    for row in rows.iter_mut().take(half) {
        row.selected = true;
    }
    rows
}

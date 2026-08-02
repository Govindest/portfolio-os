//! SQLite persistence: versioned migrations, foreign keys on, UTC timestamps,
//! transactional writes. No credentials are ever stored here.

use crate::errors::BrokerError;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

pub struct Db {
    pub conn: Mutex<Connection>,
}

/// Ordered, versioned migrations. Never edit an entry after release — append.
const MIGRATIONS: &[&str] = &[
    // v1: initial schema
    r#"
    CREATE TABLE app_settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    CREATE TABLE onboarding_state (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        completed INTEGER NOT NULL DEFAULT 0,
        step TEXT NOT NULL DEFAULT 'welcome',
        updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    CREATE TABLE brokerage_connections (
        id TEXT PRIMARY KEY,
        broker_kind TEXT NOT NULL,
        label TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'connected',
        paper INTEGER NOT NULL DEFAULT 1,
        auto_sync_minutes INTEGER,
        default_account_id TEXT,
        gateway_url TEXT,
        masked_credential TEXT,
        last_sync_at TEXT,
        last_error TEXT,
        created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    CREATE TABLE brokerage_accounts (
        id TEXT NOT NULL,
        connection_id TEXT NOT NULL REFERENCES brokerage_connections(id) ON DELETE CASCADE,
        name TEXT NOT NULL,
        paper INTEGER NOT NULL DEFAULT 1,
        currency TEXT NOT NULL DEFAULT 'USD',
        PRIMARY KEY (connection_id, id)
    );

    CREATE TABLE account_snapshots (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        connection_id TEXT NOT NULL REFERENCES brokerage_connections(id) ON DELETE CASCADE,
        account_id TEXT NOT NULL,
        portfolio_value REAL NOT NULL,
        cash REAL NOT NULL,
        buying_power REAL NOT NULL,
        daily_change_percent REAL,
        taken_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    CREATE TABLE positions (
        connection_id TEXT NOT NULL REFERENCES brokerage_connections(id) ON DELETE CASCADE,
        account_id TEXT NOT NULL,
        symbol TEXT NOT NULL,
        name TEXT,
        quantity REAL NOT NULL,
        current_price REAL NOT NULL,
        market_value REAL NOT NULL,
        cost_basis REAL,
        unrealized_pl REAL,
        updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        PRIMARY KEY (connection_id, account_id, symbol)
    );

    CREATE TABLE position_snapshots (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        connection_id TEXT NOT NULL REFERENCES brokerage_connections(id) ON DELETE CASCADE,
        account_id TEXT NOT NULL,
        symbol TEXT NOT NULL,
        quantity REAL NOT NULL,
        market_value REAL NOT NULL,
        taken_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    CREATE TABLE strategies (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        kind TEXT NOT NULL,
        current_version INTEGER NOT NULL DEFAULT 1,
        archived INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    CREATE TABLE strategy_versions (
        strategy_id TEXT NOT NULL REFERENCES strategies(id) ON DELETE CASCADE,
        version INTEGER NOT NULL,
        yaml TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        PRIMARY KEY (strategy_id, version)
    );

    CREATE TABLE strategy_assets (
        strategy_id TEXT NOT NULL REFERENCES strategies(id) ON DELETE CASCADE,
        symbol TEXT NOT NULL,
        target_weight REAL NOT NULL,
        PRIMARY KEY (strategy_id, symbol)
    );

    CREATE TABLE rebalance_runs (
        id TEXT PRIMARY KEY,
        connection_id TEXT NOT NULL REFERENCES brokerage_connections(id) ON DELETE CASCADE,
        account_id TEXT NOT NULL,
        strategy_id TEXT REFERENCES strategies(id) ON DELETE SET NULL,
        status TEXT NOT NULL DEFAULT 'proposed',
        summary_json TEXT NOT NULL,
        created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    CREATE TABLE proposed_orders (
        id TEXT PRIMARY KEY,
        run_id TEXT NOT NULL REFERENCES rebalance_runs(id) ON DELETE CASCADE,
        symbol TEXT NOT NULL,
        side TEXT NOT NULL,
        quantity REAL NOT NULL,
        estimated_value REAL NOT NULL,
        current_weight REAL NOT NULL,
        target_weight REAL NOT NULL,
        drift REAL NOT NULL,
        reason TEXT NOT NULL,
        warnings_json TEXT NOT NULL DEFAULT '[]',
        valid INTEGER NOT NULL DEFAULT 1,
        validation_message TEXT
    );

    CREATE TABLE submitted_orders (
        client_order_id TEXT PRIMARY KEY,
        run_id TEXT REFERENCES rebalance_runs(id) ON DELETE SET NULL,
        connection_id TEXT NOT NULL REFERENCES brokerage_connections(id) ON DELETE CASCADE,
        account_id TEXT NOT NULL,
        broker_order_id TEXT,
        symbol TEXT NOT NULL,
        side TEXT NOT NULL,
        quantity REAL NOT NULL,
        filled_quantity REAL NOT NULL DEFAULT 0,
        avg_fill_price REAL,
        status TEXT NOT NULL,
        message TEXT,
        submitted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
        updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    CREATE TABLE order_events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        client_order_id TEXT NOT NULL REFERENCES submitted_orders(client_order_id) ON DELETE CASCADE,
        status TEXT NOT NULL,
        detail TEXT,
        created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    CREATE TABLE synchronization_runs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        connection_id TEXT NOT NULL REFERENCES brokerage_connections(id) ON DELETE CASCADE,
        trigger TEXT NOT NULL,
        success INTEGER NOT NULL,
        detail TEXT,
        started_at TEXT NOT NULL,
        finished_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    CREATE TABLE audit_events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        event_type TEXT NOT NULL,
        description TEXT NOT NULL,
        strategy_id TEXT,
        connection_id TEXT,
        account_id TEXT,
        order_id TEXT,
        technical_json TEXT,
        created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    );

    CREATE INDEX idx_audit_created ON audit_events(created_at DESC);
    CREATE INDEX idx_submitted_run ON submitted_orders(run_id);
    CREATE INDEX idx_snapshots_account ON account_snapshots(connection_id, account_id, taken_at);
    "#,
];

impl Db {
    pub fn open(path: &Path) -> Result<Self, BrokerError> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, BrokerError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, BrokerError> {
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Db {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), BrokerError> {
        let mut conn = self.conn.lock().unwrap();
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate() {
            let target = (i + 1) as i64;
            if version < target {
                let tx = conn.transaction()?;
                tx.execute_batch(sql)?;
                tx.pragma_update(None, "user_version", target)?;
                tx.commit()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_cleanly_and_are_idempotent() {
        let db = Db::open_in_memory().unwrap();
        // Re-running migrate must be a no-op.
        db.migrate().unwrap();
        let conn = db.conn.lock().unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
        // All required tables exist.
        for table in [
            "app_settings",
            "onboarding_state",
            "brokerage_connections",
            "brokerage_accounts",
            "account_snapshots",
            "positions",
            "position_snapshots",
            "strategies",
            "strategy_versions",
            "strategy_assets",
            "rebalance_runs",
            "proposed_orders",
            "submitted_orders",
            "order_events",
            "synchronization_runs",
            "audit_events",
        ] {
            let n: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "missing table {table}");
        }
    }

    #[test]
    fn foreign_keys_cascade_on_connection_delete() {
        let db = Db::open_in_memory().unwrap();
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO brokerage_connections (id, broker_kind, label) VALUES ('c1','mock','Demo')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO brokerage_accounts (id, connection_id, name) VALUES ('a1','c1','Acct')",
            [],
        )
        .unwrap();
        conn.execute("DELETE FROM brokerage_connections WHERE id='c1'", [])
            .unwrap();
        let n: i64 = conn
            .query_row("SELECT count(*) FROM brokerage_accounts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}

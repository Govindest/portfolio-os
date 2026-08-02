//! Local audit trail. Every consequential action lands here. Never contains
//! credentials or full secrets.

use crate::db::Db;
use crate::errors::BrokerError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: i64,
    pub event_type: String,
    pub description: String,
    pub strategy_id: Option<String>,
    pub connection_id: Option<String>,
    pub account_id: Option<String>,
    pub order_id: Option<String>,
    pub technical: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Default, Clone)]
pub struct AuditRefs {
    pub strategy_id: Option<String>,
    pub connection_id: Option<String>,
    pub account_id: Option<String>,
    pub order_id: Option<String>,
}

pub fn record(
    db: &Db,
    event_type: &str,
    description: &str,
    refs: AuditRefs,
    technical: Option<serde_json::Value>,
) -> Result<(), BrokerError> {
    let conn = db.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO audit_events (event_type, description, strategy_id, connection_id, account_id, order_id, technical_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            event_type,
            description,
            refs.strategy_id,
            refs.connection_id,
            refs.account_id,
            refs.order_id,
            technical.map(|t| t.to_string()),
        ],
    )?;
    Ok(())
}

pub fn list(
    db: &Db,
    limit: u32,
    event_type: Option<String>,
) -> Result<Vec<AuditEvent>, BrokerError> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, event_type, description, strategy_id, connection_id, account_id, order_id, technical_json, created_at
         FROM audit_events
         WHERE (?1 IS NULL OR event_type = ?1)
         ORDER BY id DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![event_type, limit], |r| {
        Ok(AuditEvent {
            id: r.get(0)?,
            event_type: r.get(1)?,
            description: r.get(2)?,
            strategy_id: r.get(3)?,
            connection_id: r.get(4)?,
            account_id: r.get(5)?,
            order_id: r.get(6)?,
            technical: r
                .get::<_, Option<String>>(7)?
                .and_then(|s| serde_json::from_str(&s).ok()),
            created_at: r.get(8)?,
        })
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

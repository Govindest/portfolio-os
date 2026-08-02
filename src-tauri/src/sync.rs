//! Background synchronization. Read-only by design: it refreshes balances,
//! positions, orders, and fills. It can never submit trades.

use crate::service::AppService;
use std::sync::Arc;
use tauri::Emitter;

pub fn spawn(app: tauri::AppHandle, svc: Arc<AppService>) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let Ok(settings) = svc.get_settings() else {
                continue;
            };
            let Some(interval) = settings.sync_interval_minutes else {
                continue;
            };
            let Ok(connections) = svc.list_connections() else {
                continue;
            };
            for c in connections {
                if c.broker_kind == "manual" {
                    continue;
                }
                let due = match &c.last_sync_at {
                    None => true,
                    Some(ts) => chrono::DateTime::parse_from_rfc3339(ts)
                        .map(|t| {
                            (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_minutes()
                                >= interval as i64
                        })
                        .unwrap_or(true),
                };
                if due {
                    let _ = svc.sync_connection(&c.id, "background").await;
                    let _ = app.emit("sync-completed", &c.id);
                }
            }
        }
    });
}

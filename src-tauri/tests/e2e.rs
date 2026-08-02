//! End-to-end tests over the service layer: the exact flows the UI drives,
//! against a real SQLite file and the deterministic mock broker. No network
//! and no real brokerage credentials required.

use portfolio_os_lib::broker::alpaca::AlpacaBroker;
use portfolio_os_lib::broker::{BrokerAdapter, OrderSide};
use portfolio_os_lib::credentials::{BrokerSecrets, CredentialStore, MemoryStore};
use portfolio_os_lib::db::Db;
use portfolio_os_lib::errors::BrokerError;
use portfolio_os_lib::service::AppService;
use portfolio_os_lib::strategy;
use std::io::{Read, Write};
use std::sync::Arc;

fn service(dir: &std::path::Path) -> AppService {
    let db = Db::open(&dir.join("test.sqlite3")).unwrap();
    AppService::new(db, Box::new(MemoryStore::default()), dir.to_path_buf())
}

#[tokio::test]
async fn full_demo_flow_first_launch_to_fills() {
    let dir = tempfile::tempdir().unwrap();
    let svc = service(dir.path());

    // 1. First launch: onboarding incomplete, no connections.
    let (completed, step) = svc.get_onboarding().unwrap();
    assert!(!completed);
    assert_eq!(step, "welcome");
    assert!(svc.list_connections().unwrap().is_empty());

    // 2. Demo-mode onboarding: connect the mock broker.
    let conn = svc
        .create_connection("mock", "Demo Portfolio", BrokerSecrets::default(), None)
        .await
        .unwrap();
    assert_eq!(conn.broker_kind, "mock");
    assert!(conn.paper);
    assert_eq!(conn.accounts.len(), 1);
    svc.set_onboarding(true, "done").unwrap();

    let account_id = conn.accounts[0].id.clone();
    let dash = svc.get_dashboard(&conn.id, &account_id).unwrap();
    let summary = dash.summary.expect("initial sync populated a snapshot");
    assert!(summary.portfolio_value > 0.0);
    assert!(!dash.positions.is_empty());
    assert!(!dash.history.is_empty(), "demo history seeded");

    // 3. Create an equal-weight strategy.
    let doc = strategy::equal_weight(
        "Demo Equal Weight",
        Some("test".into()),
        &["AAPL".into(), "MSFT".into(), "VOO".into(), "BND".into()],
    )
    .unwrap();
    let saved = svc.save_strategy("equal_weight", &doc, None).unwrap();
    assert_eq!(saved.asset_count, 4);

    // 4. Generate a rebalance.
    let run = svc
        .run_rebalance(&conn.id, &account_id, &saved.id)
        .await
        .unwrap();
    assert!(
        !run.plan.orders.is_empty(),
        "drifted demo portfolio proposes orders"
    );
    for o in &run.plan.orders {
        assert!(!o.reason.is_empty(), "every order carries an explanation");
    }

    // Approval is required: refusing acknowledgment must fail.
    assert!(svc.approve_and_submit(&run.id, false).await.is_err());

    // 5. Approve and submit.
    let submitted = svc.approve_and_submit(&run.id, true).await.unwrap();
    assert_eq!(submitted.len(), run.plan.orders.len());

    // Duplicate submission is idempotent — same orders, no new ones.
    let again = svc.approve_and_submit(&run.id, true).await.unwrap();
    assert_eq!(again.len(), submitted.len());

    // 6. Wait past the mock fill delay, sync, and verify fills reconciled.
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    svc.sync_connection(&conn.id, "manual").await.unwrap();
    let orders = svc.orders_for_run(&run.id).unwrap();
    assert!(
        orders
            .iter()
            .all(|o| o.status == "filled" || o.status == "rejected"),
        "orders settled: {orders:?}"
    );
    assert!(orders.iter().any(|o| o.status == "filled"));

    // Audit trail captured the whole journey.
    let events = portfolio_os_lib::audit::list(&svc.db, 100, None).unwrap();
    let types: Vec<&str> = events.iter().map(|e| e.event_type.as_str()).collect();
    for expected in [
        "connection_created",
        "strategy_created",
        "rebalance_calculated",
        "orders_approved",
        "order_submitted",
        "sync_completed",
    ] {
        assert!(
            types.contains(&expected),
            "missing audit event {expected}: {types:?}"
        );
    }
}

#[tokio::test]
async fn emergency_stop_blocks_submission() {
    let dir = tempfile::tempdir().unwrap();
    let svc = service(dir.path());
    let conn = svc
        .create_connection("mock", "Demo", BrokerSecrets::default(), None)
        .await
        .unwrap();
    let account_id = conn.accounts[0].id.clone();
    let doc = strategy::equal_weight("EW", None, &["AAPL".into(), "MSFT".into()]).unwrap();
    let saved = svc.save_strategy("equal_weight", &doc, None).unwrap();
    let run = svc
        .run_rebalance(&conn.id, &account_id, &saved.id)
        .await
        .unwrap();

    svc.set_emergency_stop(true).unwrap();
    let err = svc.approve_and_submit(&run.id, true).await.unwrap_err();
    assert!(err.user_message().contains("Emergency stop"));

    svc.set_emergency_stop(false).unwrap();
    assert!(svc.approve_and_submit(&run.id, true).await.is_ok());
}

#[tokio::test]
async fn read_only_mode_blocks_submission() {
    let dir = tempfile::tempdir().unwrap();
    let svc = service(dir.path());
    let conn = svc
        .create_connection("mock", "Demo", BrokerSecrets::default(), None)
        .await
        .unwrap();
    let account_id = conn.accounts[0].id.clone();
    let doc = strategy::equal_weight("EW", None, &["AAPL".into(), "MSFT".into()]).unwrap();
    let saved = svc.save_strategy("equal_weight", &doc, None).unwrap();
    let run = svc
        .run_rebalance(&conn.id, &account_id, &saved.id)
        .await
        .unwrap();

    let mut settings = svc.get_settings().unwrap();
    settings.read_only = true;
    svc.save_settings(&settings).unwrap();
    let err = svc.approve_and_submit(&run.id, true).await.unwrap_err();
    assert!(err.user_message().contains("read-only"));
}

/// Serve one canned HTTP response on a local port, then exit.
fn one_shot_http(status_line: &'static str, body: &'static str) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(4) {
            let Ok(mut s) = stream else { continue };
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 {status_line}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = s.write_all(resp.as_bytes());
        }
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn alpaca_invalid_credentials_map_to_friendly_error() {
    // 7/8. A mocked Alpaca endpoint rejecting the key: users see guidance, not raw JSON.
    let base = one_shot_http(
        "401 Unauthorized",
        r#"{"message":"access key verification failed"}"#,
    );
    let broker = AlpacaBroker::with_bases("bad".into(), "creds".into(), base.clone(), base);
    let err = broker.test_connection().await.unwrap_err();
    match &err {
        BrokerError::Authentication(d) => {
            assert!(d.user_message.contains("paper-trading credentials"));
            assert!(d.technical.as_deref().unwrap_or("").contains("401"));
        }
        other => panic!("expected Authentication, got {other:?}"),
    }
}

#[tokio::test]
async fn alpaca_happy_path_against_mock_endpoint() {
    let base = one_shot_http(
        "200 OK",
        r#"{"account_number":"PA123","currency":"USD","equity":"1000","last_equity":"990","cash":"500","buying_power":"1000","status":"ACTIVE"}"#,
    );
    let broker = AlpacaBroker::with_bases("k".into(), "s".into(), base.clone(), base);
    let status = broker.test_connection().await.unwrap();
    assert!(status.connected);
    assert!(status.paper);
    let accounts = broker.get_accounts().await.unwrap();
    assert_eq!(accounts[0].id, "PA123");
}

#[tokio::test]
async fn disconnect_removes_credentials_from_store() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::default());
    struct Shared(Arc<MemoryStore>);
    impl CredentialStore for Shared {
        fn save(&self, id: &str, s: &BrokerSecrets) -> Result<(), BrokerError> {
            self.0.save(id, s)
        }
        fn get(&self, id: &str) -> Result<Option<BrokerSecrets>, BrokerError> {
            self.0.get(id)
        }
        fn delete(&self, id: &str) -> Result<(), BrokerError> {
            self.0.delete(id)
        }
    }
    let db = Db::open(&dir.path().join("t.sqlite3")).unwrap();
    let svc = AppService::new(
        db,
        Box::new(Shared(store.clone())),
        dir.path().to_path_buf(),
    );

    // Mock broker with fake credentials saved to prove deletion happens.
    let conn = svc
        .create_connection(
            "mock",
            "Demo",
            BrokerSecrets {
                key: "k".into(),
                secret: "s".into(),
            },
            None,
        )
        .await
        .unwrap();
    assert!(store.get(&conn.id).unwrap().is_some());
    svc.disconnect(&conn.id).unwrap();
    assert!(store.get(&conn.id).unwrap().is_none());
    assert!(svc.list_connections().unwrap().is_empty());
}

#[tokio::test]
async fn restart_persistence() {
    // 10. Everything survives an app restart (new service over the same file).
    let dir = tempfile::tempdir().unwrap();
    let conn_id;
    let strategy_id;
    {
        let svc = service(dir.path());
        let conn = svc
            .create_connection("mock", "Demo", BrokerSecrets::default(), None)
            .await
            .unwrap();
        conn_id = conn.id.clone();
        let doc =
            strategy::equal_weight("Persisted", None, &["AAPL".into(), "VOO".into()]).unwrap();
        strategy_id = svc.save_strategy("equal_weight", &doc, None).unwrap().id;
        svc.set_onboarding(true, "done").unwrap();
    }
    let svc2 = service(dir.path());
    let (completed, _) = svc2.get_onboarding().unwrap();
    assert!(completed);
    let conns = svc2.list_connections().unwrap();
    assert_eq!(conns.len(), 1);
    assert_eq!(conns[0].id, conn_id);
    let s = svc2.get_strategy(&strategy_id).unwrap();
    assert_eq!(s.name, "Persisted");
}

#[tokio::test]
async fn manual_portfolio_csv_import_and_rebalance_export() {
    let dir = tempfile::tempdir().unwrap();
    let svc = service(dir.path());
    let conn = svc
        .create_connection("manual", "My 401k", BrokerSecrets::default(), None)
        .await
        .unwrap();
    svc.set_manual_cash(&conn.id, 5_000.0).unwrap();
    let n = svc
        .import_positions_csv(&conn.id, "symbol,quantity,price\nAAPL,10,200\nVOO,5,500\n")
        .unwrap();
    assert_eq!(n, 2);
    let positions = svc.stored_positions(&conn.id, "manual").unwrap();
    assert_eq!(positions.len(), 2);

    let doc = strategy::equal_weight("Manual EW", None, &["AAPL".into(), "VOO".into()]).unwrap();
    let saved = svc.save_strategy("equal_weight", &doc, None).unwrap();
    let run = svc
        .run_rebalance(&conn.id, "manual", &saved.id)
        .await
        .unwrap();
    // Manual portfolios never submit — they export a trade list instead.
    let err = svc.approve_and_submit(&run.id, true).await.unwrap_err();
    assert!(err.user_message().contains("Export the trade list"));
    let csv = svc.proposed_orders_csv(&run.id).unwrap();
    assert!(csv.starts_with("symbol,side,quantity"));
}

#[tokio::test]
async fn mock_order_cancellation_and_sell_side() {
    let dir = tempfile::tempdir().unwrap();
    let svc = service(dir.path());
    let conn = svc
        .create_connection("mock", "Demo", BrokerSecrets::default(), None)
        .await
        .unwrap();
    let account_id = conn.accounts[0].id.clone();
    // Strategy that forces selling AAPL entirely (not in strategy).
    let doc = strategy::equal_weight("Bonds", None, &["BND".into()]).unwrap();
    let saved = svc.save_strategy("equal_weight", &doc, None).unwrap();
    let run = svc
        .run_rebalance(&conn.id, &account_id, &saved.id)
        .await
        .unwrap();
    assert!(run.plan.orders.iter().any(|o| o.side == OrderSide::Sell));
    let submitted = svc.approve_and_submit(&run.id, true).await.unwrap();
    // Cancel one accepted order before the fill delay elapses.
    let target = submitted
        .iter()
        .find(|o| o.status == "accepted")
        .expect("accepted order");
    let broker_id = target.broker_order_id.clone().unwrap();
    svc.cancel_order(&conn.id, &broker_id).await.unwrap();
    let orders = svc.orders_for_run(&run.id).unwrap();
    assert!(orders.iter().any(|o| o.status == "canceled"));
}

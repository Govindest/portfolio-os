//! Dev utility: seed the real app-data database with a demo connection and
//! strategy, exactly as onboarding would. Used to exercise the native app
//! against real persisted data.
//!
//! Usage: cargo run --example seed_demo -- "<app-data-dir>"

use portfolio_os_lib::credentials::{BrokerSecrets, KeyringStore};
use portfolio_os_lib::db::Db;
use portfolio_os_lib::service::AppService;
use portfolio_os_lib::strategy;

#[tokio::main]
async fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("app data dir arg"));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Db::open(&dir.join("portfolio-os.sqlite3")).unwrap();
    let svc = AppService::new(db, Box::new(KeyringStore), dir);

    let conn = svc
        .create_connection("mock", "Demo Portfolio", BrokerSecrets::default(), None)
        .await
        .unwrap();
    let doc = strategy::equal_weight(
        "Core Four",
        Some("Equal weight across four core holdings".into()),
        &["AAPL".into(), "MSFT".into(), "VOO".into(), "NVDA".into()],
    )
    .unwrap();
    let saved = svc.save_strategy("equal_weight", &doc, None).unwrap();
    svc.set_onboarding(true, "done").unwrap();

    let run = svc
        .run_rebalance(&conn.id, &conn.accounts[0].id, &saved.id)
        .await
        .unwrap();
    println!(
        "seeded: connection={} strategy={} rebalance_orders={}",
        conn.id,
        saved.id,
        run.plan.orders.len()
    );
}

pub mod audit;
pub mod broker;
pub mod commands;
pub mod credentials;
pub mod db;
pub mod errors;
pub mod rebalance;
pub mod service;
pub mod strategy;
pub mod sync;

use credentials::KeyringStore;
use db::Db;
use service::AppService;
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = Db::open(&data_dir.join("portfolio-os.sqlite3"))
                .map_err(|e| format!("database init failed: {e}"))?;
            let svc = Arc::new(AppService::new(db, Box::new(KeyringStore), data_dir));
            sync::spawn(app.handle().clone(), svc.clone());
            app.manage(svc);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::update_settings,
            commands::set_onboarding,
            commands::set_emergency_stop,
            commands::preview_connection,
            commands::create_connection,
            commands::list_connections,
            commands::test_connection,
            commands::replace_credentials,
            commands::disconnect,
            commands::set_default_account,
            commands::sync_now,
            commands::recent_syncs,
            commands::get_dashboard,
            commands::upsert_manual_position,
            commands::set_manual_cash,
            commands::import_positions_csv,
            commands::export_positions_csv,
            commands::list_strategies,
            commands::save_strategy,
            commands::delete_strategy,
            commands::import_strategy_yaml,
            commands::export_strategy_yaml,
            commands::save_strategy_export,
            commands::example_strategy_yaml,
            commands::get_momentum_demo,
            commands::run_rebalance,
            commands::approve_and_submit,
            commands::orders_for_run,
            commands::list_orders,
            commands::cancel_order,
            commands::cancel_all_open,
            commands::export_orders_csv,
            commands::list_activity,
            commands::reset_demo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PortfolioOS");
}

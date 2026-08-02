//! Tauri command layer — thin wrappers over the service. No business logic here.

use crate::broker::ConnectionStatus;
use crate::credentials::BrokerSecrets;
use crate::errors::BrokerError;
use crate::service::{
    momentum_demo_rows, AppService, AppSettings, ConnectionDto, DashboardDto, MomentumRow,
    RebalanceRunDto, StrategySummary, SubmittedOrderDto,
};
use crate::strategy::{StrategyDoc, EXAMPLE_YAML};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

pub type Svc = Arc<AppService>;

#[derive(Serialize)]
pub struct AppStateDto {
    pub settings: AppSettings,
    pub onboarding_completed: bool,
    pub onboarding_step: String,
    pub connections: Vec<ConnectionDto>,
    pub version: String,
}

#[tauri::command]
pub fn get_app_state(svc: State<'_, Svc>) -> Result<AppStateDto, BrokerError> {
    let (completed, step) = svc.get_onboarding()?;
    Ok(AppStateDto {
        settings: svc.get_settings()?,
        onboarding_completed: completed,
        onboarding_step: step,
        connections: svc.list_connections()?,
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

#[tauri::command]
pub fn update_settings(svc: State<'_, Svc>, settings: AppSettings) -> Result<(), BrokerError> {
    svc.save_settings(&settings)
}

#[tauri::command]
pub fn set_onboarding(
    svc: State<'_, Svc>,
    completed: bool,
    step: String,
) -> Result<(), BrokerError> {
    svc.set_onboarding(completed, &step)
}

#[tauri::command]
pub fn set_emergency_stop(svc: State<'_, Svc>, enabled: bool) -> Result<AppSettings, BrokerError> {
    svc.set_emergency_stop(enabled)
}

// ---------- connections ----------

#[tauri::command]
pub async fn preview_connection(
    svc: State<'_, Svc>,
    kind: String,
    key: String,
    secret: String,
    gateway_url: Option<String>,
) -> Result<ConnectionStatus, BrokerError> {
    svc.preview_connection(&kind, &BrokerSecrets { key, secret }, gateway_url)
        .await
}

#[tauri::command]
pub async fn create_connection(
    svc: State<'_, Svc>,
    kind: String,
    label: String,
    key: String,
    secret: String,
    gateway_url: Option<String>,
) -> Result<ConnectionDto, BrokerError> {
    svc.create_connection(&kind, &label, BrokerSecrets { key, secret }, gateway_url)
        .await
}

#[tauri::command]
pub fn list_connections(svc: State<'_, Svc>) -> Result<Vec<ConnectionDto>, BrokerError> {
    svc.list_connections()
}

#[tauri::command]
pub async fn test_connection(
    svc: State<'_, Svc>,
    id: String,
) -> Result<ConnectionStatus, BrokerError> {
    svc.test_connection(&id).await
}

#[tauri::command]
pub async fn replace_credentials(
    svc: State<'_, Svc>,
    id: String,
    key: String,
    secret: String,
) -> Result<ConnectionStatus, BrokerError> {
    let kind: String = svc.get_connection(&id)?.broker_kind;
    let secrets = BrokerSecrets { key, secret };
    // Verify new credentials before replacing the stored ones.
    let status = svc.preview_connection(&kind, &secrets, None).await?;
    svc.creds.save(&id, &secrets)?;
    Ok(status)
}

#[tauri::command]
pub fn disconnect(svc: State<'_, Svc>, id: String) -> Result<(), BrokerError> {
    svc.disconnect(&id)
}

#[tauri::command]
pub fn set_default_account(
    svc: State<'_, Svc>,
    connection_id: String,
    account_id: String,
) -> Result<(), BrokerError> {
    svc.set_default_account(&connection_id, &account_id)
}

#[tauri::command]
pub async fn sync_now(svc: State<'_, Svc>, id: String) -> Result<(), BrokerError> {
    svc.sync_connection(&id, "manual").await
}

#[tauri::command]
pub fn recent_syncs(
    svc: State<'_, Svc>,
    id: String,
) -> Result<Vec<serde_json::Value>, BrokerError> {
    svc.recent_syncs(&id, 10)
}

// ---------- dashboard / portfolio ----------

#[tauri::command]
pub fn get_dashboard(
    svc: State<'_, Svc>,
    connection_id: String,
    account_id: String,
) -> Result<DashboardDto, BrokerError> {
    svc.get_dashboard(&connection_id, &account_id)
}

#[tauri::command]
pub fn upsert_manual_position(
    svc: State<'_, Svc>,
    connection_id: String,
    symbol: String,
    quantity: f64,
    price: f64,
    cost_basis: Option<f64>,
) -> Result<(), BrokerError> {
    svc.upsert_manual_position(&connection_id, &symbol, quantity, price, cost_basis)
}

#[tauri::command]
pub fn set_manual_cash(
    svc: State<'_, Svc>,
    connection_id: String,
    cash: f64,
) -> Result<(), BrokerError> {
    svc.set_manual_cash(&connection_id, cash)
}

#[tauri::command]
pub fn import_positions_csv(
    svc: State<'_, Svc>,
    connection_id: String,
    content: String,
) -> Result<u32, BrokerError> {
    svc.import_positions_csv(&connection_id, &content)
}

#[tauri::command]
pub fn export_positions_csv(
    svc: State<'_, Svc>,
    connection_id: String,
    account_id: String,
) -> Result<String, BrokerError> {
    let csv = svc.positions_csv(&connection_id, &account_id)?;
    svc.save_export("portfolio-positions.csv", &csv)
}

// ---------- strategies ----------

#[tauri::command]
pub fn list_strategies(svc: State<'_, Svc>) -> Result<Vec<StrategySummary>, BrokerError> {
    svc.list_strategies()
}

#[tauri::command]
pub fn save_strategy(
    svc: State<'_, Svc>,
    kind: String,
    doc: StrategyDoc,
    existing_id: Option<String>,
) -> Result<StrategySummary, BrokerError> {
    svc.save_strategy(&kind, &doc, existing_id)
}

#[tauri::command]
pub fn delete_strategy(svc: State<'_, Svc>, id: String) -> Result<(), BrokerError> {
    svc.delete_strategy(&id)
}

#[tauri::command]
pub fn import_strategy_yaml(
    svc: State<'_, Svc>,
    yaml: String,
) -> Result<StrategySummary, BrokerError> {
    svc.import_strategy_yaml(&yaml)
}

#[tauri::command]
pub fn export_strategy_yaml(svc: State<'_, Svc>, id: String) -> Result<String, BrokerError> {
    svc.export_strategy_yaml(&id)
}

#[tauri::command]
pub fn save_strategy_export(svc: State<'_, Svc>, id: String) -> Result<String, BrokerError> {
    let yaml = svc.export_strategy_yaml(&id)?;
    let name = svc.get_strategy(&id)?.name.to_lowercase().replace(' ', "-");
    svc.save_export(&format!("{name}.strategy.yaml"), &yaml)
}

#[tauri::command]
pub fn example_strategy_yaml() -> String {
    EXAMPLE_YAML.to_string()
}

#[tauri::command]
pub fn get_momentum_demo() -> Vec<MomentumRow> {
    momentum_demo_rows()
}

// ---------- rebalance & orders ----------

#[tauri::command]
pub async fn run_rebalance(
    svc: State<'_, Svc>,
    connection_id: String,
    account_id: String,
    strategy_id: String,
) -> Result<RebalanceRunDto, BrokerError> {
    svc.run_rebalance(&connection_id, &account_id, &strategy_id)
        .await
}

#[tauri::command]
pub async fn approve_and_submit(
    svc: State<'_, Svc>,
    run_id: String,
    acknowledged: bool,
) -> Result<Vec<SubmittedOrderDto>, BrokerError> {
    svc.approve_and_submit(&run_id, acknowledged).await
}

#[tauri::command]
pub fn orders_for_run(
    svc: State<'_, Svc>,
    run_id: String,
) -> Result<Vec<SubmittedOrderDto>, BrokerError> {
    svc.orders_for_run(&run_id)
}

#[tauri::command]
pub fn list_orders(
    svc: State<'_, Svc>,
    connection_id: Option<String>,
    limit: u32,
) -> Result<Vec<SubmittedOrderDto>, BrokerError> {
    svc.list_submitted_orders(connection_id.as_deref(), limit)
}

#[tauri::command]
pub async fn cancel_order(
    svc: State<'_, Svc>,
    connection_id: String,
    broker_order_id: String,
) -> Result<(), BrokerError> {
    svc.cancel_order(&connection_id, &broker_order_id).await
}

#[tauri::command]
pub async fn cancel_all_open(
    svc: State<'_, Svc>,
    connection_id: String,
) -> Result<u32, BrokerError> {
    svc.cancel_all_open(&connection_id).await
}

#[tauri::command]
pub fn export_orders_csv(svc: State<'_, Svc>, run_id: String) -> Result<String, BrokerError> {
    let csv = svc.proposed_orders_csv(&run_id)?;
    svc.save_export("proposed-orders.csv", &csv)
}

// ---------- activity & demo ----------

#[tauri::command]
pub fn list_activity(
    svc: State<'_, Svc>,
    limit: u32,
    event_type: Option<String>,
) -> Result<Vec<crate::audit::AuditEvent>, BrokerError> {
    crate::audit::list(&svc.db, limit, event_type)
}

#[tauri::command]
pub fn reset_demo(svc: State<'_, Svc>) -> Result<(), BrokerError> {
    svc.reset_demo()
}

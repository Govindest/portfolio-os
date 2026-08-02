// Typed bridge to the Rust backend. Every UI data access goes through here.
import { invoke } from "@tauri-apps/api/core";

// ---------- types mirrored from Rust ----------

export interface SafetySettings {
  max_order_value: number;
  max_batch_value: number;
  max_position_weight: number;
  min_order_value: number;
  allow_fractional: boolean;
  allow_sells: boolean;
}

export interface AppSettings {
  safety: SafetySettings;
  sync_interval_minutes: number | null;
  theme: string;
  read_only: boolean;
  emergency_stop: boolean;
}

export interface AccountDto {
  id: string;
  name: string;
  paper: boolean;
  currency: string;
}

export interface ConnectionDto {
  id: string;
  broker_kind: string;
  label: string;
  status: string;
  paper: boolean;
  default_account_id: string | null;
  masked_credential: string | null;
  last_sync_at: string | null;
  last_error: string | null;
  accounts: AccountDto[];
  supports_trading: boolean;
  created_at: string;
}

export interface AppState {
  settings: AppSettings;
  onboarding_completed: boolean;
  onboarding_step: string;
  connections: ConnectionDto[];
  version: string;
}

export interface ConnectionStatus {
  connected: boolean;
  paper: boolean;
  message: string;
}

export interface AccountSummary {
  account_id: string;
  portfolio_value: number;
  cash: number;
  buying_power: number;
  currency: string;
  daily_change_percent: number | null;
}

export interface PositionDto {
  symbol: string;
  name: string | null;
  quantity: number;
  current_price: number;
  market_value: number;
  cost_basis: number | null;
  unrealized_pl: number | null;
}

export interface StrategyAsset {
  symbol: string;
  target_weight: number;
}

export interface StrategyDoc {
  schema_version: number;
  name: string;
  description?: string | null;
  assets: StrategyAsset[];
  rebalance: {
    method: string;
    drift_threshold_percent: number;
    minimum_order_value: number;
    allow_fractional_shares: boolean;
  };
  risk_controls: {
    maximum_position_weight: number;
    maximum_order_value: number;
    maximum_batch_value: number;
  };
}

export interface StrategySummary {
  id: string;
  name: string;
  description: string | null;
  kind: string;
  current_version: number;
  asset_count: number;
  updated_at: string;
  doc: StrategyDoc;
}

export interface ProposedOrderDto {
  symbol: string;
  side: "buy" | "sell";
  quantity: number;
  estimated_value: number;
  current_weight: number;
  target_weight: number;
  drift: number;
  reason: string;
  warnings: string[];
}

export interface RebalancePlan {
  orders: ProposedOrderDto[];
  skipped: { symbol: string; reason: string }[];
  warnings: string[];
  total_buy_value: number;
  total_sell_value: number;
  estimated_cash_after: number;
  portfolio_value: number;
}

export interface RebalanceRun {
  id: string;
  connection_id: string;
  account_id: string;
  strategy_id: string | null;
  status: string;
  plan: RebalancePlan;
  created_at: string;
}

export interface SubmittedOrderDto {
  client_order_id: string;
  run_id: string | null;
  broker_order_id: string | null;
  symbol: string;
  side: string;
  quantity: number;
  filled_quantity: number;
  avg_fill_price: number | null;
  status: string;
  message: string | null;
  submitted_at: string;
  updated_at: string;
}

export interface HistoryPoint {
  taken_at: string;
  portfolio_value: number;
}

export interface Dashboard {
  summary: AccountSummary | null;
  positions: PositionDto[];
  recent_orders: SubmittedOrderDto[];
  drift_percent: number | null;
  active_strategy: StrategySummary | null;
  history: HistoryPoint[];
}

export interface AuditEvent {
  id: number;
  event_type: string;
  description: string;
  strategy_id: string | null;
  connection_id: string | null;
  account_id: string | null;
  order_id: string | null;
  technical: unknown;
  created_at: string;
}

export interface MomentumRow {
  symbol: string;
  sample_return_percent: number;
  selected: boolean;
}

/** Backend errors arrive as { category, detail: { user_message, technical } }. */
export interface BackendError {
  category: string;
  detail: { user_message: string; technical: string | null };
}

export function errorMessage(e: unknown): string {
  const err = e as BackendError;
  if (err && typeof err === "object" && "detail" in err && err.detail?.user_message) {
    return err.detail.user_message;
  }
  return typeof e === "string" ? e : "Something went wrong. Please try again.";
}

export function errorTechnical(e: unknown): string | null {
  const err = e as BackendError;
  if (err && typeof err === "object" && "detail" in err) return err.detail?.technical ?? null;
  return null;
}

// ---------- commands ----------

export const api = {
  getAppState: () => invoke<AppState>("get_app_state"),
  updateSettings: (settings: AppSettings) => invoke<void>("update_settings", { settings }),
  setOnboarding: (completed: boolean, step: string) =>
    invoke<void>("set_onboarding", { completed, step }),
  setEmergencyStop: (enabled: boolean) => invoke<AppSettings>("set_emergency_stop", { enabled }),

  previewConnection: (kind: string, key: string, secret: string, gatewayUrl?: string) =>
    invoke<ConnectionStatus>("preview_connection", {
      kind,
      key,
      secret,
      gatewayUrl: gatewayUrl ?? null,
    }),
  createConnection: (
    kind: string,
    label: string,
    key: string,
    secret: string,
    gatewayUrl?: string,
  ) =>
    invoke<ConnectionDto>("create_connection", {
      kind,
      label,
      key,
      secret,
      gatewayUrl: gatewayUrl ?? null,
    }),
  listConnections: () => invoke<ConnectionDto[]>("list_connections"),
  testConnection: (id: string) => invoke<ConnectionStatus>("test_connection", { id }),
  replaceCredentials: (id: string, key: string, secret: string) =>
    invoke<ConnectionStatus>("replace_credentials", { id, key, secret }),
  disconnect: (id: string) => invoke<void>("disconnect", { id }),
  setDefaultAccount: (connectionId: string, accountId: string) =>
    invoke<void>("set_default_account", { connectionId, accountId }),
  syncNow: (id: string) => invoke<void>("sync_now", { id }),
  recentSyncs: (id: string) =>
    invoke<
      {
        trigger: string;
        success: boolean;
        detail: string | null;
        started_at: string;
        finished_at: string;
      }[]
    >("recent_syncs", { id }),

  getDashboard: (connectionId: string, accountId: string) =>
    invoke<Dashboard>("get_dashboard", { connectionId, accountId }),
  upsertManualPosition: (
    connectionId: string,
    symbol: string,
    quantity: number,
    price: number,
    costBasis?: number,
  ) =>
    invoke<void>("upsert_manual_position", {
      connectionId,
      symbol,
      quantity,
      price,
      costBasis: costBasis ?? null,
    }),
  setManualCash: (connectionId: string, cash: number) =>
    invoke<void>("set_manual_cash", { connectionId, cash }),
  importPositionsCsv: (connectionId: string, content: string) =>
    invoke<number>("import_positions_csv", { connectionId, content }),
  exportPositionsCsv: (connectionId: string, accountId: string) =>
    invoke<string>("export_positions_csv", { connectionId, accountId }),

  listStrategies: () => invoke<StrategySummary[]>("list_strategies"),
  saveStrategy: (kind: string, doc: StrategyDoc, existingId?: string) =>
    invoke<StrategySummary>("save_strategy", { kind, doc, existingId: existingId ?? null }),
  deleteStrategy: (id: string) => invoke<void>("delete_strategy", { id }),
  importStrategyYaml: (yaml: string) => invoke<StrategySummary>("import_strategy_yaml", { yaml }),
  exportStrategyYaml: (id: string) => invoke<string>("export_strategy_yaml", { id }),
  saveStrategyExport: (id: string) => invoke<string>("save_strategy_export", { id }),
  exampleStrategyYaml: () => invoke<string>("example_strategy_yaml"),
  getMomentumDemo: () => invoke<MomentumRow[]>("get_momentum_demo"),

  runRebalance: (connectionId: string, accountId: string, strategyId: string) =>
    invoke<RebalanceRun>("run_rebalance", { connectionId, accountId, strategyId }),
  approveAndSubmit: (runId: string, acknowledged: boolean) =>
    invoke<SubmittedOrderDto[]>("approve_and_submit", { runId, acknowledged }),
  ordersForRun: (runId: string) => invoke<SubmittedOrderDto[]>("orders_for_run", { runId }),
  listOrders: (connectionId: string | null, limit: number) =>
    invoke<SubmittedOrderDto[]>("list_orders", { connectionId, limit }),
  cancelOrder: (connectionId: string, brokerOrderId: string) =>
    invoke<void>("cancel_order", { connectionId, brokerOrderId }),
  cancelAllOpen: (connectionId: string) => invoke<number>("cancel_all_open", { connectionId }),
  exportOrdersCsv: (runId: string) => invoke<string>("export_orders_csv", { runId }),

  listActivity: (limit: number, eventType?: string) =>
    invoke<AuditEvent[]>("list_activity", { limit, eventType: eventType ?? null }),
  resetDemo: () => invoke<void>("reset_demo"),
};

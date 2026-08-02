// Browser-only development mock. When the app runs in a plain browser (no
// Tauri runtime), this simulates the Rust backend with the same deterministic
// demo data so the UI can be developed and visually reviewed with `npm run dev`.
// It is never active inside the packaged desktop app.
import { mockIPC } from "@tauri-apps/api/mocks";
import type {
  AppSettings,
  AppState,
  ConnectionDto,
  Dashboard,
  StrategySummary,
  SubmittedOrderDto,
  RebalanceRun,
  AuditEvent,
} from "./api";

const PRICES: Record<string, number> = {
  AAPL: 227.52,
  MSFT: 415.3,
  NVDA: 131.26,
  GOOGL: 178.35,
  AMZN: 197.93,
  VOO: 512.58,
  BND: 73.57,
  META: 585.25,
  TSLA: 248.98,
  QQQ: 483.85,
};
const NAMES: Record<string, string> = {
  AAPL: "Apple Inc.",
  MSFT: "Microsoft Corporation",
  NVDA: "NVIDIA Corporation",
  GOOGL: "Alphabet Inc.",
  AMZN: "Amazon.com, Inc.",
  VOO: "Vanguard S&P 500 ETF",
  BND: "Vanguard Total Bond Market ETF",
};

interface MockDb {
  settings: AppSettings;
  onboarded: boolean;
  connections: ConnectionDto[];
  strategies: StrategySummary[];
  orders: SubmittedOrderDto[];
  runs: RebalanceRun[];
  audit: AuditEvent[];
  positions: { symbol: string; quantity: number; cost: number }[];
  cash: number;
  auditSeq: number;
}

const db: MockDb = {
  settings: {
    safety: {
      max_order_value: 1000,
      max_batch_value: 5000,
      max_position_weight: 0.35,
      min_order_value: 5,
      allow_fractional: true,
      allow_sells: true,
    },
    sync_interval_minutes: 15,
    theme: "system",
    read_only: false,
    emergency_stop: false,
  },
  onboarded: false,
  connections: [],
  strategies: [],
  orders: [],
  runs: [],
  audit: [],
  positions: [
    { symbol: "AAPL", quantity: 42, cost: 8105.16 },
    { symbol: "MSFT", quantity: 18, cost: 6642.0 },
    { symbol: "NVDA", quantity: 60, cost: 5460.3 },
    { symbol: "VOO", quantity: 25, cost: 11310.75 },
    { symbol: "BND", quantity: 80, cost: 5986.4 },
    { symbol: "AMZN", quantity: 15, cost: 2688.3 },
  ],
  cash: 12450,
  auditSeq: 1,
};

function log(type: string, description: string) {
  db.audit.unshift({
    id: db.auditSeq++,
    event_type: type,
    description,
    strategy_id: null,
    connection_id: null,
    account_id: null,
    order_id: null,
    technical: null,
    created_at: new Date().toISOString(),
  });
}

function dashboard(): Dashboard {
  const positions = db.positions.map((p) => {
    const price = PRICES[p.symbol] ?? 100;
    return {
      symbol: p.symbol,
      name: NAMES[p.symbol] ?? null,
      quantity: p.quantity,
      current_price: price,
      market_value: p.quantity * price,
      cost_basis: p.cost,
      unrealized_pl: p.quantity * price - p.cost,
    };
  });
  const value = positions.reduce((s, p) => s + p.market_value, 0) + db.cash;
  const history = Array.from({ length: 30 }, (_, i) => ({
    taken_at: new Date(Date.now() - (30 - i) * 86_400_000).toISOString(),
    portfolio_value: 43_000 * (1 + 0.002 * i + 0.008 * Math.sin(i * 0.9)),
  }));
  const strat = db.strategies[0] ?? null;
  let drift: number | null = null;
  if (strat) {
    drift = 0;
    for (const a of strat.doc.assets) {
      const held = positions.find((p) => p.symbol === a.symbol)?.market_value ?? 0;
      drift += Math.abs(held / value - a.target_weight);
    }
    drift = (drift / 2) * 100;
  }
  return {
    summary: {
      account_id: "DEMO-000001",
      portfolio_value: value,
      cash: db.cash,
      buying_power: db.cash,
      currency: "USD",
      daily_change_percent: 0.42,
    },
    positions,
    recent_orders: db.orders.slice(0, 10),
    drift_percent: drift,
    active_strategy: strat,
    history,
  };
}

function appState(): AppState {
  return {
    settings: db.settings,
    onboarding_completed: db.onboarded,
    onboarding_step: "welcome",
    connections: db.connections,
    version: "0.1.0-browser-preview",
  };
}

export function installDevMock() {
  mockIPC(async (cmd, payload) => {
    const p = (payload ?? {}) as Record<string, unknown>;
    switch (cmd) {
      case "get_app_state":
        return appState();
      case "update_settings":
        db.settings = p.settings as AppSettings;
        return null;
      case "set_onboarding":
        db.onboarded = Boolean(p.completed);
        return null;
      case "set_emergency_stop":
        db.settings = { ...db.settings, emergency_stop: Boolean(p.enabled) };
        log(
          p.enabled ? "emergency_stop_enabled" : "emergency_stop_disabled",
          "Emergency stop toggled.",
        );
        return db.settings;
      case "preview_connection":
        return {
          connected: true,
          paper: true,
          message: "Demo mode is ready. No external account required.",
        };
      case "create_connection": {
        const conn: ConnectionDto = {
          id: "conn-demo",
          broker_kind: String(p.kind),
          label: String(p.label),
          status: "connected",
          paper: true,
          default_account_id: p.kind === "manual" ? "manual" : "DEMO-000001",
          masked_credential: p.key ? `${String(p.key).slice(0, 4)}••••••••` : null,
          last_sync_at: new Date().toISOString(),
          last_error: null,
          accounts: [
            {
              id: p.kind === "manual" ? "manual" : "DEMO-000001",
              name: String(p.label),
              paper: true,
              currency: "USD",
            },
          ],
          supports_trading: p.kind !== "manual" && p.kind !== "ibkr",
          created_at: new Date().toISOString(),
        };
        db.connections = [conn];
        log("connection_created", `Connected ${conn.label}.`);
        return conn;
      }
      case "list_connections":
        return db.connections;
      case "test_connection":
        return { connected: true, paper: true, message: "Connection is healthy." };
      case "sync_now":
        db.connections = db.connections.map((c) => ({
          ...c,
          last_sync_at: new Date().toISOString(),
        }));
        log("sync_completed", "Account data was synchronized.");
        return null;
      case "recent_syncs":
        return [];
      case "disconnect":
        db.connections = [];
        log("connection_removed", "Disconnected and removed stored credentials.");
        return null;
      case "get_dashboard":
        return dashboard();
      case "list_strategies":
        return db.strategies;
      case "save_strategy": {
        const doc = p.doc as StrategySummary["doc"];
        const existing = p.existingId as string | null;
        const s: StrategySummary = {
          id: existing ?? `strat-${db.strategies.length + 1}`,
          name: doc.name,
          description: doc.description ?? null,
          kind: String(p.kind),
          current_version: existing ? 2 : 1,
          asset_count: doc.assets.length,
          updated_at: new Date().toISOString(),
          doc,
        };
        db.strategies = [s, ...db.strategies.filter((x) => x.id !== s.id)];
        log(existing ? "strategy_updated" : "strategy_created", `Strategy "${doc.name}" saved.`);
        return s;
      }
      case "delete_strategy":
        db.strategies = db.strategies.filter((s) => s.id !== p.id);
        return null;
      case "import_strategy_yaml":
        throw {
          category: "Validation",
          detail: { user_message: "YAML import requires the desktop app.", technical: null },
        };
      case "export_strategy_yaml":
      case "example_strategy_yaml":
        return "schema_version: 1\nname: Example\nassets:\n  - symbol: VOO\n    target_weight: 1.0\n";
      case "save_strategy_export":
      case "export_positions_csv":
      case "export_orders_csv":
        return "~/Downloads/portfolio-os-export (desktop app only)";
      case "get_momentum_demo":
        return [
          { symbol: "NVDA", sample_return_percent: 38.2, selected: true },
          { symbol: "META", sample_return_percent: 24.7, selected: true },
          { symbol: "AAPL", sample_return_percent: 12.1, selected: true },
          { symbol: "MSFT", sample_return_percent: 9.8, selected: true },
          { symbol: "GOOGL", sample_return_percent: 7.4, selected: true },
          { symbol: "AMZN", sample_return_percent: 6.9, selected: false },
          { symbol: "V", sample_return_percent: 4.2, selected: false },
          { symbol: "JPM", sample_return_percent: 3.1, selected: false },
          { symbol: "JNJ", sample_return_percent: -2.4, selected: false },
          { symbol: "XOM", sample_return_percent: -5.8, selected: false },
        ];
      case "run_rebalance": {
        const strat = db.strategies.find((s) => s.id === p.strategyId) ?? db.strategies[0];
        const dash = dashboard();
        const value = dash.summary!.portfolio_value;
        const orders = (strat?.doc.assets ?? [])
          .map((a) => {
            const held = dash.positions.find((x) => x.symbol === a.symbol)?.market_value ?? 0;
            const current = held / value;
            const delta = a.target_weight * value - held;
            const price = PRICES[a.symbol] ?? 100;
            const side = delta > 0 ? ("buy" as const) : ("sell" as const);
            const capped = Math.min(Math.abs(delta), db.settings.safety.max_order_value);
            return {
              symbol: a.symbol,
              side,
              quantity: Math.round((capped / price) * 10000) / 10000,
              estimated_value: capped,
              current_weight: current,
              target_weight: a.target_weight,
              drift: current - a.target_weight,
              reason:
                side === "buy"
                  ? `Buy approximately $${capped.toFixed(2)} of ${a.symbol} because its current allocation is ${(current * 100).toFixed(1)}%, below the ${(a.target_weight * 100).toFixed(1)}% target by ${((a.target_weight - current) * 100).toFixed(1)} percentage points.`
                  : `Sell approximately $${capped.toFixed(2)} of ${a.symbol} because its current allocation is ${(current * 100).toFixed(1)}%, above the ${(a.target_weight * 100).toFixed(1)}% target by ${((current - a.target_weight) * 100).toFixed(1)} percentage points.`,
              warnings: [],
            };
          })
          .filter(
            (o) =>
              o.estimated_value > db.settings.safety.min_order_value && Math.abs(o.drift) > 0.02,
          );
        const run: RebalanceRun = {
          id: `run-${db.runs.length + 1}`,
          connection_id: String(p.connectionId),
          account_id: String(p.accountId),
          strategy_id: strat?.id ?? null,
          status: "proposed",
          plan: {
            orders,
            skipped: [{ symbol: "BND", reason: "Within the 2.0% drift threshold." }],
            warnings: [],
            total_buy_value: orders
              .filter((o) => o.side === "buy")
              .reduce((s, o) => s + o.estimated_value, 0),
            total_sell_value: orders
              .filter((o) => o.side === "sell")
              .reduce((s, o) => s + o.estimated_value, 0),
            estimated_cash_after: db.cash,
            portfolio_value: value,
          },
          created_at: new Date().toISOString(),
        };
        db.runs.push(run);
        log("rebalance_calculated", `Rebalance proposed ${orders.length} orders.`);
        return run;
      }
      case "approve_and_submit": {
        const run = db.runs.find((r) => r.id === p.runId);
        if (!run)
          throw {
            category: "Validation",
            detail: { user_message: "Run not found.", technical: null },
          };
        if (db.settings.emergency_stop)
          throw {
            category: "Validation",
            detail: {
              user_message:
                "Emergency stop is active. Disable it in Settings before submitting orders.",
              technical: null,
            },
          };
        const now = new Date().toISOString();
        const orders: SubmittedOrderDto[] = run.plan.orders.map((o, i) => ({
          client_order_id: `pos-${run.id}-${o.side}-${o.symbol}`,
          run_id: run.id,
          broker_order_id: `demo-${run.id}-${i}`,
          symbol: o.symbol,
          side: o.side,
          quantity: o.quantity,
          filled_quantity: 0,
          avg_fill_price: null,
          status: "accepted",
          message: null,
          submitted_at: now,
          updated_at: now,
        }));
        db.orders = [...orders, ...db.orders];
        run.status = "submitted";
        setTimeout(() => {
          for (const o of db.orders) {
            if (o.run_id === run.id) {
              o.status = "filled";
              o.filled_quantity = o.quantity;
              o.avg_fill_price = PRICES[o.symbol] ?? 100;
            }
          }
        }, 3000);
        log("orders_approved", `User approved a batch of ${orders.length} orders.`);
        return orders;
      }
      case "orders_for_run":
        return db.orders.filter((o) => o.run_id === p.runId);
      case "list_orders":
        return db.orders;
      case "cancel_order":
      case "cancel_all_open":
        return 0;
      case "list_activity":
        return db.audit;
      case "reset_demo":
        log("demo_reset", "Demo data was reset to its initial state.");
        return null;
      case "upsert_manual_position":
      case "set_manual_cash":
      case "import_positions_csv":
      case "set_default_account":
      case "replace_credentials":
        return null;
      default:
        console.warn("devMock: unhandled command", cmd);
        return null;
    }
  });
  console.info(
    "PortfolioOS: running with the browser development mock (no Tauri runtime detected).",
  );
}

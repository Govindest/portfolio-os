import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { HashRouter } from "react-router";
import { mockIPC } from "@tauri-apps/api/mocks";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AppProvider } from "@/context/AppContext";
import App from "@/App";
import type { AppState } from "@/lib/api";

const emptyState: AppState = {
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
    theme: "light",
    read_only: false,
    emergency_stop: false,
  },
  onboarding_completed: false,
  onboarding_step: "welcome",
  connections: [],
  version: "0.1.0",
};

function renderApp() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <HashRouter>
        <AppProvider>
          <TooltipProvider>
            <App />
          </TooltipProvider>
        </AppProvider>
      </HashRouter>
    </QueryClientProvider>,
  );
}

describe("first-launch onboarding", () => {
  it("shows the welcome screen with privacy and safety messaging", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_app_state") return emptyState;
      return null;
    });
    renderApp();
    expect(await screen.findByText(/stays on your computer/i)).toBeInTheDocument();
    expect(screen.getByText(/Local-first privacy/i)).toBeInTheDocument();
    expect(screen.getByText(/You approve every trade/i)).toBeInTheDocument();
    expect(screen.getByText(/Paper trading first/i)).toBeInTheDocument();
    // Legal language is visible.
    expect(screen.getByText(/not a broker-dealer/i)).toBeInTheDocument();
  });

  it("walks welcome → safety limits → broker choice", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_app_state") return emptyState;
      if (cmd === "update_settings") return null;
      return null;
    });
    renderApp();
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /get started/i }));
    expect(await screen.findByText(/safety limits/i)).toBeInTheDocument();
    // Batch approval toggle is on and locked.
    const approvalSwitch = screen.getByRole("switch", { name: /require approval/i });
    expect(approvalSwitch).toBeDisabled();
    expect(approvalSwitch).toBeChecked();
    await user.click(screen.getByRole("button", { name: /continue/i }));
    // Broker cards render with honest labels.
    expect(await screen.findByText("Alpaca Paper")).toBeInTheDocument();
    expect(screen.getByText(/Recommended for first-time setup/i)).toBeInTheDocument();
    expect(screen.getByText("Demo Mode")).toBeInTheDocument();
    expect(screen.getByText(/Experimental · read-only/i)).toBeInTheDocument();
  });
});

describe("connected app", () => {
  it("boots straight to the dashboard when onboarding is complete", async () => {
    mockIPC((cmd) => {
      switch (cmd) {
        case "get_app_state":
          return {
            ...emptyState,
            onboarding_completed: true,
            connections: [
              {
                id: "c1",
                broker_kind: "mock",
                label: "Demo Portfolio",
                status: "connected",
                paper: true,
                default_account_id: "DEMO-000001",
                masked_credential: null,
                last_sync_at: new Date().toISOString(),
                last_error: null,
                accounts: [
                  { id: "DEMO-000001", name: "Demo Portfolio", paper: true, currency: "USD" },
                ],
                supports_trading: true,
                created_at: new Date().toISOString(),
              },
            ],
          };
        case "get_dashboard":
          return {
            summary: {
              account_id: "DEMO-000001",
              portfolio_value: 43210.99,
              cash: 12450,
              buying_power: 12450,
              currency: "USD",
              daily_change_percent: 0.42,
            },
            positions: [
              {
                symbol: "AAPL",
                name: "Apple Inc.",
                quantity: 42,
                current_price: 227.52,
                market_value: 9555.84,
                cost_basis: 8105.16,
                unrealized_pl: 1450.68,
              },
            ],
            recent_orders: [],
            drift_percent: 4.2,
            active_strategy: null,
            history: [],
          };
        case "list_strategies":
          return [];
        default:
          return null;
      }
    });
    renderApp();
    expect(await screen.findByRole("heading", { name: "Overview" })).toBeInTheDocument();
    expect(await screen.findByText("$43,210.99")).toBeInTheDocument();
    // Sidebar shows paper badge and connection state.
    expect(screen.getAllByText(/paper/i).length).toBeGreaterThan(0);
    expect(screen.getByText("Connected")).toBeInTheDocument();
  });

  it("shows an empty state when nothing is connected", async () => {
    mockIPC((cmd) => {
      if (cmd === "get_app_state") return { ...emptyState, onboarding_completed: true };
      if (cmd === "list_strategies") return [];
      return null;
    });
    renderApp();
    const matches = await screen.findAllByText(/No brokerage connected/i);
    expect(matches.length).toBeGreaterThan(0);
  });
});

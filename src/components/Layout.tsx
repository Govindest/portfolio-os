import { NavLink, Outlet } from "react-router";
import {
  Activity,
  Landmark,
  LayoutDashboard,
  PieChart,
  RefreshCw,
  Scale,
  Settings,
  Target,
} from "lucide-react";
import { useApp } from "@/context/AppContext";
import { Badge } from "@/components/ui/badge";
import { Select } from "@/components/ui/select";
import { cn, timeAgo } from "@/lib/utils";

const NAV = [
  { to: "/", label: "Overview", icon: LayoutDashboard, end: true },
  { to: "/portfolio", label: "Portfolio", icon: PieChart },
  { to: "/strategies", label: "Strategies", icon: Target },
  { to: "/rebalance", label: "Rebalance", icon: Scale },
  { to: "/activity", label: "Activity", icon: Activity },
  { to: "/brokerages", label: "Brokerages", icon: Landmark },
  { to: "/settings", label: "Settings", icon: Settings },
];

const BROKER_NAMES: Record<string, string> = {
  mock: "Demo Mode",
  alpaca: "Alpaca Paper",
  tradier: "Tradier Sandbox",
  ibkr: "Interactive Brokers",
  manual: "Manual Portfolio",
};

export function Layout() {
  const { state, activeConnection, setActiveConnectionId } = useApp();
  const connections = state?.connections ?? [];

  return (
    <div className="flex h-full">
      <aside className="flex w-60 shrink-0 flex-col border-r border-border bg-card">
        <div className="flex items-center gap-2 px-5 pb-2 pt-5">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <PieChart className="h-4 w-4" aria-hidden />
          </div>
          <span className="text-base font-semibold tracking-tight">PortfolioOS</span>
        </div>
        {state?.settings.emergency_stop && (
          <div className="mx-4 mt-2 rounded-md bg-destructive/10 px-3 py-1.5 text-xs font-medium text-destructive">
            Emergency stop active
          </div>
        )}
        <nav className="mt-4 flex-1 space-y-0.5 px-3" aria-label="Main navigation">
          {NAV.map(({ to, label, icon: Icon, end }) => (
            <NavLink
              key={to}
              to={to}
              end={end}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                  isActive
                    ? "bg-accent text-accent-foreground"
                    : "text-muted-foreground hover:bg-muted hover:text-foreground",
                )
              }
            >
              <Icon className="h-4 w-4" aria-hidden />
              {label}
            </NavLink>
          ))}
        </nav>
        <div className="space-y-2 border-t border-border p-4">
          {connections.length > 1 && (
            <Select
              aria-label="Active brokerage"
              value={activeConnection?.id ?? ""}
              onChange={(e) => setActiveConnectionId(e.target.value)}
              className="h-8 text-xs"
            >
              {connections.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.label}
                </option>
              ))}
            </Select>
          )}
          {activeConnection ? (
            <div className="space-y-1.5 text-xs text-muted-foreground">
              <div className="flex items-center justify-between">
                <span className="font-medium text-foreground">
                  {BROKER_NAMES[activeConnection.broker_kind] ?? activeConnection.broker_kind}
                </span>
                <span
                  className={cn(
                    "inline-flex items-center gap-1",
                    activeConnection.status === "connected" ? "text-success" : "text-destructive",
                  )}
                >
                  <span
                    className={cn(
                      "h-1.5 w-1.5 rounded-full",
                      activeConnection.status === "connected" ? "bg-success" : "bg-destructive",
                    )}
                    aria-hidden
                  />
                  {activeConnection.status === "connected" ? "Connected" : "Error"}
                </span>
              </div>
              <div className="flex items-center justify-between">
                {activeConnection.paper && <Badge variant="accent">Paper</Badge>}
                <span className="inline-flex items-center gap-1">
                  <RefreshCw className="h-3 w-3" aria-hidden />
                  {timeAgo(activeConnection.last_sync_at)}
                </span>
              </div>
            </div>
          ) : (
            <p className="text-xs text-muted-foreground">No brokerage connected</p>
          )}
          <p className="text-[10px] text-muted-foreground/70">Version {state?.version ?? "…"}</p>
        </div>
      </aside>
      <main className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto max-w-6xl px-8 py-8">
          <Outlet />
        </div>
      </main>
    </div>
  );
}

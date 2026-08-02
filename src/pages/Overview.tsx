import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router";
import { RefreshCw, TrendingDown, TrendingUp } from "lucide-react";
import {
  Area,
  AreaChart,
  Cell,
  Pie,
  PieChart as RPieChart,
  ResponsiveContainer,
  Tooltip as RTooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api } from "@/lib/api";
import { useApp } from "@/context/AppContext";
import { Button, buttonVariants } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { TermTip } from "@/components/ui/tooltip";
import { EmptyState, StatusBadge } from "@/components/shared";
import { formatMoney, formatPercent, formatDateTime, timeAgo, formatQty } from "@/lib/utils";

const CHART_COLORS = [
  "#6366f1",
  "#818cf8",
  "#a5b4fc",
  "#c7d2fe",
  "#4f46e5",
  "#4338ca",
  "#3730a3",
  "#312e81",
];

export default function Overview() {
  const { activeConnection, activeAccountId, refresh } = useApp();
  const qc = useQueryClient();

  const { data: dash, isLoading } = useQuery({
    queryKey: ["dashboard", activeConnection?.id, activeAccountId],
    queryFn: () => api.getDashboard(activeConnection!.id, activeAccountId!),
    enabled: Boolean(activeConnection && activeAccountId),
    refetchInterval: 15_000,
  });

  const sync = useMutation({
    mutationFn: () => api.syncNow(activeConnection!.id),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["dashboard"] });
      refresh();
    },
  });

  if (!activeConnection) {
    return (
      <div className="space-y-5">
        <h1 className="text-2xl font-semibold tracking-tight">Overview</h1>
        <EmptyState
          title="No brokerage connected"
          description="Connect a brokerage or explore demo mode to see your portfolio here."
          action={
            <Link to="/brokerages" className={buttonVariants()}>
              Connect a brokerage
            </Link>
          }
        />
      </div>
    );
  }

  if (isLoading || !dash) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-8 w-52" />
        <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
          {[...Array(4)].map((_, i) => (
            <Skeleton key={i} className="h-24" />
          ))}
        </div>
        <Skeleton className="h-64" />
      </div>
    );
  }

  const summary = dash.summary;
  const change = summary?.daily_change_percent ?? null;
  const allocation = dash.positions.map((p) => ({ name: p.symbol, value: p.market_value }));
  if (summary && summary.cash > 0) allocation.push({ name: "Cash", value: summary.cash });
  const history = dash.history.map((h) => ({
    date: new Date(h.taken_at).toLocaleDateString(undefined, { month: "short", day: "numeric" }),
    value: h.portfolio_value,
  }));

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Overview</h1>
          <p className="text-sm text-muted-foreground">
            {activeConnection.label} · Last synced {timeAgo(activeConnection.last_sync_at)}
          </p>
        </div>
        <Button variant="outline" onClick={() => sync.mutate()} disabled={sync.isPending}>
          <RefreshCw className={sync.isPending ? "animate-spin" : ""} aria-hidden />
          Sync now
        </Button>
      </div>

      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        <StatCard
          label="Portfolio value"
          value={formatMoney(summary?.portfolio_value)}
          sub={
            change !== null ? (
              <span className={change >= 0 ? "text-success" : "text-destructive"}>
                {change >= 0 ? (
                  <TrendingUp className="mr-1 inline h-3.5 w-3.5" aria-hidden />
                ) : (
                  <TrendingDown className="mr-1 inline h-3.5 w-3.5" aria-hidden />
                )}
                {formatPercent(change, 2)} today
              </span>
            ) : undefined
          }
        />
        <StatCard label="Cash" value={formatMoney(summary?.cash)} />
        <StatCard
          label="Buying power"
          value={formatMoney(summary?.buying_power)}
          term={
            <TermTip term="Buying power">
              The amount available to spend on new purchases, including cash and any margin your
              brokerage extends.
            </TermTip>
          }
        />
        <StatCard
          label="Portfolio drift"
          value={dash.drift_percent !== null ? formatPercent(dash.drift_percent) : "—"}
          sub={
            dash.active_strategy ? (
              <>vs. {dash.active_strategy.name}</>
            ) : (
              <Link to="/strategies" className="text-primary hover:underline">
                Create a strategy
              </Link>
            )
          }
          term={
            <TermTip term="Drift">
              How far your current allocation has moved away from the strategy's target weights.
              Higher drift means a rebalance would trade more.
            </TermTip>
          }
        />
      </div>

      <div className="grid gap-4 lg:grid-cols-5">
        <Card className="lg:col-span-3">
          <CardHeader>
            <CardTitle>Portfolio value</CardTitle>
            <CardDescription>Based on synchronized account snapshots</CardDescription>
          </CardHeader>
          <CardContent>
            {history.length > 1 ? (
              <ResponsiveContainer width="100%" height={220}>
                <AreaChart data={history} margin={{ left: 8, right: 8, top: 4 }}>
                  <defs>
                    <linearGradient id="pv" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="#6366f1" stopOpacity={0.3} />
                      <stop offset="100%" stopColor="#6366f1" stopOpacity={0} />
                    </linearGradient>
                  </defs>
                  <XAxis
                    dataKey="date"
                    tickLine={false}
                    axisLine={false}
                    fontSize={11}
                    minTickGap={40}
                  />
                  <YAxis
                    tickLine={false}
                    axisLine={false}
                    fontSize={11}
                    width={70}
                    domain={["auto", "auto"]}
                    tickFormatter={(v: number) => formatMoney(v).replace(".00", "")}
                  />
                  <RTooltip formatter={(v) => formatMoney(Number(v))} />
                  <Area
                    type="monotone"
                    dataKey="value"
                    stroke="#6366f1"
                    strokeWidth={2}
                    fill="url(#pv)"
                  />
                </AreaChart>
              </ResponsiveContainer>
            ) : (
              <p className="py-12 text-center text-sm text-muted-foreground">
                Value history builds up as PortfolioOS synchronizes over time.
              </p>
            )}
          </CardContent>
        </Card>

        <Card className="lg:col-span-2">
          <CardHeader>
            <CardTitle>Allocation</CardTitle>
            <CardDescription>{dash.positions.length} positions</CardDescription>
          </CardHeader>
          <CardContent>
            {allocation.length > 0 ? (
              <div className="flex items-center gap-4">
                <ResponsiveContainer width={140} height={140}>
                  <RPieChart>
                    <Pie
                      data={allocation}
                      dataKey="value"
                      nameKey="name"
                      innerRadius={40}
                      outerRadius={65}
                      strokeWidth={1}
                    >
                      {allocation.map((_, i) => (
                        <Cell key={i} fill={CHART_COLORS[i % CHART_COLORS.length]} />
                      ))}
                    </Pie>
                    <RTooltip formatter={(v) => formatMoney(Number(v))} />
                  </RPieChart>
                </ResponsiveContainer>
                <ul className="flex-1 space-y-1 text-xs">
                  {allocation.slice(0, 6).map((a, i) => (
                    <li key={a.name} className="flex items-center gap-2">
                      <span
                        className="h-2 w-2 rounded-full"
                        style={{ background: CHART_COLORS[i % CHART_COLORS.length] }}
                        aria-hidden
                      />
                      <span className="flex-1 truncate">{a.name}</span>
                      <span className="tabular-nums text-muted-foreground">
                        {formatMoney(a.value)}
                      </span>
                    </li>
                  ))}
                  {allocation.length > 6 && (
                    <li className="text-muted-foreground">+{allocation.length - 6} more</li>
                  )}
                </ul>
              </div>
            ) : (
              <p className="py-8 text-center text-sm text-muted-foreground">No positions yet.</p>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Recent orders</CardTitle>
        </CardHeader>
        <CardContent>
          {dash.recent_orders.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              No orders yet. Run a rebalance to see proposed trades.
            </p>
          ) : (
            <ul className="divide-y divide-border">
              {dash.recent_orders.map((o) => (
                <li key={o.client_order_id} className="flex items-center gap-3 py-2.5 text-sm">
                  <span className="w-14 font-medium uppercase">{o.side}</span>
                  <span className="w-16 font-semibold">{o.symbol}</span>
                  <span className="tabular-nums text-muted-foreground">
                    {formatQty(o.quantity)} shares
                  </span>
                  <span className="ml-auto text-xs text-muted-foreground">
                    {formatDateTime(o.submitted_at)}
                  </span>
                  <StatusBadge status={o.status} />
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function StatCard({
  label,
  value,
  sub,
  term,
}: {
  label: string;
  value: string;
  sub?: React.ReactNode;
  term?: React.ReactNode;
}) {
  return (
    <Card>
      <CardContent className="p-5">
        <p className="text-xs text-muted-foreground">{term ?? label}</p>
        <p className="mt-1 text-2xl font-semibold tabular-nums tracking-tight">{value}</p>
        {sub && <p className="mt-1 text-xs text-muted-foreground">{sub}</p>}
      </CardContent>
    </Card>
  );
}

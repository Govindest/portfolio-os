import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router";
import { AlertTriangle, Download, Info, Scale } from "lucide-react";
import { api, type RebalanceRun, type SubmittedOrderDto } from "@/lib/api";
import { useApp } from "@/context/AppContext";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Select } from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { EmptyState, ErrorNote, StatusBadge } from "@/components/shared";
import { cn, formatMoney, formatQty, weightPercent } from "@/lib/utils";

export default function Rebalance() {
  const { state, activeConnection, activeAccountId } = useApp();
  const qc = useQueryClient();
  const { data: strategies } = useQuery({ queryKey: ["strategies"], queryFn: api.listStrategies });
  const [strategyId, setStrategyId] = React.useState<string>("");
  const [run, setRun] = React.useState<RebalanceRun | null>(null);
  const [submitted, setSubmitted] = React.useState<SubmittedOrderDto[] | null>(null);
  const [acknowledged, setAcknowledged] = React.useState(false);
  const [confirmText, setConfirmText] = React.useState("");

  const effectiveStrategyId = strategyId || strategies?.[0]?.id || "";
  const isManual = activeConnection?.broker_kind === "manual";
  const canTrade = activeConnection?.supports_trading && !isManual;
  const emergencyStop = state?.settings.emergency_stop ?? false;

  const compute = useMutation({
    mutationFn: () => api.runRebalance(activeConnection!.id, activeAccountId!, effectiveStrategyId),
    onSuccess: (r) => {
      setRun(r);
      setSubmitted(null);
      setAcknowledged(false);
      setConfirmText("");
    },
  });

  const submit = useMutation({
    mutationFn: () => api.approveAndSubmit(run!.id, true),
    onSuccess: (orders) => {
      setSubmitted(orders);
      void qc.invalidateQueries({ queryKey: ["dashboard"] });
    },
  });

  const exportCsv = useMutation({ mutationFn: () => api.exportOrdersCsv(run!.id) });

  // Poll fills after submission.
  const { data: liveOrders } = useQuery({
    queryKey: ["run-orders", run?.id],
    queryFn: () => api.ordersForRun(run!.id),
    enabled: Boolean(run && submitted),
    refetchInterval: (q) => {
      const orders = q.state.data;
      const settled = orders?.every((o) => ["filled", "rejected", "canceled"].includes(o.status));
      return settled ? false : 2_000;
    },
  });
  const displayOrders = liveOrders ?? submitted;

  React.useEffect(() => {
    if (displayOrders && activeConnection) {
      void api.syncNow(activeConnection.id).catch(() => undefined);
    }
    // Trigger one sync when orders appear so fills reconcile quickly.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [displayOrders?.every((o) => o.status === "filled")]);

  // Empty states keep the page heading so the document outline and screen-reader
  // navigation stay consistent across every route.
  if (!activeConnection || !activeAccountId) {
    return (
      <PageShell>
        <EmptyState
          title="No brokerage connected"
          description="Connect a brokerage before rebalancing."
        />
      </PageShell>
    );
  }
  if (!strategies || strategies.length === 0) {
    return (
      <PageShell>
        <EmptyState
          icon={Scale}
          title="No strategy to rebalance against"
          description="Create a strategy first — the rebalancer compares your holdings to its target weights."
          action={
            <Link to="/strategies" className={buttonVariants()}>
              Create a strategy
            </Link>
          }
        />
      </PageShell>
    );
  }

  const warningsToAck = run
    ? [...run.plan.warnings, ...run.plan.orders.flatMap((o) => o.warnings)]
    : [];
  const needsAck = warningsToAck.length > 0;
  const confirmed = confirmText.trim().toUpperCase() === "SUBMIT";

  return (
    <div className="space-y-5">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Rebalance</h1>
        <p className="text-sm text-muted-foreground">
          Preview the trades needed to move {activeConnection.label} toward a strategy. Nothing is
          submitted without your approval.
        </p>
      </div>

      {emergencyStop && (
        <div
          className="flex items-center gap-2 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm"
          role="alert"
        >
          <AlertTriangle className="h-4 w-4 text-destructive" aria-hidden />
          Emergency stop is active. You can preview rebalances, but submission is blocked until you
          disable it in Settings.
        </div>
      )}

      <div className="flex items-end gap-3">
        <div className="w-72 space-y-1.5">
          <label htmlFor="rb-strategy" className="text-sm font-medium">
            Strategy
          </label>
          <Select
            id="rb-strategy"
            value={effectiveStrategyId}
            onChange={(e) => setStrategyId(e.target.value)}
          >
            {strategies.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </Select>
        </div>
        <Button
          onClick={() => compute.mutate()}
          disabled={compute.isPending || !effectiveStrategyId}
        >
          <Scale aria-hidden /> {compute.isPending ? "Calculating…" : "Preview rebalance"}
        </Button>
      </div>

      {compute.isError && <ErrorNote error={compute.error} />}

      {run && !submitted && (
        <>
          <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
            <SummaryStat label="Estimated buys" value={formatMoney(run.plan.total_buy_value)} />
            <SummaryStat label="Estimated sells" value={formatMoney(run.plan.total_sell_value)} />
            <SummaryStat
              label="Cash after execution"
              value={formatMoney(run.plan.estimated_cash_after)}
            />
            <SummaryStat label="Orders" value={String(run.plan.orders.length)} />
          </div>

          {run.plan.warnings.length > 0 && (
            <div className="space-y-1">
              {run.plan.warnings.map((w, i) => (
                <div
                  key={i}
                  className="flex items-center gap-2 rounded-md border border-warning/40 bg-warning/10 px-3 py-2 text-sm"
                >
                  <AlertTriangle className="h-4 w-4 shrink-0 text-warning" aria-hidden />
                  {w}
                </div>
              ))}
            </div>
          )}

          {run.plan.orders.length === 0 ? (
            <EmptyState
              title="Nothing to do"
              description="Your portfolio is within the strategy's drift threshold. No trades are needed."
            />
          ) : (
            <Card>
              <CardHeader>
                <CardTitle>Proposed orders</CardTitle>
                <CardDescription>
                  Each order explains exactly why it is proposed. Review every row before approving.
                </CardDescription>
              </CardHeader>
              <CardContent className="p-0">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Order</TableHead>
                      <TableHead className="text-right">Quantity</TableHead>
                      <TableHead className="text-right">Est. value</TableHead>
                      <TableHead className="text-right">Current → Target</TableHead>
                      <TableHead>Why</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {run.plan.orders.map((o) => (
                      <TableRow key={`${o.side}-${o.symbol}`}>
                        <TableCell>
                          <Badge variant={o.side === "buy" ? "success" : "destructive"}>
                            {o.side.toUpperCase()}
                          </Badge>{" "}
                          <span className="font-semibold">{o.symbol}</span>
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {formatQty(o.quantity)}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">
                          {formatMoney(o.estimated_value)}
                        </TableCell>
                        <TableCell className="text-right tabular-nums text-muted-foreground">
                          {weightPercent(o.current_weight)} → {weightPercent(o.target_weight)}
                        </TableCell>
                        <TableCell className="max-w-md text-xs text-muted-foreground">
                          {o.reason}
                          {o.warnings.map((w, i) => (
                            <span key={i} className="mt-0.5 flex items-center gap-1 text-warning">
                              <AlertTriangle className="h-3 w-3 shrink-0" aria-hidden /> {w}
                            </span>
                          ))}
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          )}

          {run.plan.skipped.length > 0 && (
            <details className="rounded-md border border-border bg-card px-4 py-3 text-sm">
              <summary className="cursor-pointer font-medium">
                {run.plan.skipped.length} assets skipped — see why
              </summary>
              <ul className="mt-2 space-y-1 text-muted-foreground">
                {run.plan.skipped.map((s, i) => (
                  <li key={i} className="flex gap-2">
                    <Info className="mt-0.5 h-3.5 w-3.5 shrink-0" aria-hidden />
                    <span>
                      <span className="font-medium text-foreground">{s.symbol}</span>: {s.reason}
                    </span>
                  </li>
                ))}
              </ul>
            </details>
          )}

          {run.plan.orders.length > 0 && (
            <Card>
              <CardHeader>
                <CardTitle>Approve and submit</CardTitle>
                <CardDescription>
                  {canTrade
                    ? "Orders are submitted to your paper-trading account as market orders."
                    : "This connection cannot submit orders. Export the trade list and place it with your broker."}
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                {needsAck && canTrade && (
                  <label className="flex items-start gap-2 text-sm">
                    <input
                      type="checkbox"
                      checked={acknowledged}
                      onChange={(e) => setAcknowledged(e.target.checked)}
                      className="mt-0.5 h-4 w-4 accent-[var(--primary)]"
                    />
                    I have read the {warningsToAck.length} warning
                    {warningsToAck.length === 1 ? "" : "s"} above and want to continue.
                  </label>
                )}
                {canTrade && (
                  <div className="space-y-1.5">
                    <label htmlFor="rb-confirm" className="text-sm font-medium">
                      Type <span className="font-mono font-semibold">SUBMIT</span> to confirm
                    </label>
                    <input
                      id="rb-confirm"
                      value={confirmText}
                      onChange={(e) => setConfirmText(e.target.value)}
                      className="flex h-9 w-48 rounded-md border border-input bg-transparent px-3 py-1 font-mono text-sm uppercase shadow-sm focus-visible:outline-2 focus-visible:outline-ring"
                      autoComplete="off"
                    />
                  </div>
                )}
                {submit.isError && <ErrorNote error={submit.error} />}
                <div className="flex gap-2">
                  {canTrade && (
                    <Button
                      onClick={() => submit.mutate()}
                      disabled={
                        submit.isPending ||
                        !confirmed ||
                        (needsAck && !acknowledged) ||
                        emergencyStop
                      }
                    >
                      {submit.isPending ? "Submitting…" : `Submit ${run.plan.orders.length} orders`}
                    </Button>
                  )}
                  <Button
                    variant="outline"
                    onClick={() => exportCsv.mutate()}
                    disabled={exportCsv.isPending}
                  >
                    <Download aria-hidden /> Export trade list (CSV)
                  </Button>
                </div>
                {exportCsv.isSuccess && (
                  <p className="text-sm text-success" role="status">
                    Exported to {exportCsv.data}
                  </p>
                )}
              </CardContent>
            </Card>
          )}
        </>
      )}

      {displayOrders && (
        <Card>
          <CardHeader>
            <CardTitle>Order status</CardTitle>
            <CardDescription>Live status updates as your brokerage reports fills.</CardDescription>
          </CardHeader>
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Order</TableHead>
                  <TableHead className="text-right">Quantity</TableHead>
                  <TableHead className="text-right">Filled</TableHead>
                  <TableHead className="text-right">Avg. fill price</TableHead>
                  <TableHead className="text-right">Status</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {displayOrders.map((o) => (
                  <TableRow key={o.client_order_id}>
                    <TableCell>
                      <span
                        className={cn(
                          "font-medium uppercase",
                          o.side === "buy" ? "text-success" : "text-destructive",
                        )}
                      >
                        {o.side}
                      </span>{" "}
                      <span className="font-semibold">{o.symbol}</span>
                      {o.message && <p className="text-xs text-muted-foreground">{o.message}</p>}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">
                      {formatQty(o.quantity)}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">
                      {formatQty(o.filled_quantity)}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">
                      {o.avg_fill_price !== null ? formatMoney(o.avg_fill_price) : "—"}
                    </TableCell>
                    <TableCell className="text-right">
                      <StatusBadge status={o.status} />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function PageShell({ children }: { children: React.ReactNode }) {
  return (
    <div className="space-y-5">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Rebalance</h1>
        <p className="text-sm text-muted-foreground">
          Preview the trades a strategy implies. Nothing is submitted without your approval.
        </p>
      </div>
      {children}
    </div>
  );
}

function SummaryStat({ label, value }: { label: string; value: string }) {
  return (
    <Card>
      <CardContent className="p-4">
        <p className="text-xs text-muted-foreground">{label}</p>
        <p className="mt-1 text-xl font-semibold tabular-nums">{value}</p>
      </CardContent>
    </Card>
  );
}

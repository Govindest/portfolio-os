import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, Download, Plus, RefreshCw, Search, Upload } from "lucide-react";
import { api } from "@/lib/api";
import { useApp } from "@/context/AppContext";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState, ErrorNote } from "@/components/shared";
import { cn, formatMoney, formatQty, timeAgo, weightPercent } from "@/lib/utils";

type SortKey = "symbol" | "market_value" | "unrealized_pl" | "weight";

export default function Portfolio() {
  const { activeConnection, activeAccountId, refresh } = useApp();
  const qc = useQueryClient();
  const [search, setSearch] = React.useState("");
  const [sortKey, setSortKey] = React.useState<SortKey>("market_value");
  const [sortDesc, setSortDesc] = React.useState(true);
  const [manualOpen, setManualOpen] = React.useState(false);

  const isManual = activeConnection?.broker_kind === "manual";

  const { data: dash, isLoading } = useQuery({
    queryKey: ["dashboard", activeConnection?.id, activeAccountId],
    queryFn: () => api.getDashboard(activeConnection!.id, activeAccountId!),
    enabled: Boolean(activeConnection && activeAccountId),
  });

  const sync = useMutation({
    mutationFn: () => api.syncNow(activeConnection!.id),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["dashboard"] });
      refresh();
    },
  });

  const exportCsv = useMutation({
    mutationFn: () => api.exportPositionsCsv(activeConnection!.id, activeAccountId!),
  });

  const importCsv = useMutation({
    mutationFn: (content: string) => api.importPositionsCsv(activeConnection!.id, content),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ["dashboard"] }),
  });

  if (!activeConnection || !activeAccountId) {
    return (
      <div className="space-y-5">
        <h1 className="text-2xl font-semibold tracking-tight">Portfolio</h1>
        <EmptyState
          title="No brokerage connected"
          description="Connect a brokerage to see positions."
        />
      </div>
    );
  }

  const positions = dash?.positions ?? [];
  const totalValue = positions.reduce((s, p) => s + p.market_value, 0) + (dash?.summary?.cash ?? 0);
  const targets = new Map(
    (dash?.active_strategy?.doc.assets ?? []).map((a) => [a.symbol, a.target_weight]),
  );

  const rows = positions
    .filter(
      (p) =>
        p.symbol.toLowerCase().includes(search.toLowerCase()) ||
        (p.name ?? "").toLowerCase().includes(search.toLowerCase()),
    )
    .map((p) => {
      const weight = totalValue > 0 ? p.market_value / totalValue : 0;
      const target = targets.get(p.symbol) ?? null;
      const drift = target !== null ? weight - target : null;
      return { ...p, weight, target, drift };
    })
    .sort((a, b) => {
      const dir = sortDesc ? -1 : 1;
      switch (sortKey) {
        case "symbol":
          return dir * a.symbol.localeCompare(b.symbol);
        case "unrealized_pl":
          return dir * ((a.unrealized_pl ?? 0) - (b.unrealized_pl ?? 0));
        case "weight":
          return dir * (a.weight - b.weight);
        default:
          return dir * (a.market_value - b.market_value);
      }
    });

  const header = (key: SortKey, label: string, className?: string) => (
    <TableHead className={className}>
      <button
        type="button"
        className="inline-flex items-center gap-1 uppercase cursor-pointer hover:text-foreground"
        onClick={() => {
          if (sortKey === key) setSortDesc((d) => !d);
          else {
            setSortKey(key);
            setSortDesc(true);
          }
        }}
      >
        {label}
        {sortKey === key &&
          (sortDesc ? (
            <ArrowDown className="h-3 w-3" aria-hidden />
          ) : (
            <ArrowUp className="h-3 w-3" aria-hidden />
          ))}
      </button>
    </TableHead>
  );

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Portfolio</h1>
          <p className="text-sm text-muted-foreground">
            {positions.length} positions · Cash {formatMoney(dash?.summary?.cash)} · Updated{" "}
            {timeAgo(activeConnection.last_sync_at)}
          </p>
        </div>
        <div className="flex gap-2">
          {isManual && (
            <>
              <Button variant="outline" onClick={() => setManualOpen(true)}>
                <Plus aria-hidden /> Add position
              </Button>
              <label className="cursor-pointer">
                <input
                  type="file"
                  accept=".csv,text/csv"
                  className="sr-only"
                  onChange={async (e) => {
                    const file = e.target.files?.[0];
                    if (file) importCsv.mutate(await file.text());
                    e.target.value = "";
                  }}
                />
                <span className="inline-flex h-9 items-center gap-2 rounded-md border border-border px-4 text-sm font-medium hover:bg-muted">
                  <Upload className="h-4 w-4" aria-hidden /> Import CSV
                </span>
              </label>
            </>
          )}
          <Button
            variant="outline"
            onClick={() => exportCsv.mutate()}
            disabled={exportCsv.isPending}
          >
            <Download aria-hidden /> Export CSV
          </Button>
          {!isManual && (
            <Button variant="outline" onClick={() => sync.mutate()} disabled={sync.isPending}>
              <RefreshCw className={sync.isPending ? "animate-spin" : ""} aria-hidden /> Refresh
            </Button>
          )}
        </div>
      </div>

      {exportCsv.isSuccess && (
        <p className="text-sm text-success" role="status">
          Exported to {exportCsv.data}
        </p>
      )}
      {importCsv.isSuccess && (
        <p className="text-sm text-success" role="status">
          Imported {importCsv.data} positions.
        </p>
      )}
      {importCsv.isError && <ErrorNote error={importCsv.error} />}

      <div className="relative max-w-xs">
        <Search
          className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground"
          aria-hidden
        />
        <Input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search symbol or name"
          className="pl-9"
          aria-label="Search positions"
        />
      </div>

      {isLoading ? (
        <Skeleton className="h-72" />
      ) : rows.length === 0 ? (
        <EmptyState
          title={search ? "No matching positions" : "No positions yet"}
          description={
            search
              ? "Try a different search."
              : isManual
                ? "Add positions manually or import a CSV with columns: symbol, quantity, price."
                : "Positions appear after your first synchronization."
          }
        />
      ) : (
        <Card>
          <CardContent className="p-0">
            <Table>
              <TableHeader>
                <TableRow>
                  {header("symbol", "Symbol")}
                  <TableHead className="text-right">Qty</TableHead>
                  <TableHead className="text-right">Price</TableHead>
                  {header("market_value", "Value", "text-right")}
                  {header("unrealized_pl", "Gain / Loss", "text-right")}
                  {header("weight", "Weight", "text-right")}
                  <TableHead className="text-right">Target</TableHead>
                  <TableHead className="text-right">Drift</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((p) => (
                  <TableRow key={p.symbol}>
                    <TableCell>
                      <div className="font-semibold">{p.symbol}</div>
                      {p.name && (
                        <div className="max-w-44 truncate text-xs text-muted-foreground">
                          {p.name}
                        </div>
                      )}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">
                      {formatQty(p.quantity)}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">
                      {formatMoney(p.current_price)}
                    </TableCell>
                    <TableCell className="text-right font-medium tabular-nums">
                      {formatMoney(p.market_value)}
                    </TableCell>
                    <TableCell
                      className={cn(
                        "text-right tabular-nums",
                        (p.unrealized_pl ?? 0) >= 0 ? "text-success" : "text-destructive",
                      )}
                    >
                      {p.unrealized_pl !== null ? formatMoney(p.unrealized_pl) : "—"}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">
                      {weightPercent(p.weight)}
                    </TableCell>
                    <TableCell className="text-right tabular-nums text-muted-foreground">
                      {p.target !== null ? weightPercent(p.target) : "—"}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">
                      {p.drift !== null ? (
                        <span
                          className={
                            Math.abs(p.drift) > 0.02 ? "text-warning" : "text-muted-foreground"
                          }
                        >
                          {p.drift >= 0 ? "+" : ""}
                          {weightPercent(p.drift)}
                        </span>
                      ) : (
                        "—"
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      {isManual && (
        <ManualPositionDialog
          open={manualOpen}
          onOpenChange={setManualOpen}
          connectionId={activeConnection.id}
          cash={dash?.summary?.cash ?? 0}
          onSaved={() => void qc.invalidateQueries({ queryKey: ["dashboard"] })}
        />
      )}
    </div>
  );
}

function ManualPositionDialog({
  open,
  onOpenChange,
  connectionId,
  cash,
  onSaved,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  connectionId: string;
  cash: number;
  onSaved: () => void;
}) {
  const [symbol, setSymbol] = React.useState("");
  const [quantity, setQuantity] = React.useState("");
  const [price, setPrice] = React.useState("");
  const [cashInput, setCashInput] = React.useState(String(cash));

  React.useEffect(() => setCashInput(String(cash)), [cash]);

  const save = useMutation({
    mutationFn: async () => {
      if (symbol.trim()) {
        await api.upsertManualPosition(connectionId, symbol, Number(quantity), Number(price));
      }
      if (Number(cashInput) !== cash) {
        await api.setManualCash(connectionId, Number(cashInput));
      }
    },
    onSuccess: () => {
      onSaved();
      onOpenChange(false);
      setSymbol("");
      setQuantity("");
      setPrice("");
    },
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add or update a position</DialogTitle>
          <DialogDescription>
            Enter a holding as it appears at your brokerage. Setting quantity to 0 removes it.
          </DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-3 gap-3">
          <div className="space-y-1.5">
            <Label htmlFor="mp-symbol">Symbol</Label>
            <Input
              id="mp-symbol"
              value={symbol}
              onChange={(e) => setSymbol(e.target.value.toUpperCase())}
              placeholder="VOO"
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="mp-qty">Quantity</Label>
            <Input
              id="mp-qty"
              type="number"
              min={0}
              step="any"
              value={quantity}
              onChange={(e) => setQuantity(e.target.value)}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="mp-price">Price ($)</Label>
            <Input
              id="mp-price"
              type="number"
              min={0}
              step="any"
              value={price}
              onChange={(e) => setPrice(e.target.value)}
            />
          </div>
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="mp-cash">Cash balance ($)</Label>
          <Input
            id="mp-cash"
            type="number"
            min={0}
            step="any"
            value={cashInput}
            onChange={(e) => setCashInput(e.target.value)}
          />
        </div>
        {save.isError && <ErrorNote error={save.error} />}
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={() => save.mutate()} disabled={save.isPending}>
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

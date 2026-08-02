import * as React from "react";
import { useMutation } from "@tanstack/react-query";
import { AlertOctagon, RotateCcw } from "lucide-react";
import { api, type AppSettings } from "@/lib/api";
import { useApp } from "@/context/AppContext";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { ConfirmDialog, Disclaimer, ErrorNote } from "@/components/shared";

export default function SettingsPage() {
  const { state, activeConnection, refresh } = useApp();
  const [settings, setSettings] = React.useState<AppSettings | null>(null);
  const [stopConfirm, setStopConfirm] = React.useState(false);
  const [resetConfirm, setResetConfirm] = React.useState(false);

  React.useEffect(() => {
    if (state && !settings) setSettings(structuredClone(state.settings));
  }, [state, settings]);

  const save = useMutation({
    mutationFn: (s: AppSettings) => api.updateSettings(s),
    onSuccess: refresh,
  });

  const emergency = useMutation({
    mutationFn: (enabled: boolean) => api.setEmergencyStop(enabled),
    onSuccess: (s) => {
      setSettings(structuredClone(s));
      setStopConfirm(false);
      refresh();
    },
  });

  const cancelAll = useMutation({
    mutationFn: () => api.cancelAllOpen(activeConnection!.id),
  });

  const resetDemo = useMutation({
    mutationFn: api.resetDemo,
    onSuccess: () => {
      setResetConfirm(false);
      refresh();
    },
  });

  if (!settings) return null;

  const update = (patch: Partial<AppSettings>) => {
    const next = { ...settings, ...patch };
    setSettings(next);
    save.mutate(next);
  };
  const updateSafety = (patch: Partial<AppSettings["safety"]>) =>
    update({ safety: { ...settings.safety, ...patch } });

  const hasDemo = state?.connections.some((c) => c.broker_kind === "mock");

  return (
    <div className="max-w-3xl space-y-6">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Settings</h1>
        <p className="text-sm text-muted-foreground">
          Safety limits, synchronization, and appearance.
        </p>
      </div>

      <Card className={settings.emergency_stop ? "border-destructive" : ""}>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <AlertOctagon className="h-4 w-4 text-destructive" aria-hidden /> Emergency stop
          </CardTitle>
          <CardDescription>
            Blocks all new order submission until you turn it off. Existing records are preserved,
            and you can cancel eligible open orders.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-wrap items-center gap-3">
          {settings.emergency_stop ? (
            <>
              <Button
                variant="outline"
                onClick={() => emergency.mutate(false)}
                disabled={emergency.isPending}
              >
                Disable emergency stop
              </Button>
              {activeConnection && activeConnection.broker_kind !== "manual" && (
                <Button
                  variant="destructive"
                  onClick={() => cancelAll.mutate()}
                  disabled={cancelAll.isPending}
                >
                  Cancel open orders on {activeConnection.label}
                </Button>
              )}
              {cancelAll.isSuccess && (
                <p className="text-sm text-success" role="status">
                  Canceled {cancelAll.data} open orders.
                </p>
              )}
            </>
          ) : (
            <Button variant="destructive" onClick={() => setStopConfirm(true)}>
              Enable emergency stop
            </Button>
          )}
          {emergency.isError && <ErrorNote error={emergency.error} />}
          {cancelAll.isError && <ErrorNote error={cancelAll.error} />}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Trading safety limits</CardTitle>
          <CardDescription>
            Applied to every rebalance on top of each strategy's own risk controls. Explicit batch
            approval is always required and cannot be disabled in this version.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <NumberField
              label="Maximum order value ($)"
              value={settings.safety.max_order_value}
              onCommit={(v) => updateSafety({ max_order_value: v })}
            />
            <NumberField
              label="Maximum batch value ($)"
              value={settings.safety.max_batch_value}
              onCommit={(v) => updateSafety({ max_batch_value: v })}
            />
            <NumberField
              label="Minimum order value ($)"
              value={settings.safety.min_order_value}
              onCommit={(v) => updateSafety({ min_order_value: v })}
            />
            <NumberField
              label="Maximum position weight (%)"
              value={Math.round(settings.safety.max_position_weight * 100)}
              onCommit={(v) => updateSafety({ max_position_weight: v / 100 })}
            />
          </div>
          <ToggleRow
            label="Allow selling"
            description="Rebalances may propose sells for overweight or removed positions."
            checked={settings.safety.allow_sells}
            onChange={(v) => updateSafety({ allow_sells: v })}
          />
          <ToggleRow
            label="Allow fractional shares"
            description="Trade partial shares when the brokerage supports it."
            checked={settings.safety.allow_fractional}
            onChange={(v) => updateSafety({ allow_fractional: v })}
          />
          <ToggleRow
            label="Read-only mode"
            description="Browse and preview everything, but block all order submission."
            checked={settings.read_only}
            onChange={(v) => update({ read_only: v })}
          />
          <div className="rounded-lg border border-border bg-muted/40 p-4 text-sm text-muted-foreground">
            Orders are submitted as market orders. Limit-order support is planned but not yet
            available.
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Background synchronization</CardTitle>
          <CardDescription>
            Refreshes balances, positions, and fills while the app is open. Background sync is
            read-only and can never submit trades.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-3">
            <Label htmlFor="set-sync">Frequency</Label>
            <Select
              id="set-sync"
              className="w-48"
              value={
                settings.sync_interval_minutes === null
                  ? "manual"
                  : String(settings.sync_interval_minutes)
              }
              onChange={(e) =>
                update({
                  sync_interval_minutes:
                    e.target.value === "manual" ? null : Number(e.target.value),
                })
              }
            >
              <option value="manual">Manual only</option>
              <option value="5">Every 5 minutes</option>
              <option value="15">Every 15 minutes</option>
              <option value="60">Every hour</option>
            </Select>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Appearance</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-3">
            <Label htmlFor="set-theme">Theme</Label>
            <Select
              id="set-theme"
              className="w-48"
              value={settings.theme}
              onChange={(e) => update({ theme: e.target.value })}
            >
              <option value="system">Match system</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </Select>
          </div>
        </CardContent>
      </Card>

      {hasDemo && (
        <Card>
          <CardHeader>
            <CardTitle>Demo data</CardTitle>
            <CardDescription>
              Restore the demo portfolio, orders, and history to their initial state.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Button variant="outline" onClick={() => setResetConfirm(true)}>
              <RotateCcw aria-hidden /> Reset demo data
            </Button>
            {resetDemo.isSuccess && (
              <p className="mt-2 text-sm text-success" role="status">
                Demo data was reset.
              </p>
            )}
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle>About PortfolioOS</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3 text-sm text-muted-foreground">
          <p>Version {state?.version} · MIT licensed open-source software.</p>
          <Disclaimer />
        </CardContent>
      </Card>

      <ConfirmDialog
        open={stopConfirm}
        onOpenChange={setStopConfirm}
        title="Enable emergency stop?"
        description="All new order submission will be blocked until you disable it. Automated background sync continues (read-only)."
        confirmLabel="Enable emergency stop"
        destructive
        busy={emergency.isPending}
        onConfirm={() => emergency.mutate(true)}
      />
      <ConfirmDialog
        open={resetConfirm}
        onOpenChange={setResetConfirm}
        title="Reset demo data?"
        description="The demo portfolio returns to its starting state. Your strategies and audit history are kept."
        confirmLabel="Reset demo"
        busy={resetDemo.isPending}
        onConfirm={() => resetDemo.mutate()}
      />
    </div>
  );
}

function NumberField({
  label,
  value,
  onCommit,
}: {
  label: string;
  value: number;
  onCommit: (v: number) => void;
}) {
  const id = React.useId();
  const [draft, setDraft] = React.useState(String(value));
  React.useEffect(() => setDraft(String(value)), [value]);
  return (
    <div className="space-y-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        type="number"
        min={0}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => {
          const n = Number(draft);
          if (!Number.isNaN(n) && n >= 0 && n !== value) onCommit(n);
        }}
      />
    </div>
  );
}

function ToggleRow({
  label,
  description,
  checked,
  onChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between rounded-lg border border-border p-4">
      <div>
        <p className="text-sm font-medium">{label}</p>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>
      <Switch checked={checked} onCheckedChange={onChange} aria-label={label} />
    </div>
  );
}

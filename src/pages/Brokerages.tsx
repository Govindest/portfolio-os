import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircle2, Loader2, Plus, RefreshCw, Unplug } from "lucide-react";
import { api, type ConnectionDto } from "@/lib/api";
import { useApp } from "@/context/AppContext";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select } from "@/components/ui/select";
import { ConfirmDialog, ErrorNote, EmptyState, StatusBadge } from "@/components/shared";
import { ConnectWizard } from "@/components/ConnectWizard";
import { maskAccountId, timeAgo } from "@/lib/utils";

const BROKER_NAMES: Record<string, string> = {
  mock: "Demo Mode",
  alpaca: "Alpaca Paper",
  tradier: "Tradier Sandbox",
  ibkr: "Interactive Brokers",
  manual: "Manual Portfolio",
};

export default function Brokerages() {
  const { state, refresh } = useApp();
  const [wizardOpen, setWizardOpen] = React.useState(false);
  const connections = state?.connections ?? [];

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Brokerages</h1>
          <p className="text-sm text-muted-foreground">
            Manage connections. Credentials stay in your operating system's credential manager.
          </p>
        </div>
        <Button onClick={() => setWizardOpen(true)}>
          <Plus aria-hidden /> Add connection
        </Button>
      </div>

      {connections.length === 0 ? (
        <EmptyState
          title="No brokerages connected"
          description="Connect Alpaca Paper, explore demo mode, or track a portfolio manually."
          action={
            <Button onClick={() => setWizardOpen(true)}>
              <Plus aria-hidden /> Connect a brokerage
            </Button>
          }
        />
      ) : (
        <div className="space-y-4">
          {connections.map((c) => (
            <ConnectionCard key={c.id} connection={c} onChanged={refresh} />
          ))}
        </div>
      )}

      <Dialog open={wizardOpen} onOpenChange={setWizardOpen}>
        <DialogContent className="max-w-3xl">
          <DialogHeader>
            <DialogTitle>Connect a brokerage</DialogTitle>
            <DialogDescription>
              Paper trading and sandboxes only in this version — a safe place to learn the workflow.
            </DialogDescription>
          </DialogHeader>
          <ConnectWizard
            onDone={() => {
              setWizardOpen(false);
              refresh();
            }}
            onCancel={() => setWizardOpen(false)}
          />
        </DialogContent>
      </Dialog>
    </div>
  );
}

function ConnectionCard({
  connection: c,
  onChanged,
}: {
  connection: ConnectionDto;
  onChanged: () => void;
}) {
  const qc = useQueryClient();
  const [disconnectOpen, setDisconnectOpen] = React.useState(false);
  const [credsOpen, setCredsOpen] = React.useState(false);

  const test = useMutation({ mutationFn: () => api.testConnection(c.id) });
  const sync = useMutation({
    mutationFn: () => api.syncNow(c.id),
    onSuccess: () => {
      onChanged();
      void qc.invalidateQueries({ queryKey: ["dashboard"] });
    },
  });
  const disconnect = useMutation({
    mutationFn: () => api.disconnect(c.id),
    onSuccess: () => {
      setDisconnectOpen(false);
      onChanged();
    },
  });
  const setDefault = useMutation({
    mutationFn: (accountId: string) => api.setDefaultAccount(c.id, accountId),
    onSuccess: onChanged,
  });

  const { data: syncs } = useQuery({
    queryKey: ["syncs", c.id],
    queryFn: () => api.recentSyncs(c.id),
  });
  const lastFailure = syncs?.find((s) => !s.success);

  const needsCreds = c.broker_kind === "alpaca" || c.broker_kind === "tradier";

  return (
    <Card>
      <CardHeader>
        <div className="flex items-start justify-between">
          <div>
            <CardTitle className="flex items-center gap-2">
              {c.label}
              {c.paper && <Badge variant="accent">Paper</Badge>}
              {c.broker_kind === "tradier" && <Badge variant="warning">Beta</Badge>}
              {c.broker_kind === "ibkr" && (
                <Badge variant="warning">Experimental · read-only</Badge>
              )}
            </CardTitle>
            <CardDescription>
              {BROKER_NAMES[c.broker_kind] ?? c.broker_kind}
              {c.masked_credential && <> · Credential {c.masked_credential}</>}
              {" · "}Last synced {timeAgo(c.last_sync_at)}
            </CardDescription>
          </div>
          <StatusBadge status={c.status} />
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {c.accounts.length > 0 && (
          <div className="flex items-center gap-3 text-sm">
            <Label htmlFor={`acct-${c.id}`} className="shrink-0">
              Default account
            </Label>
            <Select
              id={`acct-${c.id}`}
              className="w-64"
              value={c.default_account_id ?? c.accounts[0].id}
              onChange={(e) => setDefault.mutate(e.target.value)}
            >
              {c.accounts.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name} ({maskAccountId(a.id)})
                </option>
              ))}
            </Select>
          </div>
        )}

        {c.last_error && (
          <p className="text-sm text-destructive" role="alert">
            Last connection error: {c.last_error}
          </p>
        )}
        {lastFailure && !c.last_error && (
          <p className="text-sm text-muted-foreground">
            A recent sync failed: {String(lastFailure.detail ?? "unknown reason")}
          </p>
        )}

        {test.isSuccess && (
          <p className="flex items-center gap-1.5 text-sm text-success" role="status">
            <CheckCircle2 className="h-4 w-4" aria-hidden /> {test.data.message}
          </p>
        )}
        {test.isError && <ErrorNote error={test.error} />}
        {sync.isError && <ErrorNote error={sync.error} />}

        <div className="flex flex-wrap gap-2">
          {c.broker_kind !== "manual" && (
            <>
              <Button
                variant="outline"
                size="sm"
                onClick={() => test.mutate()}
                disabled={test.isPending}
              >
                {test.isPending ? (
                  <Loader2 className="animate-spin" aria-hidden />
                ) : (
                  <CheckCircle2 aria-hidden />
                )}
                Test connection
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => sync.mutate()}
                disabled={sync.isPending}
              >
                <RefreshCw className={sync.isPending ? "animate-spin" : ""} aria-hidden /> Sync now
              </Button>
            </>
          )}
          {needsCreds && (
            <Button variant="outline" size="sm" onClick={() => setCredsOpen(true)}>
              Replace credentials
            </Button>
          )}
          <Button
            variant="ghost"
            size="sm"
            className="text-destructive"
            onClick={() => setDisconnectOpen(true)}
          >
            <Unplug aria-hidden /> Disconnect
          </Button>
        </div>
      </CardContent>

      <ConfirmDialog
        open={disconnectOpen}
        onOpenChange={setDisconnectOpen}
        title={`Disconnect ${c.label}?`}
        description="Stored credentials are removed from your system credential manager. Local history and the audit trail are preserved."
        confirmLabel="Disconnect"
        destructive
        busy={disconnect.isPending}
        onConfirm={() => disconnect.mutate()}
      />

      <ReplaceCredentialsDialog
        open={credsOpen}
        onOpenChange={setCredsOpen}
        connection={c}
        onChanged={onChanged}
      />
    </Card>
  );
}

function ReplaceCredentialsDialog({
  open,
  onOpenChange,
  connection,
  onChanged,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  connection: ConnectionDto;
  onChanged: () => void;
}) {
  const [key, setKey] = React.useState("");
  const [secret, setSecret] = React.useState("");
  const replace = useMutation({
    mutationFn: () => api.replaceCredentials(connection.id, key.trim(), secret.trim()),
    onSuccess: () => {
      onOpenChange(false);
      setKey("");
      setSecret("");
      onChanged();
    },
  });
  const needsSecret = connection.broker_kind === "alpaca";

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Replace credentials</DialogTitle>
          <DialogDescription>
            The new credentials are verified with the brokerage before the old ones are replaced.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="rc-key">{needsSecret ? "API key" : "Access token"}</Label>
            <Input
              id="rc-key"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              autoComplete="off"
              spellCheck={false}
            />
          </div>
          {needsSecret && (
            <div className="space-y-1.5">
              <Label htmlFor="rc-secret">API secret</Label>
              <Input
                id="rc-secret"
                type="password"
                value={secret}
                onChange={(e) => setSecret(e.target.value)}
                autoComplete="off"
              />
            </div>
          )}
          {replace.isError && <ErrorNote error={replace.error} />}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            onClick={() => replace.mutate()}
            disabled={replace.isPending || key.trim().length < 6}
          >
            {replace.isPending ? "Verifying…" : "Verify and replace"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

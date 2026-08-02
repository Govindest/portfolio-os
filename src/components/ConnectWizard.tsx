import * as React from "react";
import { useMutation } from "@tanstack/react-query";
import {
  ArrowLeft,
  CheckCircle2,
  ClipboardPaste,
  Eye,
  EyeOff,
  ExternalLink,
  FlaskConical,
  Landmark,
  Loader2,
  PenLine,
  Rocket,
  Sparkles,
} from "lucide-react";
import { api, type ConnectionDto, type ConnectionStatus } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { ErrorNote, PaperBadge } from "@/components/shared";
import { formatMoney, maskAccountId, timeAgo } from "@/lib/utils";
import { cn } from "@/lib/utils";

type BrokerChoice = "alpaca" | "tradier" | "ibkr" | "mock" | "manual";

interface BrokerCardInfo {
  kind: BrokerChoice;
  name: string;
  monogram: string;
  connection: string;
  difficulty: "Easy" | "Moderate" | "Advanced";
  features: string;
  paper: string;
  recommended?: boolean;
  beta?: boolean;
  experimental?: boolean;
  icon: React.ComponentType<{ className?: string }>;
}

const BROKERS: BrokerCardInfo[] = [
  {
    kind: "alpaca",
    name: "Alpaca Paper",
    monogram: "A",
    connection: "API key + secret",
    difficulty: "Easy",
    features: "Balances, positions, orders, paper trading, fractional shares",
    paper: "Paper trading",
    recommended: true,
    icon: Rocket,
  },
  {
    kind: "mock",
    name: "Demo Mode",
    monogram: "D",
    connection: "No account needed",
    difficulty: "Easy",
    features: "Full product tour with realistic sample data",
    paper: "Simulated",
    icon: Sparkles,
  },
  {
    kind: "tradier",
    name: "Tradier Sandbox",
    monogram: "T",
    connection: "Access token",
    difficulty: "Moderate",
    features: "Balances, positions, sandbox orders (whole shares)",
    paper: "Sandbox",
    beta: true,
    icon: FlaskConical,
  },
  {
    kind: "ibkr",
    name: "Interactive Brokers",
    monogram: "IB",
    connection: "Local Client Portal Gateway",
    difficulty: "Advanced",
    features: "Read-only balances and positions in this version",
    paper: "Paper supported",
    experimental: true,
    icon: Landmark,
  },
  {
    kind: "manual",
    name: "Manual Portfolio",
    monogram: "M",
    connection: "Enter or import positions",
    difficulty: "Easy",
    features: "Track any brokerage, CSV import, trade-list export",
    paper: "Tracking only",
    icon: PenLine,
  },
];

const SETUP_STEPS: Record<
  string,
  { steps: string[]; docsUrl: string; docsLabel: string; footnote?: string }
> = {
  alpaca: {
    steps: [
      "Create a free account at app.alpaca.markets (no funding required for paper trading).",
      "In the dashboard, use the account switcher in the top-left and choose Paper Trading.",
      "On the paper overview page, find the API Keys panel and click Generate New Keys.",
      "Copy the shorter value labeled “API Key ID” (it starts with PK) — that is your API key.",
      "Copy the longer value labeled “Secret Key” — Alpaca shows it only once; that is your API secret.",
      "Paste both below, click Test connection, then Save and continue.",
    ],
    docsUrl: "https://docs.alpaca.markets/docs/getting-started",
    docsLabel: "Alpaca setup guide",
    footnote:
      "To disconnect later, use Brokerages → Disconnect. That removes the stored credentials from your computer; you can also revoke the key in the Alpaca dashboard.",
  },
  tradier: {
    steps: [
      "Open a Tradier developer account.",
      "Create a sandbox application to get a sandbox access token.",
      "Copy the access token.",
      "Paste it below, then test the connection.",
    ],
    docsUrl: "https://documentation.tradier.com/brokerage-api",
    docsLabel: "Tradier API documentation",
  },
  ibkr: {
    steps: [
      "Install and start the IBKR Client Portal Gateway on this computer (it listens on https://localhost:5000).",
      "Sign in through the gateway's browser window — use a paper account. PortfolioOS never sees your IBKR password.",
      "Keep the gateway running while you use PortfolioOS; IBKR sessions expire and may need re-login.",
      "Test the connection below — PortfolioOS talks only to your local gateway.",
    ],
    docsUrl: "https://www.interactivebrokers.com/campus/ibkr-api-page/cpapi-v1/",
    docsLabel: "Client Portal Gateway guide",
    footnote:
      "This integration is read-only in this version: balances and positions sync, but PortfolioOS cannot place IBKR orders.",
  },
};

export function ConnectWizard({
  onDone,
  onCancel,
  initialKind,
}: {
  onDone: (connection: ConnectionDto) => void;
  onCancel?: () => void;
  initialKind?: BrokerChoice;
}) {
  const [step, setStep] = React.useState<"choose" | "setup" | "success">(
    initialKind ? "setup" : "choose",
  );
  const [kind, setKind] = React.useState<BrokerChoice | null>(initialKind ?? null);
  const [key, setKey] = React.useState("");
  const [secret, setSecret] = React.useState("");
  const [gatewayUrl, setGatewayUrl] = React.useState("");
  const [label, setLabel] = React.useState("");
  const [showSecret, setShowSecret] = React.useState(false);
  const [testResult, setTestResult] = React.useState<ConnectionStatus | null>(null);
  const [created, setCreated] = React.useState<ConnectionDto | null>(null);

  const info = BROKERS.find((b) => b.kind === kind);

  const test = useMutation({
    mutationFn: () =>
      api.previewConnection(kind!, key.trim(), secret.trim(), gatewayUrl.trim() || undefined),
    onSuccess: setTestResult,
    onMutate: () => setTestResult(null),
  });

  const create = useMutation({
    mutationFn: () =>
      api.createConnection(
        kind!,
        label.trim() || info?.name || "Brokerage",
        key.trim(),
        secret.trim(),
        gatewayUrl.trim() || undefined,
      ),
    onSuccess: (conn) => {
      setCreated(conn);
      setStep("success");
    },
  });

  const pasteInto = async (setter: (v: string) => void) => {
    try {
      setter((await navigator.clipboard.readText()).trim());
    } catch {
      // Clipboard may be unavailable; the user can paste manually.
    }
  };

  const needsKey = kind === "alpaca" || kind === "tradier";
  const needsSecret = kind === "alpaca";
  const keyValid = !needsKey || key.trim().length > 5;
  const secretValid = !needsSecret || secret.trim().length > 5;
  const canSave = keyValid && secretValid && !create.isPending;

  if (step === "choose") {
    return (
      <div className="space-y-4">
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {BROKERS.map((b) => (
            <button
              key={b.kind}
              type="button"
              onClick={() => {
                setKind(b.kind);
                setLabel(b.name);
                setTestResult(null);
                setStep("setup");
              }}
              className={cn(
                "group relative rounded-lg border border-border bg-card p-4 text-left transition-all hover:border-primary/50 hover:shadow-md cursor-pointer",
                b.recommended && "border-primary/40 ring-1 ring-primary/20",
              )}
            >
              <div className="flex items-start justify-between">
                <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-accent text-sm font-bold text-accent-foreground">
                  {b.monogram}
                </div>
                <div className="flex flex-col items-end gap-1">
                  {b.recommended && <Badge>Recommended for first-time setup</Badge>}
                  {b.beta && <Badge variant="warning">Beta</Badge>}
                  {b.experimental && <Badge variant="warning">Experimental · read-only</Badge>}
                </div>
              </div>
              <p className="mt-3 font-semibold">{b.name}</p>
              <p className="mt-1 text-xs text-muted-foreground">{b.features}</p>
              <div className="mt-3 flex flex-wrap gap-1.5 text-xs">
                <Badge variant="outline">{b.connection}</Badge>
                <Badge variant="outline">{b.difficulty}</Badge>
                <Badge variant="accent">{b.paper}</Badge>
              </div>
            </button>
          ))}
        </div>
        {onCancel && (
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
        )}
      </div>
    );
  }

  if (step === "success" && created) {
    const account = created.accounts[0];
    return (
      <SuccessScreen
        connection={created}
        accountName={account?.name}
        onDone={() => onDone(created)}
      />
    );
  }

  // setup step
  const guide = kind ? SETUP_STEPS[kind] : undefined;
  return (
    <div className="space-y-5">
      <button
        type="button"
        onClick={() => setStep("choose")}
        className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground cursor-pointer"
      >
        <ArrowLeft className="h-4 w-4" aria-hidden /> Choose a different brokerage
      </button>

      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-accent text-sm font-bold text-accent-foreground">
          {info?.monogram}
        </div>
        <div>
          <h2 className="font-semibold">{info?.name}</h2>
          <p className="text-sm text-muted-foreground">{info?.connection}</p>
        </div>
      </div>

      {guide && (
        <Card>
          <CardContent className="pt-5">
            <ol className="space-y-2 text-sm">
              {guide.steps.map((s, i) => (
                <li key={i} className="flex gap-3">
                  <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-accent text-xs font-semibold text-accent-foreground">
                    {i + 1}
                  </span>
                  {s}
                </li>
              ))}
            </ol>
            <a
              href={guide.docsUrl}
              target="_blank"
              rel="noreferrer"
              className="mt-3 inline-flex items-center gap-1 text-sm text-primary hover:underline"
            >
              <ExternalLink className="h-3.5 w-3.5" aria-hidden /> {guide.docsLabel}
            </a>
            {guide.footnote && (
              <p className="mt-3 border-t border-border pt-3 text-xs text-muted-foreground">
                {guide.footnote}
              </p>
            )}
          </CardContent>
        </Card>
      )}

      {kind === "mock" && (
        <p className="text-sm text-muted-foreground">
          Demo mode uses a realistic simulated portfolio stored on this computer. No account, no
          credentials, and nothing leaves your machine. You can reset the demo at any time from
          Settings.
        </p>
      )}
      {kind === "manual" && (
        <p className="text-sm text-muted-foreground">
          Track a portfolio at any brokerage by entering positions yourself or importing a CSV.
          PortfolioOS computes allocations and drift, and exports a trade list you can place with
          your broker.
        </p>
      )}

      <div className="space-y-4">
        <div className="space-y-1.5">
          <Label htmlFor="conn-label">Connection name</Label>
          <Input
            id="conn-label"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            placeholder={info?.name}
          />
        </div>

        {needsKey && (
          <div className="space-y-1.5">
            <Label htmlFor="conn-key">{kind === "alpaca" ? "API key" : "Access token"}</Label>
            <div className="flex gap-2">
              <Input
                id="conn-key"
                value={key}
                onChange={(e) => setKey(e.target.value)}
                placeholder={kind === "alpaca" ? "PK…" : "Sandbox access token"}
                autoComplete="off"
                spellCheck={false}
              />
              <Button
                variant="outline"
                size="icon"
                aria-label="Paste from clipboard"
                onClick={() => void pasteInto(setKey)}
              >
                <ClipboardPaste />
              </Button>
            </div>
            {!keyValid && key.length > 0 && (
              <p className="text-xs text-destructive">
                This looks too short to be a valid credential.
              </p>
            )}
          </div>
        )}

        {needsSecret && (
          <div className="space-y-1.5">
            <Label htmlFor="conn-secret">API secret</Label>
            <div className="flex gap-2">
              <Input
                id="conn-secret"
                type={showSecret ? "text" : "password"}
                value={secret}
                onChange={(e) => setSecret(e.target.value)}
                autoComplete="off"
                spellCheck={false}
              />
              <Button
                variant="outline"
                size="icon"
                aria-label={showSecret ? "Hide secret" : "Show secret"}
                onClick={() => setShowSecret((s) => !s)}
              >
                {showSecret ? <EyeOff /> : <Eye />}
              </Button>
              <Button
                variant="outline"
                size="icon"
                aria-label="Paste from clipboard"
                onClick={() => void pasteInto(setSecret)}
              >
                <ClipboardPaste />
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              Stored in your operating system's credential manager — never in files or databases.
            </p>
          </div>
        )}

        {kind === "ibkr" && (
          <div className="space-y-1.5">
            <Label htmlFor="conn-gateway">Gateway URL (optional)</Label>
            <Input
              id="conn-gateway"
              value={gatewayUrl}
              onChange={(e) => setGatewayUrl(e.target.value)}
              placeholder="https://localhost:5000/v1/api"
            />
          </div>
        )}
      </div>

      {test.isError && <ErrorNote error={test.error} />}
      {create.isError && <ErrorNote error={create.error} />}
      {testResult && (
        <div
          role="status"
          className="flex items-center gap-2 rounded-md border border-success/30 bg-success/5 px-3 py-2 text-sm"
        >
          <CheckCircle2 className="h-4 w-4 text-success" aria-hidden />
          {testResult.message}
        </div>
      )}

      <div className="flex gap-2">
        {kind !== "manual" && (
          <Button
            variant="outline"
            onClick={() => test.mutate()}
            disabled={test.isPending || !keyValid || !secretValid}
          >
            {test.isPending && <Loader2 className="animate-spin" aria-hidden />}
            Test connection
          </Button>
        )}
        <Button onClick={() => create.mutate()} disabled={!canSave}>
          {create.isPending && <Loader2 className="animate-spin" aria-hidden />}
          {kind === "manual" || kind === "mock" ? "Create and continue" : "Save and continue"}
        </Button>
      </div>
    </div>
  );
}

function SuccessScreen({
  connection,
  accountName,
  onDone,
}: {
  connection: ConnectionDto;
  accountName?: string;
  onDone: () => void;
}) {
  const [dash, setDash] = React.useState<Awaited<ReturnType<typeof api.getDashboard>> | null>(null);
  const accountId = connection.default_account_id ?? connection.accounts[0]?.id;

  React.useEffect(() => {
    if (accountId) {
      api
        .getDashboard(connection.id, accountId)
        .then(setDash)
        .catch(() => setDash(null));
    }
  }, [connection.id, accountId]);

  return (
    <div className="space-y-6 text-center">
      <div className="flex flex-col items-center gap-3">
        <div className="flex h-14 w-14 items-center justify-center rounded-full bg-success/15">
          <CheckCircle2 className="h-8 w-8 text-success" aria-hidden />
        </div>
        <h2 className="text-xl font-semibold">You're connected</h2>
        <p className="text-sm text-muted-foreground">
          {accountName ?? connection.label} · Account {accountId ? maskAccountId(accountId) : "—"}
        </p>
        <div className="flex gap-2">
          {connection.paper && <PaperBadge />}
          <Badge variant="success">Synced {timeAgo(connection.last_sync_at)}</Badge>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <Stat label="Portfolio value" value={formatMoney(dash?.summary?.portfolio_value)} />
        <Stat label="Cash" value={formatMoney(dash?.summary?.cash)} />
        <Stat label="Buying power" value={formatMoney(dash?.summary?.buying_power)} />
        <Stat label="Positions" value={dash ? String(dash.positions.length) : "—"} />
      </div>

      <Button size="lg" onClick={onDone}>
        Go to your dashboard
      </Button>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="mt-1 text-lg font-semibold tabular-nums">{value}</p>
    </div>
  );
}

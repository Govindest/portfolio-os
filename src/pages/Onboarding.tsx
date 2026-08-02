import * as React from "react";
import { useMutation } from "@tanstack/react-query";
import { Lock, PieChart, ShieldCheck, Wallet } from "lucide-react";
import { api, type AppSettings, type ConnectionDto } from "@/lib/api";
import { useApp } from "@/context/AppContext";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { ConnectWizard } from "@/components/ConnectWizard";
import { Disclaimer } from "@/components/shared";

type Step = "welcome" | "safety" | "connect";

export default function Onboarding() {
  const { state, refresh } = useApp();
  const [step, setStep] = React.useState<Step>("welcome");
  const [settings, setSettings] = React.useState<AppSettings | null>(null);

  React.useEffect(() => {
    if (state && !settings) setSettings(state.settings);
  }, [state, settings]);

  const finish = useMutation({
    mutationFn: async (_conn: ConnectionDto | null) => {
      await api.setOnboarding(true, "done");
    },
    onSuccess: refresh,
  });

  const saveSafety = useMutation({
    mutationFn: async () => {
      if (settings) await api.updateSettings(settings);
    },
    onSuccess: () => setStep("connect"),
  });

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto flex min-h-full max-w-2xl flex-col justify-center px-6 py-12">
        <div className="mb-8 flex items-center gap-2">
          <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <PieChart className="h-5 w-5" aria-hidden />
          </div>
          <span className="text-lg font-semibold tracking-tight">PortfolioOS</span>
        </div>

        {step === "welcome" && (
          <div className="space-y-6">
            <div>
              <h1 className="text-2xl font-semibold tracking-tight">
                Investment automation that stays on your computer
              </h1>
              <p className="mt-2 text-muted-foreground">
                PortfolioOS helps you define transparent strategies, see exactly what trades they
                imply, and approve every order yourself.
              </p>
            </div>
            <ul className="space-y-4">
              <WelcomePoint icon={Lock} title="Local-first privacy">
                Everything runs on this computer. Brokerage credentials live in your operating
                system's credential manager and never leave your device.
              </WelcomePoint>
              <WelcomePoint icon={ShieldCheck} title="You approve every trade">
                Nothing is ever submitted automatically. Every batch of orders requires your
                explicit review and confirmation.
              </WelcomePoint>
              <WelcomePoint icon={Wallet} title="Paper trading first">
                Version 1 supports paper (simulated) trading only, so you can learn the workflow
                without risking money.
              </WelcomePoint>
            </ul>
            <Disclaimer />
            <Button size="lg" onClick={() => setStep("safety")}>
              Get started
            </Button>
          </div>
        )}

        {step === "safety" && settings && (
          <div className="space-y-6">
            <div>
              <h1 className="text-2xl font-semibold tracking-tight">Set your safety limits</h1>
              <p className="mt-2 text-muted-foreground">
                These guardrails apply to every rebalance. You can change them later in Settings.
              </p>
            </div>
            <div className="space-y-4">
              <div className="rounded-lg border border-border bg-card p-4">
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-sm font-medium">Require approval before every trade batch</p>
                    <p className="text-xs text-muted-foreground">Always on in this version.</p>
                  </div>
                  <Switch
                    checked
                    disabled
                    aria-label="Require approval before every trade batch (always on)"
                  />
                </div>
              </div>
              <div className="grid grid-cols-2 gap-4">
                <MoneyField
                  label="Maximum order value"
                  value={settings.safety.max_order_value}
                  onChange={(v) =>
                    setSettings({ ...settings, safety: { ...settings.safety, max_order_value: v } })
                  }
                />
                <MoneyField
                  label="Maximum batch value"
                  value={settings.safety.max_batch_value}
                  onChange={(v) =>
                    setSettings({ ...settings, safety: { ...settings.safety, max_batch_value: v } })
                  }
                />
                <MoneyField
                  label="Minimum order value"
                  value={settings.safety.min_order_value}
                  onChange={(v) =>
                    setSettings({ ...settings, safety: { ...settings.safety, min_order_value: v } })
                  }
                />
                <div className="space-y-1.5">
                  <Label htmlFor="ob-maxpos">Maximum position weight (%)</Label>
                  <Input
                    id="ob-maxpos"
                    type="number"
                    min={1}
                    max={100}
                    value={Math.round(settings.safety.max_position_weight * 100)}
                    onChange={(e) =>
                      setSettings({
                        ...settings,
                        safety: {
                          ...settings.safety,
                          max_position_weight: Number(e.target.value) / 100,
                        },
                      })
                    }
                  />
                </div>
              </div>
              <ToggleRow
                label="Allow selling"
                description="Rebalances may propose sell orders to reduce overweight positions."
                checked={settings.safety.allow_sells}
                onChange={(v) =>
                  setSettings({ ...settings, safety: { ...settings.safety, allow_sells: v } })
                }
              />
              <ToggleRow
                label="Allow fractional shares"
                description="Buy or sell partial shares where the brokerage supports it."
                checked={settings.safety.allow_fractional}
                onChange={(v) =>
                  setSettings({ ...settings, safety: { ...settings.safety, allow_fractional: v } })
                }
              />
            </div>
            <div className="flex gap-2">
              <Button variant="outline" onClick={() => setStep("welcome")}>
                Back
              </Button>
              <Button onClick={() => saveSafety.mutate()} disabled={saveSafety.isPending}>
                Continue
              </Button>
            </div>
          </div>
        )}

        {step === "connect" && (
          <div className="space-y-6">
            <div>
              <h1 className="text-2xl font-semibold tracking-tight">Choose how to begin</h1>
              <p className="mt-2 text-muted-foreground">
                Explore instantly with demo data, or connect a supported brokerage. You can add more
                later.
              </p>
            </div>
            <ConnectWizard onDone={(conn) => finish.mutate(conn)} />
          </div>
        )}
      </div>
    </div>
  );
}

function WelcomePoint({
  icon: Icon,
  title,
  children,
}: {
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <li className="flex gap-3">
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent">
        <Icon className="h-4 w-4 text-accent-foreground" aria-hidden />
      </div>
      <div>
        <p className="font-medium">{title}</p>
        <p className="text-sm text-muted-foreground">{children}</p>
      </div>
    </li>
  );
}

function MoneyField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
}) {
  const id = React.useId();
  return (
    <div className="space-y-1.5">
      <Label htmlFor={id}>{label} ($)</Label>
      <Input
        id={id}
        type="number"
        min={0}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
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
    <div className="flex items-center justify-between rounded-lg border border-border bg-card p-4">
      <div>
        <p className="text-sm font-medium">{label}</p>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>
      <Switch checked={checked} onCheckedChange={onChange} aria-label={label} />
    </div>
  );
}

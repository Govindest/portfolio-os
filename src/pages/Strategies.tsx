import * as React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useForm, useFieldArray } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { Download, FileUp, GraduationCap, Plus, Target, Trash2 } from "lucide-react";
import { api, type StrategyDoc, type StrategySummary } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { Skeleton } from "@/components/ui/skeleton";
import { ConfirmDialog, EmptyState, ErrorNote } from "@/components/shared";
import { weightPercent, formatDateTime, formatPercent } from "@/lib/utils";

const assetSchema = z.object({
  symbol: z
    .string()
    .trim()
    .min(1, "Symbol is required")
    .max(10)
    .regex(/^[A-Za-z][A-Za-z0-9.]*$/, "Letters, numbers, and dots only"),
  weight: z.coerce.number().positive("Must be positive").max(100, "Cannot exceed 100%"),
});

export const formSchema = z
  .object({
    name: z.string().trim().min(1, "Give the strategy a name").max(80),
    description: z.string().max(300).optional(),
    kind: z.enum(["equal_weight", "custom"]),
    driftThreshold: z.coerce.number().min(0).max(50),
    minOrderValue: z.coerce.number().min(0),
    assets: z.array(assetSchema).min(1, "Add at least one symbol"),
  })
  .superRefine((data, ctx) => {
    const symbols = data.assets.map((a) => a.symbol.toUpperCase());
    const dupes = symbols.filter((s, i) => symbols.indexOf(s) !== i);
    if (dupes.length > 0) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["assets"],
        message: `Duplicate symbol: ${dupes[0]}`,
      });
    }
    if (data.kind === "custom") {
      const total = data.assets.reduce((s, a) => s + a.weight, 0);
      if (Math.abs(total - 100) > 0.5) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ["assets"],
          message: `Weights add up to ${total.toFixed(1)}%. They must total 100%.`,
        });
      }
    }
  });

type FormValues = z.infer<typeof formSchema>;

export default function Strategies() {
  const qc = useQueryClient();
  const { data: strategies, isLoading } = useQuery({
    queryKey: ["strategies"],
    queryFn: api.listStrategies,
  });
  const [editorOpen, setEditorOpen] = React.useState(false);
  const [editing, setEditing] = React.useState<StrategySummary | null>(null);
  const [importOpen, setImportOpen] = React.useState(false);
  const [momentumOpen, setMomentumOpen] = React.useState(false);
  const [deleting, setDeleting] = React.useState<StrategySummary | null>(null);

  const del = useMutation({
    mutationFn: (id: string) => api.deleteStrategy(id),
    onSuccess: () => {
      setDeleting(null);
      void qc.invalidateQueries({ queryKey: ["strategies"] });
    },
  });

  const exportYaml = useMutation({ mutationFn: (id: string) => api.saveStrategyExport(id) });

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Strategies</h1>
          <p className="text-sm text-muted-foreground">
            Transparent target allocations. Every proposed trade traces back to these rules.
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setMomentumOpen(true)}>
            <GraduationCap aria-hidden /> Momentum example
          </Button>
          <Button variant="outline" onClick={() => setImportOpen(true)}>
            <FileUp aria-hidden /> Import YAML
          </Button>
          <Button
            onClick={() => {
              setEditing(null);
              setEditorOpen(true);
            }}
          >
            <Plus aria-hidden /> New strategy
          </Button>
        </div>
      </div>

      {exportYaml.isSuccess && (
        <p className="text-sm text-success" role="status">
          Exported to {exportYaml.data}
        </p>
      )}

      {isLoading ? (
        <Skeleton className="h-56" />
      ) : !strategies || strategies.length === 0 ? (
        <EmptyState
          icon={Target}
          title="No strategies yet"
          description="Create an equal-weight or custom allocation, or import a strategy YAML file."
          action={
            <Button
              onClick={() => {
                setEditing(null);
                setEditorOpen(true);
              }}
            >
              <Plus aria-hidden /> Create your first strategy
            </Button>
          }
        />
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {strategies.map((s) => (
            <Card key={s.id}>
              <CardHeader>
                <div className="flex items-start justify-between">
                  <div>
                    <CardTitle>{s.name}</CardTitle>
                    <CardDescription>
                      {s.description || `${s.asset_count} assets`} · v{s.current_version} · Updated{" "}
                      {formatDateTime(s.updated_at)}
                    </CardDescription>
                  </div>
                  <Badge variant="secondary">{s.kind.replace("_", " ")}</Badge>
                </div>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="flex flex-wrap gap-1.5">
                  {s.doc.assets.slice(0, 8).map((a) => (
                    <Badge key={a.symbol} variant="outline">
                      {a.symbol} {weightPercent(a.target_weight, 0)}
                    </Badge>
                  ))}
                  {s.doc.assets.length > 8 && (
                    <Badge variant="outline">+{s.doc.assets.length - 8}</Badge>
                  )}
                </div>
                <p className="text-xs text-muted-foreground">
                  Rebalances when drift exceeds{" "}
                  {formatPercent(s.doc.rebalance.drift_threshold_percent)} · Minimum order $
                  {s.doc.rebalance.minimum_order_value}
                </p>
                <div className="flex gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => {
                      setEditing(s);
                      setEditorOpen(true);
                    }}
                  >
                    Edit
                  </Button>
                  <Button variant="outline" size="sm" onClick={() => exportYaml.mutate(s.id)}>
                    <Download aria-hidden /> Export
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-destructive"
                    onClick={() => setDeleting(s)}
                  >
                    <Trash2 aria-hidden /> Archive
                  </Button>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      <StrategyEditor
        open={editorOpen}
        onOpenChange={setEditorOpen}
        editing={editing}
        onSaved={() => void qc.invalidateQueries({ queryKey: ["strategies"] })}
      />
      <ImportDialog
        open={importOpen}
        onOpenChange={setImportOpen}
        onImported={() => void qc.invalidateQueries({ queryKey: ["strategies"] })}
      />
      <MomentumDialog
        open={momentumOpen}
        onOpenChange={setMomentumOpen}
        onCreated={() => void qc.invalidateQueries({ queryKey: ["strategies"] })}
      />
      <ConfirmDialog
        open={deleting !== null}
        onOpenChange={(o) => !o && setDeleting(null)}
        title={`Archive “${deleting?.name}”?`}
        description="The strategy is hidden from lists but its history and audit trail are preserved."
        confirmLabel="Archive"
        destructive
        busy={del.isPending}
        onConfirm={() => deleting && del.mutate(deleting.id)}
      />
    </div>
  );
}

function StrategyEditor({
  open,
  onOpenChange,
  editing,
  onSaved,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  editing: StrategySummary | null;
  onSaved: () => void;
}) {
  const form = useForm<FormValues>({
    resolver: zodResolver(formSchema),
    defaultValues: defaultsFrom(editing),
  });
  const { fields, append, remove } = useFieldArray({ control: form.control, name: "assets" });
  const kind = form.watch("kind");
  const assets = form.watch("assets");

  React.useEffect(() => {
    form.reset(defaultsFrom(editing));
  }, [editing, open]); // eslint-disable-line react-hooks/exhaustive-deps

  const save = useMutation({
    mutationFn: (values: FormValues) => {
      const n = values.assets.length;
      const doc: StrategyDoc = {
        schema_version: 1,
        name: values.name,
        description: values.description || null,
        assets: values.assets.map((a) => ({
          symbol: a.symbol.toUpperCase(),
          target_weight: values.kind === "equal_weight" ? 1 / n : a.weight / 100,
        })),
        rebalance: {
          method: "target_weight",
          drift_threshold_percent: values.driftThreshold,
          minimum_order_value: values.minOrderValue,
          allow_fractional_shares: true,
        },
        risk_controls: {
          maximum_position_weight: Math.max(
            0.35,
            values.kind === "equal_weight"
              ? 1 / n + 0.05
              : Math.max(...values.assets.map((a) => a.weight / 100)) + 0.05,
          ),
          maximum_order_value: 1000,
          maximum_batch_value: 5000,
        },
      };
      return api.saveStrategy(values.kind, doc, editing?.id);
    },
    onSuccess: () => {
      onSaved();
      onOpenChange(false);
    },
  });

  const totalWeight = assets?.reduce((s, a) => s + (Number(a.weight) || 0), 0) ?? 0;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>{editing ? "Edit strategy" : "New strategy"}</DialogTitle>
          <DialogDescription>
            Define target weights. PortfolioOS only ever proposes trades toward these targets — it
            never trades without your approval.
          </DialogDescription>
        </DialogHeader>
        <form className="space-y-4" onSubmit={form.handleSubmit((v) => save.mutate(v))} noValidate>
          <div className="space-y-1.5">
            <Label htmlFor="st-name">Name</Label>
            <Input id="st-name" {...form.register("name")} placeholder="Core index mix" />
            <FieldError message={form.formState.errors.name?.message} />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="st-desc">Description (optional)</Label>
            <Input id="st-desc" {...form.register("description")} />
          </div>
          <Tabs
            value={kind}
            onValueChange={(v) => form.setValue("kind", v as "equal_weight" | "custom")}
          >
            <TabsList>
              <TabsTrigger value="equal_weight">Equal weight</TabsTrigger>
              <TabsTrigger value="custom">Custom allocation</TabsTrigger>
            </TabsList>
            <TabsContent value="equal_weight" className="text-sm text-muted-foreground">
              Every symbol receives the same target weight
              {fields.length > 0 && <> ({weightPercent(1 / fields.length)} each)</>}.
            </TabsContent>
            <TabsContent value="custom" className="text-sm text-muted-foreground">
              Set an exact percentage per symbol. Weights must total 100%. Current total:{" "}
              <span
                className={
                  Math.abs(totalWeight - 100) > 0.5
                    ? "font-medium text-warning"
                    : "font-medium text-success"
                }
              >
                {totalWeight.toFixed(1)}%
              </span>
            </TabsContent>
          </Tabs>

          <div className="space-y-2">
            {fields.map((field, i) => (
              <div key={field.id} className="flex items-start gap-2">
                <div className="flex-1">
                  <Input
                    {...form.register(`assets.${i}.symbol`)}
                    placeholder="Symbol (e.g. VOO)"
                    aria-label={`Symbol ${i + 1}`}
                    className="uppercase"
                  />
                  <FieldError message={form.formState.errors.assets?.[i]?.symbol?.message} />
                </div>
                {kind === "custom" && (
                  <div className="w-28">
                    <Input
                      {...form.register(`assets.${i}.weight`)}
                      type="number"
                      step="any"
                      min={0}
                      max={100}
                      placeholder="%"
                      aria-label={`Weight ${i + 1} percent`}
                    />
                    <FieldError message={form.formState.errors.assets?.[i]?.weight?.message} />
                  </div>
                )}
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Remove symbol ${i + 1}`}
                  onClick={() => remove(i)}
                  disabled={fields.length <= 1}
                >
                  <Trash2 />
                </Button>
              </div>
            ))}
            <FieldError
              message={
                form.formState.errors.assets?.message ?? form.formState.errors.assets?.root?.message
              }
            />
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => append({ symbol: "", weight: 0 })}
            >
              <Plus aria-hidden /> Add symbol
            </Button>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <Label htmlFor="st-drift">Drift threshold (%)</Label>
              <Input
                id="st-drift"
                type="number"
                step="any"
                min={0}
                max={50}
                {...form.register("driftThreshold")}
              />
              <FieldError message={form.formState.errors.driftThreshold?.message} />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="st-minorder">Minimum order value ($)</Label>
              <Input
                id="st-minorder"
                type="number"
                step="any"
                min={0}
                {...form.register("minOrderValue")}
              />
            </div>
          </div>

          {save.isError && <ErrorNote error={save.error} />}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={save.isPending}>
              {editing ? "Save changes" : "Create strategy"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function defaultsFrom(editing: StrategySummary | null): FormValues {
  if (!editing) {
    return {
      name: "",
      description: "",
      kind: "equal_weight",
      driftThreshold: 2,
      minOrderValue: 5,
      assets: [{ symbol: "", weight: 0 }],
    };
  }
  return {
    name: editing.name,
    description: editing.description ?? "",
    kind: editing.kind === "custom" ? "custom" : "equal_weight",
    driftThreshold: editing.doc.rebalance.drift_threshold_percent,
    minOrderValue: editing.doc.rebalance.minimum_order_value,
    assets: editing.doc.assets.map((a) => ({ symbol: a.symbol, weight: a.target_weight * 100 })),
  };
}

function FieldError({ message }: { message?: string }) {
  if (!message) return null;
  return (
    <p className="mt-1 text-xs text-destructive" role="alert">
      {message}
    </p>
  );
}

function ImportDialog({
  open,
  onOpenChange,
  onImported,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  onImported: () => void;
}) {
  const [yaml, setYaml] = React.useState("");
  const importMut = useMutation({
    mutationFn: () => api.importStrategyYaml(yaml),
    onSuccess: () => {
      onImported();
      onOpenChange(false);
      setYaml("");
    },
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>Import a strategy</DialogTitle>
          <DialogDescription>
            Paste strategy YAML or choose a file. Strategies are data only — imported files are
            validated and never executed.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="flex gap-2">
            <label className="cursor-pointer">
              <input
                type="file"
                accept=".yaml,.yml,text/yaml"
                className="sr-only"
                onChange={async (e) => {
                  const f = e.target.files?.[0];
                  if (f) setYaml(await f.text());
                  e.target.value = "";
                }}
              />
              <span className="inline-flex h-8 items-center gap-2 rounded-md border border-border px-3 text-xs font-medium hover:bg-muted">
                <FileUp className="h-3.5 w-3.5" aria-hidden /> Choose file
              </span>
            </label>
            <Button
              variant="outline"
              size="sm"
              onClick={async () => setYaml(await api.exampleStrategyYaml())}
            >
              Load example
            </Button>
          </div>
          <Textarea
            value={yaml}
            onChange={(e) => setYaml(e.target.value)}
            rows={14}
            placeholder="schema_version: 1&#10;name: My Strategy&#10;assets:&#10;  - symbol: VOO&#10;    target_weight: 0.6"
            aria-label="Strategy YAML"
          />
          {importMut.isError && <ErrorNote error={importMut.error} />}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={() => importMut.mutate()} disabled={!yaml.trim() || importMut.isPending}>
            Validate and import
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function MomentumDialog({
  open,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  onCreated: () => void;
}) {
  const { data: rows } = useQuery({
    queryKey: ["momentum-demo"],
    queryFn: api.getMomentumDemo,
    enabled: open,
  });

  const create = useMutation({
    mutationFn: () => {
      const selected = (rows ?? []).filter((r) => r.selected);
      const doc: StrategyDoc = {
        schema_version: 1,
        name: "Momentum Demo",
        description: "Educational example built from bundled sample data — not investment advice",
        assets: selected.map((r) => ({ symbol: r.symbol, target_weight: 1 / selected.length })),
        rebalance: {
          method: "target_weight",
          drift_threshold_percent: 2,
          minimum_order_value: 5,
          allow_fractional_shares: true,
        },
        risk_controls: {
          maximum_position_weight: 1 / Math.max(1, selected.length) + 0.05,
          maximum_order_value: 1000,
          maximum_batch_value: 5000,
        },
      };
      return api.saveStrategy("momentum_demo", doc);
    },
    onSuccess: () => {
      onCreated();
      onOpenChange(false);
    },
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>Momentum demo</DialogTitle>
          <DialogDescription>
            Educational example, not investment advice. It ranks a fixed sample universe by bundled
            historical sample returns and equal-weights the top half. Past performance — especially
            sample data — does not predict future results.
          </DialogDescription>
        </DialogHeader>
        <div className="rounded-md border border-border">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                <th className="px-3 py-2">Symbol</th>
                <th className="px-3 py-2 text-right">Sample 6-month return</th>
                <th className="px-3 py-2 text-right">Included</th>
              </tr>
            </thead>
            <tbody>
              {(rows ?? []).map((r) => (
                <tr key={r.symbol} className="border-b border-border last:border-0">
                  <td className="px-3 py-1.5 font-medium">{r.symbol}</td>
                  <td className="px-3 py-1.5 text-right tabular-nums">
                    {r.sample_return_percent >= 0 ? "+" : ""}
                    {r.sample_return_percent.toFixed(1)}%
                  </td>
                  <td className="px-3 py-1.5 text-right">
                    {r.selected ? (
                      <Badge variant="success">Selected</Badge>
                    ) : (
                      <span className="text-muted-foreground">—</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {create.isError && <ErrorNote error={create.error} />}
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
          <Button onClick={() => create.mutate()} disabled={!rows || create.isPending}>
            Create demo strategy
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

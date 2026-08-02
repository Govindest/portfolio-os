import * as React from "react";
import { useQuery } from "@tanstack/react-query";
import { Activity as ActivityIcon, ChevronDown } from "lucide-react";
import { api } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Select } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/shared";
import { cn, formatDateTime } from "@/lib/utils";

const TYPE_LABELS: Record<string, string> = {
  connection_created: "Connection",
  connection_removed: "Connection",
  sync_completed: "Sync",
  settings_updated: "Settings",
  strategy_created: "Strategy",
  strategy_updated: "Strategy",
  strategy_archived: "Strategy",
  rebalance_calculated: "Rebalance",
  orders_approved: "Approval",
  order_submitted: "Order",
  order_canceled: "Order",
  positions_imported: "Import",
  demo_reset: "Demo",
  emergency_stop_enabled: "Safety",
  emergency_stop_disabled: "Safety",
};

const FILTERS = [
  { value: "", label: "All activity" },
  { value: "order_submitted", label: "Orders" },
  { value: "rebalance_calculated", label: "Rebalances" },
  { value: "sync_completed", label: "Syncs" },
  { value: "strategy_created", label: "Strategies" },
];

export default function ActivityPage() {
  const [filter, setFilter] = React.useState("");
  const { data: events, isLoading } = useQuery({
    queryKey: ["activity", filter],
    queryFn: () => api.listActivity(200, filter || undefined),
    refetchInterval: 10_000,
  });

  return (
    <div className="space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Activity</h1>
          <p className="text-sm text-muted-foreground">
            The complete local audit trail: every connection, sync, calculation, approval, and
            order.
          </p>
        </div>
        <Select
          aria-label="Filter activity"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="w-44"
        >
          {FILTERS.map((f) => (
            <option key={f.value} value={f.value}>
              {f.label}
            </option>
          ))}
        </Select>
      </div>

      {isLoading ? (
        <Skeleton className="h-64" />
      ) : !events || events.length === 0 ? (
        <EmptyState
          icon={ActivityIcon}
          title="No activity yet"
          description="Actions you take in PortfolioOS are recorded here."
        />
      ) : (
        <ul className="space-y-2">
          {events.map((e) => (
            <EventRow
              key={e.id}
              type={e.event_type}
              description={e.description}
              at={e.created_at}
              technical={e.technical}
            />
          ))}
        </ul>
      )}
    </div>
  );
}

function EventRow({
  type,
  description,
  at,
  technical,
}: {
  type: string;
  description: string;
  at: string;
  technical: unknown;
}) {
  const [open, setOpen] = React.useState(false);
  const hasTech = technical !== null && technical !== undefined;
  return (
    <li className="rounded-lg border border-border bg-card px-4 py-3">
      <div className="flex items-center gap-3">
        <Badge variant="secondary" className="w-24 justify-center">
          {TYPE_LABELS[type] ?? "Event"}
        </Badge>
        <p className="min-w-0 flex-1 truncate text-sm">{description}</p>
        <time className="shrink-0 text-xs text-muted-foreground">{formatDateTime(at)}</time>
        {hasTech && (
          <button
            type="button"
            aria-label="Toggle technical details"
            onClick={() => setOpen((o) => !o)}
            className="cursor-pointer text-muted-foreground hover:text-foreground"
          >
            <ChevronDown
              className={cn("h-4 w-4 transition-transform", open && "rotate-180")}
              aria-hidden
            />
          </button>
        )}
      </div>
      {hasTech && open && (
        <pre className="mt-2 overflow-x-auto rounded bg-muted p-2 text-xs text-muted-foreground">
          {JSON.stringify(technical, null, 2)}
        </pre>
      )}
    </li>
  );
}

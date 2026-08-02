import * as React from "react";
import { AlertTriangle, CheckCircle2, ChevronDown, Circle, Inbox, XCircle } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { errorMessage, errorTechnical } from "@/lib/api";
import { cn } from "@/lib/utils";

/** Consistent empty state used across pages. */
export function EmptyState({
  icon: Icon = Inbox,
  title,
  description,
  action,
}: {
  icon?: React.ComponentType<{ className?: string }>;
  title: string;
  description?: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border py-14 px-6 text-center">
      <Icon className="h-8 w-8 text-muted-foreground/60" aria-hidden />
      <p className="font-medium">{title}</p>
      {description && <p className="max-w-md text-sm text-muted-foreground">{description}</p>}
      {action && <div className="mt-3">{action}</div>}
    </div>
  );
}

/** Order / connection status pill. Icon + text — never color alone. */
export function StatusBadge({ status }: { status: string }) {
  const map: Record<
    string,
    {
      variant: "success" | "warning" | "destructive" | "secondary" | "accent";
      icon: React.ReactNode;
      label: string;
    }
  > = {
    filled: {
      variant: "success",
      icon: <CheckCircle2 className="h-3 w-3" aria-hidden />,
      label: "Filled",
    },
    connected: {
      variant: "success",
      icon: <CheckCircle2 className="h-3 w-3" aria-hidden />,
      label: "Connected",
    },
    accepted: {
      variant: "accent",
      icon: <Circle className="h-3 w-3" aria-hidden />,
      label: "Accepted",
    },
    pending: {
      variant: "warning",
      icon: <Circle className="h-3 w-3" aria-hidden />,
      label: "Pending",
    },
    partially_filled: {
      variant: "warning",
      icon: <Circle className="h-3 w-3" aria-hidden />,
      label: "Partially filled",
    },
    rejected: {
      variant: "destructive",
      icon: <XCircle className="h-3 w-3" aria-hidden />,
      label: "Rejected",
    },
    error: {
      variant: "destructive",
      icon: <XCircle className="h-3 w-3" aria-hidden />,
      label: "Error",
    },
    canceled: {
      variant: "secondary",
      icon: <XCircle className="h-3 w-3" aria-hidden />,
      label: "Canceled",
    },
  };
  const entry = map[status] ?? { variant: "secondary" as const, icon: null, label: status };
  return (
    <Badge variant={entry.variant}>
      {entry.icon}
      {entry.label}
    </Badge>
  );
}

/** Friendly error with expandable technical detail. */
export function ErrorNote({ error, className }: { error: unknown; className?: string }) {
  const [open, setOpen] = React.useState(false);
  const technical = errorTechnical(error);
  return (
    <div
      role="alert"
      className={cn(
        "rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm",
        className,
      )}
    >
      <div className="flex items-start gap-2">
        <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-destructive" aria-hidden />
        <div className="min-w-0 flex-1">
          <p>{errorMessage(error)}</p>
          {technical && (
            <button
              type="button"
              onClick={() => setOpen((o) => !o)}
              className="mt-1 inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground cursor-pointer"
            >
              <ChevronDown
                className={cn("h-3 w-3 transition-transform", open && "rotate-180")}
                aria-hidden
              />
              Technical details
            </button>
          )}
          {technical && open && (
            <pre className="mt-1 overflow-x-auto rounded bg-muted p-2 text-xs text-muted-foreground">
              {technical}
            </pre>
          )}
        </div>
      </div>
    </div>
  );
}

/** Standard confirmation dialog. */
export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel = "Confirm",
  destructive,
  onConfirm,
  busy,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  confirmLabel?: string;
  destructive?: boolean;
  onConfirm: () => void;
  busy?: boolean;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={busy}>
            Cancel
          </Button>
          <Button
            variant={destructive ? "destructive" : "default"}
            onClick={onConfirm}
            disabled={busy}
          >
            {busy ? "Working…" : confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function PaperBadge() {
  return <Badge variant="accent">Paper trading</Badge>;
}

/** Legal footer text used on onboarding and settings. */
export function Disclaimer({ className }: { className?: string }) {
  return (
    <p className={cn("text-xs leading-relaxed text-muted-foreground", className)}>
      PortfolioOS is open-source software. It is not a broker-dealer or investment adviser and does
      not provide individualized investment advice. Trading involves risk. You are responsible for
      reviewing every order before it is submitted. Brokerage data may be delayed or unavailable.
      Paper-trading results do not represent actual performance.
    </p>
  );
}

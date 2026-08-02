import * as React from "react";
import { cn } from "@/lib/utils";

/** Native select, styled. ponytail: platform control over a custom dropdown. */
const Select = React.forwardRef<HTMLSelectElement, React.SelectHTMLAttributes<HTMLSelectElement>>(
  ({ className, ...props }, ref) => (
    <select
      ref={ref}
      className={cn(
        "flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm cursor-pointer focus-visible:outline-2 focus-visible:outline-ring disabled:cursor-not-allowed disabled:opacity-50 dark:bg-card",
        className,
      )}
      {...props}
    />
  ),
);
Select.displayName = "Select";

export { Select };

import { describe, expect, it } from "vitest";
import { formSchema } from "@/pages/Strategies";

const base = {
  name: "Test",
  description: "",
  kind: "custom" as const,
  driftThreshold: 2,
  minOrderValue: 5,
  assets: [
    { symbol: "AAPL", weight: 50 },
    { symbol: "MSFT", weight: 50 },
  ],
};

describe("strategy form validation", () => {
  it("accepts a valid custom allocation", () => {
    expect(formSchema.safeParse(base).success).toBe(true);
  });

  it("rejects an empty name", () => {
    const r = formSchema.safeParse({ ...base, name: "  " });
    expect(r.success).toBe(false);
  });

  it("rejects duplicate symbols (case-insensitive)", () => {
    const r = formSchema.safeParse({
      ...base,
      assets: [
        { symbol: "aapl", weight: 50 },
        { symbol: "AAPL", weight: 50 },
      ],
    });
    expect(r.success).toBe(false);
    expect(JSON.stringify(r.error?.issues)).toContain("Duplicate");
  });

  it("rejects custom weights that do not total 100%", () => {
    const r = formSchema.safeParse({
      ...base,
      assets: [
        { symbol: "AAPL", weight: 40 },
        { symbol: "MSFT", weight: 40 },
      ],
    });
    expect(r.success).toBe(false);
    expect(JSON.stringify(r.error?.issues)).toContain("must total 100%");
  });

  it("accepts equal-weight strategies whose weights are still zero", () => {
    // Regression: the editor never renders a weight input in equal-weight mode,
    // so appended rows keep weight 0. A field-level positive() check here made
    // the submit button silently do nothing.
    const r = formSchema.safeParse({
      ...base,
      kind: "equal_weight",
      assets: [
        { symbol: "AAPL", weight: 0 },
        { symbol: "MSFT", weight: 0 },
        { symbol: "VOO", weight: 0 },
      ],
    });
    expect(r.success).toBe(true);
  });

  it("ignores weight totals for equal-weight strategies", () => {
    const r = formSchema.safeParse({
      ...base,
      kind: "equal_weight",
      assets: [
        { symbol: "AAPL", weight: 1 },
        { symbol: "MSFT", weight: 1 },
      ],
    });
    expect(r.success).toBe(true);
  });

  it("rejects invalid ticker characters", () => {
    const r = formSchema.safeParse({
      ...base,
      assets: [
        { symbol: "AA PL", weight: 50 },
        { symbol: "MSFT", weight: 50 },
      ],
    });
    expect(r.success).toBe(false);
  });

  it("rejects zero or negative weights in custom allocations", () => {
    const zero = formSchema.safeParse({
      ...base,
      assets: [
        { symbol: "AAPL", weight: 0 },
        { symbol: "MSFT", weight: 100 },
      ],
    });
    expect(zero.success).toBe(false);
    expect(JSON.stringify(zero.error?.issues)).toContain("greater than 0");

    const negative = formSchema.safeParse({
      ...base,
      assets: [
        { symbol: "AAPL", weight: -10 },
        { symbol: "MSFT", weight: 110 },
      ],
    });
    expect(negative.success).toBe(false);
  });
});

import { describe, expect, it } from "vitest";
import {
  formatMoney,
  formatPercent,
  formatQty,
  maskAccountId,
  timeAgo,
  weightPercent,
} from "@/lib/utils";

describe("formatters", () => {
  it("formats money", () => {
    expect(formatMoney(1234.5)).toBe("$1,234.50");
    expect(formatMoney(null)).toBe("—");
    expect(formatMoney(undefined)).toBe("—");
  });

  it("formats percent and weights", () => {
    expect(formatPercent(3.456)).toBe("3.5%");
    expect(formatPercent(null)).toBe("—");
    expect(weightPercent(0.25)).toBe("25.0%");
  });

  it("formats fractional quantities", () => {
    expect(formatQty(1.2345)).toBe("1.2345");
    expect(formatQty(10)).toBe("10");
  });

  it("masks account ids", () => {
    expect(maskAccountId("PA1234567")).toBe("••••4567");
    expect(maskAccountId("AB")).toBe("AB");
  });

  it("reports time ago", () => {
    expect(timeAgo(null)).toBe("never");
    expect(timeAgo(new Date().toISOString())).toBe("just now");
    expect(timeAgo(new Date(Date.now() - 3 * 60_000).toISOString())).toBe("3 minutes ago");
  });
});

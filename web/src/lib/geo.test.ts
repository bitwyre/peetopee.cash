import { describe, it, expect } from "vitest";
import { haversineKm, formatFiatChip, COARSEN_RADIUS_M } from "./geo";

describe("haversineKm", () => {
  it("is ~0 for identical points", () => {
    expect(haversineKm({ lat: 0, lng: 0 }, { lat: 0, lng: 0 })).toBeCloseTo(0, 5);
  });
  it("matches a known city pair (Jakarta↔Bali ~ 950km)", () => {
    const km = haversineKm({ lat: -6.2088, lng: 106.8456 }, { lat: -8.6705, lng: 115.2126 });
    expect(km).toBeGreaterThan(900);
    expect(km).toBeLessThan(1000);
  });
});

describe("formatFiatChip", () => {
  it("compacts large IDR", () => {
    expect(formatFiatChip("2100000", "IDR")).toBe("2.1M IDR");
  });
  it("keeps small EUR readable", () => {
    expect(formatFiatChip("50", "EUR")).toBe("50 EUR");
  });
});

describe("constants", () => {
  it("coarsen radius is 500m", () => {
    expect(COARSEN_RADIUS_M).toBe(500);
  });
});

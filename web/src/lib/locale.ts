import type { Currency } from "./types";
import type { LatLng } from "./geo";

// Map an ISO-3166 region (country) code to the supported fiat currency.
// Only the currencies in CURRENCIES are reachable; everything else falls back to USD.
const REGION_CURRENCY: Record<string, Currency> = {
  ID: "IDR",
  AE: "AED",
  GB: "GBP",
  RU: "RUB",
  IN: "INR",
  US: "USD",
  CA: "CAD",
  // Eurozone
  DE: "EUR", FR: "EUR", ES: "EUR", IT: "EUR", NL: "EUR", IE: "EUR", PT: "EUR",
  AT: "EUR", BE: "EUR", FI: "EUR", GR: "EUR", LU: "EUR", SK: "EUR", SI: "EUR",
  EE: "EUR", LV: "EUR", LT: "EUR", CY: "EUR", MT: "EUR", HR: "EUR",
};

export type MapView = { center: LatLng; zoom: number };

// A sensible opening map view per region, so a visitor who hasn't granted
// location still lands somewhere familiar instead of mid-ocean.
const REGION_VIEW: Record<string, MapView> = {
  ID: { center: { lat: -6.21, lng: 106.85 }, zoom: 11 }, // Jakarta
  AE: { center: { lat: 25.2, lng: 55.27 }, zoom: 11 }, // Dubai
  GB: { center: { lat: 51.51, lng: -0.12 }, zoom: 10 }, // London
  RU: { center: { lat: 55.75, lng: 37.62 }, zoom: 10 }, // Moscow
  IN: { center: { lat: 28.61, lng: 77.21 }, zoom: 10 }, // Delhi
  US: { center: { lat: 39.5, lng: -98.35 }, zoom: 4 }, // continental US
  CA: { center: { lat: 43.65, lng: -79.38 }, zoom: 9 }, // Toronto
  EUR: { center: { lat: 50.11, lng: 8.68 }, zoom: 5 }, // Frankfurt / central Europe
};

// A gentle world view — better than staring at the equator at zoom 2.
export const DEFAULT_VIEW: MapView = { center: { lat: 25, lng: 15 }, zoom: 3 };

/** Best-effort ISO region code from the browser locale (e.g. "en-US" -> "US"). No network. */
export function detectRegion(): string | null {
  if (typeof navigator === "undefined") return null;
  const langs = [navigator.language, ...(navigator.languages ?? [])].filter(Boolean);
  for (const loc of langs) {
    const m = loc.match(/[-_]([A-Za-z]{2})(?:[-_]|$)/);
    if (m) return m[1].toUpperCase();
  }
  // Fall back to maximizing the base language (e.g. "en" -> region "US").
  try {
    for (const loc of langs) {
      const region = new Intl.Locale(loc).maximize().region;
      if (region) return region.toUpperCase();
    }
  } catch {
    /* Intl.Locale unsupported — ignore */
  }
  return null;
}

/** Locale-derived default currency, falling back to USD. */
export function detectCurrency(): Currency {
  const region = detectRegion();
  return (region && REGION_CURRENCY[region]) || "USD";
}

/** Locale-derived opening map view, so the board isn't a blank globe pre-permission. */
export function detectMapView(): MapView {
  const region = detectRegion();
  if (region && REGION_VIEW[region]) return REGION_VIEW[region];
  if (region && REGION_CURRENCY[region] === "EUR") return REGION_VIEW.EUR;
  return DEFAULT_VIEW;
}

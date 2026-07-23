export type LatLng = { lat: number; lng: number };

export const COARSEN_RADIUS_M = 500;

/** Great-circle distance in kilometres. */
export function haversineKm(a: LatLng, b: LatLng): number {
  const R = 6371;
  const dLat = ((b.lat - a.lat) * Math.PI) / 180;
  const dLng = ((b.lng - a.lng) * Math.PI) / 180;
  const s =
    Math.sin(dLat / 2) ** 2 +
    Math.cos((a.lat * Math.PI) / 180) * Math.cos((b.lat * Math.PI) / 180) * Math.sin(dLng / 2) ** 2;
  return 2 * R * Math.asin(Math.sqrt(s));
}

/** Compact money label for a map pin, e.g. "2.1M IDR", "50 EUR". */
export function formatFiatChip(fiatAmount: string, currency: string): string {
  const n = parseFloat(fiatAmount);
  if (!isFinite(n)) return `— ${currency}`;
  const compact = new Intl.NumberFormat("en", {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(n);
  return `${compact} ${currency}`;
}

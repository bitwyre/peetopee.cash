# Map-first Courier Board Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the list-based `/courier` route with a full-bleed map of open cash-delivery orders plus a draggable bottom sheet, ride-hailing style.

**Architecture:** Frontend rewrite of one Next.js route into two new client components (`CourierMap`, `OrderSheet`) coordinated by a thin container page, reusing the existing `react-leaflet` stack and `/orders/open` polling. One small backend change tightens the existing open-order location coarsening from ~1 km to ~500 m.

**Tech Stack:** Next.js 15 (App Router) · React 19 · TypeScript (strict) · Tailwind v4 · react-leaflet 5 + leaflet 1.9 (OSM tiles) · Rust (Axum + SQLx) backend · vitest (new dev dep, frontend unit tests).

## Global Constraints

- Frontend HTTP goes through `api<T>(path, init?)` in `web/src/lib/api.ts`, which prefixes `/api` and sends `credentials: same-origin`. Never call `fetch` directly.
- Accept endpoint: `POST /api/orders/{id}/accept` (call as `api(\`/orders/${id}/accept\`, { method: "POST" })`).
- Open orders come from `GET /api/orders/open` → `Order[]` (type in `web/src/lib/types.ts`); poll every **10000 ms** as the current page does.
- Leaflet needs `window`; any component importing `react-leaflet`/`leaflet` must be loaded with `dynamic(() => import(...), { ssr: false })` and marked `"use client"`.
- Coarsening grid = **0.005°** (~500 m). Formula: `(x * 200.0).round() / 200.0`.
- Circle radius drawn on the map = **500 meters** (`COARSEN_RADIUS_M = 500`).
- Marker icons use `L.divIcon` (never Leaflet's default image markers — they break under the bundler), following the pattern in `web/src/components/MapPicker.tsx`.
- Theme: dark, `zinc` neutrals + `emerald` accent, matching existing components.
- Do NOT modify: landing page, order creation, order detail, settings, `layout.tsx`, `Nav.tsx`, the API schema, or any endpoint signature.
- Nav is ~49px tall (non-fixed, first in `<body>`); the map container escapes `<main>`'s `max-w-3xl`/padding via `fixed`.

---

## File Structure

- **Modify** `api/src/orders/mod.rs` — tighten `redact_location` to the 0.005° grid; add an in-file unit test.
- **Modify** `api/tests/orders_test.rs` — update the one coarsened-coordinate assertion.
- **Create** `web/src/lib/geo.ts` — pure helpers: `haversineKm`, `formatFiatChip`, `COARSEN_RADIUS_M`.
- **Create** `web/src/lib/geo.test.ts` — vitest unit tests for the pure helpers.
- **Modify** `web/package.json` — add `vitest` dev dep + `test` script.
- **Create** `web/src/components/CourierMap.tsx` — the Leaflet map (markers + circles + you-are-here + selection sync).
- **Create** `web/src/components/OrderSheet.tsx` — the draggable bottom sheet / desktop side panel with order cards + Accept.
- **Modify** `web/src/app/courier/page.tsx` — thin container wiring map + sheet, owning selection + data + geolocation.
- **Modify** `web/src/components/OrderCard.tsx` — none required; `impliedRate` stays exported here and is imported by the sheet.

---

## Task 1: Tighten open-order location coarsening to ~500 m

**Files:**
- Modify: `api/src/orders/mod.rs` (function `redact_location`, ~line 24)
- Modify: `api/tests/orders_test.rs` (assertion at ~line 109)
- Test: in-file `#[cfg(test)] mod tests` in `api/src/orders/mod.rs`

**Interfaces:**
- Consumes: existing `Order` struct (`api/src/orders/model.rs`), `redact_location(order: Order) -> Order`.
- Produces: `redact_location` now snaps `lat`/`lng` to the nearest `0.005°` and still blanks `address_text`. No signature change.

- [ ] **Step 1: Update the failing integration assertion first (documents new behavior)**

In `api/tests/orders_test.rs`, the test `open_order_detail_hides_private_fields` currently asserts the ~1 km grid. The seed order is `lat: -8.6705, lng: 115.2126`. Under the 0.005° grid: `-8.6705 → -8.67` (unchanged) and `115.2126 → 115.215`. Change:

```rust
    assert_eq!(v["address_text"], "");
    assert_eq!(v["lat"], -8.67);
    assert_eq!(v["lng"], 115.215);
```

- [ ] **Step 2: Add an in-file unit test for the grid math**

Append to the end of `api/src/orders/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn sample(lat: f64, lng: f64) -> Order {
        Order {
            id: Uuid::nil(),
            customer_id: Uuid::nil(),
            courier_id: None,
            fiat_currency: "IDR".into(),
            fiat_amount: Decimal::ZERO,
            usdt_amount: Decimal::ZERO,
            address_text: "Jl. Sunset Road 99, Kuta".into(),
            lat,
            lng,
            status: "OPEN".into(),
            payment_network: None,
            payment_txid: None,
            payment_requested_at: None,
            paid_at: None,
            created_at: Utc::now(),
            accepted_at: None,
            completed_at: None,
            cancelled_at: None,
        }
    }

    #[test]
    fn redact_snaps_to_500m_grid_and_blanks_address() {
        let r = redact_location(sample(-8.6705, 115.2126));
        assert_eq!(r.lat, -8.67);
        assert_eq!(r.lng, 115.215);
        assert_eq!(r.address_text, "");
    }

    #[test]
    fn redact_grid_is_finer_than_one_decimal() {
        // A point ~600m east must land on a different grid cell than the origin.
        let a = redact_location(sample(0.0, 0.0));
        let b = redact_location(sample(0.0, 0.0055));
        assert_ne!(a.lng, b.lng);
    }
}
```

- [ ] **Step 3: Run the new unit test to verify it fails**

Run: `cd api && cargo test --lib orders::tests -- --nocapture`
Expected: FAIL — `redact_grid_...` / assertions fail because `redact_location` still rounds to 2 decimals (`115.2126 → 115.21`, not `115.215`).

- [ ] **Step 4: Implement the 500 m grid**

In `api/src/orders/mod.rs`, change `redact_location`:

```rust
/// Coarsen an order for non-party viewers: ~500m grid, no street address.
fn redact_location(mut order: Order) -> Order {
    order.address_text = String::new();
    order.lat = (order.lat * 200.0).round() / 200.0;
    order.lng = (order.lng * 200.0).round() / 200.0;
    order
}
```

- [ ] **Step 5: Run the unit tests to verify they pass**

Run: `cd api && cargo test --lib orders::tests`
Expected: PASS (2 tests).

- [ ] **Step 6: Run the affected integration tests**

Run: `cd api && cargo test --test orders_test`
Expected: PASS (requires a test Postgres per the repo's `#[sqlx::test]` setup; if the DB is unavailable, note it and rely on Step 5 + reviewer running the suite).

- [ ] **Step 7: Commit**

```bash
git add api/src/orders/mod.rs api/tests/orders_test.rs
git commit -m "feat(api): tighten open-order coarsening to ~500m grid"
```

---

## Task 2: Frontend geo helpers + vitest

**Files:**
- Create: `web/src/lib/geo.ts`
- Create: `web/src/lib/geo.test.ts`
- Modify: `web/package.json`

**Interfaces:**
- Produces:
  - `export type LatLng = { lat: number; lng: number }`
  - `export function haversineKm(a: LatLng, b: LatLng): number`
  - `export function formatFiatChip(fiatAmount: string, currency: string): string` — compact money for map pins, e.g. `"1.5M IDR"`, `"50 EUR"`.
  - `export const COARSEN_RADIUS_M = 500`

- [ ] **Step 1: Add vitest to the web project**

In `web/package.json`, add to `devDependencies` (keep existing entries):

```json
    "vitest": "^2.1.0"
```

And add to `scripts`:

```json
    "test": "vitest run"
```

Then run: `cd web && npm install`
Expected: installs vitest without touching runtime deps.

- [ ] **Step 2: Write the failing tests**

Create `web/src/lib/geo.test.ts`:

```ts
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
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd web && npm test`
Expected: FAIL — `./geo` cannot be resolved (module not created yet).

- [ ] **Step 4: Implement the helpers**

Create `web/src/lib/geo.ts`:

```ts
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
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd web && npm test`
Expected: PASS (all describe blocks green).

- [ ] **Step 6: Commit**

```bash
git add web/src/lib/geo.ts web/src/lib/geo.test.ts web/package.json web/package-lock.json
git commit -m "feat(web): add geo helpers (haversineKm, formatFiatChip) + vitest"
```

---

## Task 3: CourierMap component

**Files:**
- Create: `web/src/components/CourierMap.tsx`

**Interfaces:**
- Consumes: `Order` (`@/lib/types`), `LatLng` / `formatFiatChip` / `COARSEN_RADIUS_M` (`@/lib/geo`).
- Produces:
  - `export default function CourierMap(props: CourierMapProps)`
  - `interface CourierMapProps { orders: Order[]; me: LatLng | null; selectedId: string | null; onSelect: (id: string | null) => void }`

- [ ] **Step 1: Implement the map component**

Create `web/src/components/CourierMap.tsx`:

```tsx
"use client";

import { useEffect, useMemo } from "react";
import { MapContainer, TileLayer, Marker, Circle, useMap } from "react-leaflet";
import "leaflet/dist/leaflet.css";
import L from "leaflet";
import type { Order } from "@/lib/types";
import { COARSEN_RADIUS_M, formatFiatChip, type LatLng } from "@/lib/geo";

interface CourierMapProps {
  orders: Order[];
  me: LatLng | null;
  selectedId: string | null;
  onSelect: (id: string | null) => void;
}

// divIcon avoids Leaflet's default marker asset paths, which break under bundlers.
function moneyIcon(order: Order, selected: boolean): L.DivIcon {
  const bg = selected ? "#059669" : "#18181b";
  const border = selected ? "#34d399" : "#3f3f46";
  const html =
    `<div style="transform:translate(-50%,-100%);white-space:nowrap;padding:2px 8px;border-radius:9999px;` +
    `font:600 12px/1.2 system-ui;color:#fff;background:${bg};border:1px solid ${border};` +
    `box-shadow:0 1px 4px rgba(0,0,0,.4)">${formatFiatChip(order.fiat_amount, order.fiat_currency)}</div>`;
  return L.divIcon({ className: "", html, iconSize: [0, 0], iconAnchor: [0, 0] });
}

function meIcon(): L.DivIcon {
  const html =
    `<div style="transform:translate(-50%,-50%);width:14px;height:14px;border-radius:9999px;` +
    `background:#38bdf8;border:2px solid #fff;box-shadow:0 0 0 4px rgba(56,189,248,.3)"></div>`;
  return L.divIcon({ className: "", html, iconSize: [0, 0], iconAnchor: [0, 0] });
}

// Fits the viewport to courier + orders once, and pans to a newly selected order.
function MapController({ orders, me, selectedId }: Omit<CourierMapProps, "onSelect">) {
  const map = useMap();
  const fitKey = useMemo(() => orders.map((o) => o.id).join(",") + "|" + (me ? "me" : ""), [orders, me]);

  useEffect(() => {
    const pts: L.LatLngExpression[] = orders.map((o) => [o.lat, o.lng]);
    if (me) pts.push([me.lat, me.lng]);
    if (pts.length === 0) return;
    if (pts.length === 1) {
      map.setView(pts[0], 14);
    } else {
      map.fitBounds(L.latLngBounds(pts).pad(0.2));
    }
    // Fit only when the set of points changes, not on every selection.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fitKey]);

  useEffect(() => {
    if (!selectedId) return;
    const o = orders.find((x) => x.id === selectedId);
    if (o) map.panTo([o.lat, o.lng]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId]);

  return null;
}

export default function CourierMap({ orders, me, selectedId, onSelect }: CourierMapProps) {
  const center: L.LatLngExpression = me
    ? [me.lat, me.lng]
    : orders[0]
    ? [orders[0].lat, orders[0].lng]
    : [0, 0];

  return (
    <MapContainer center={center} zoom={13} className="h-full w-full" zoomControl={false}>
      <TileLayer
        url="https://tile.openstreetmap.org/{z}/{x}/{y}.png"
        attribution="&copy; OpenStreetMap contributors"
      />
      {me && <Marker position={[me.lat, me.lng]} icon={meIcon()} interactive={false} />}
      {orders.map((o) => {
        const selected = o.id === selectedId;
        return (
          <div key={o.id}>
            <Circle
              center={[o.lat, o.lng]}
              radius={COARSEN_RADIUS_M}
              pathOptions={{
                color: selected ? "#34d399" : "#10b981",
                weight: 1,
                fillColor: "#10b981",
                fillOpacity: selected ? 0.18 : 0.08,
              }}
              eventHandlers={{ click: () => onSelect(o.id) }}
            />
            <Marker
              position={[o.lat, o.lng]}
              icon={moneyIcon(o, selected)}
              eventHandlers={{ click: () => onSelect(o.id) }}
              zIndexOffset={selected ? 1000 : 0}
            />
          </div>
        );
      })}
      <MapController orders={orders} me={me} selectedId={selectedId} />
    </MapContainer>
  );
}
```

- [ ] **Step 2: Type-check**

Run: `cd web && npx tsc --noEmit`
Expected: PASS (no type errors). Note: `react-leaflet` children must be its own elements; the `<div key>` wrapper around `Circle`+`Marker` is invalid inside `MapContainer`. **If tsc or runtime complains about non-layer children**, replace the wrapper with a React Fragment carrying the key:

```tsx
import { Fragment } from "react";
// ...
{orders.map((o) => {
  const selected = o.id === selectedId;
  return (
    <Fragment key={o.id}>
      <Circle ... />
      <Marker ... />
    </Fragment>
  );
})}
```

Use the `Fragment` form as the default implementation (it is the correct one for react-leaflet); the `<div>` above is not valid as a map child.

- [ ] **Step 3: Commit**

```bash
git add web/src/components/CourierMap.tsx
git commit -m "feat(web): add CourierMap with money pins and ~500m circles"
```

---

## Task 4: OrderSheet component

**Files:**
- Create: `web/src/components/OrderSheet.tsx`

**Interfaces:**
- Consumes: `Order` (`@/lib/types`), `impliedRate` (`@/components/OrderCard`), `haversineKm` / `LatLng` (`@/lib/geo`).
- Produces:
  - `export default function OrderSheet(props: OrderSheetProps)`
  - `interface OrderSheetProps { orders: Order[]; me: LatLng | null; selectedId: string | null; onSelect: (id: string | null) => void; onAccept: (id: string) => void; canAccept: boolean; accepting: string | null }`

- [ ] **Step 1: Implement the sheet**

Create `web/src/components/OrderSheet.tsx`:

```tsx
"use client";

import { useEffect, useRef, useState } from "react";
import type { Order } from "@/lib/types";
import { impliedRate } from "@/components/OrderCard";
import { haversineKm, type LatLng } from "@/lib/geo";

interface OrderSheetProps {
  orders: Order[];
  me: LatLng | null;
  selectedId: string | null;
  onSelect: (id: string | null) => void;
  onAccept: (id: string) => void;
  canAccept: boolean;
  accepting: string | null;
}

type Snap = "peek" | "half" | "full";
const SNAP_VH: Record<Snap, number> = { peek: 22, half: 55, full: 88 };

export default function OrderSheet({
  orders,
  me,
  selectedId,
  onSelect,
  onAccept,
  canAccept,
  accepting,
}: OrderSheetProps) {
  const [snap, setSnap] = useState<Snap>("half");
  const dragStart = useRef<{ y: number; vh: number } | null>(null);
  const [dragVh, setDragVh] = useState<number | null>(null);
  const cardRefs = useRef<Record<string, HTMLDivElement | null>>({});

  const sorted = me
    ? [...orders].sort((a, b) => haversineKm(me, a) - haversineKm(me, b))
    : orders;

  // Raise the sheet and scroll the selected card into view when selection changes.
  useEffect(() => {
    if (!selectedId) return;
    setSnap((s) => (s === "peek" ? "half" : s));
    const el = cardRefs.current[selectedId];
    if (el) el.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [selectedId]);

  const heightVh = dragVh ?? SNAP_VH[snap];

  function onPointerDown(e: React.PointerEvent) {
    (e.target as Element).setPointerCapture?.(e.pointerId);
    dragStart.current = { y: e.clientY, vh: SNAP_VH[snap] };
  }
  function onPointerMove(e: React.PointerEvent) {
    if (!dragStart.current) return;
    const dy = e.clientY - dragStart.current.y;
    const vh = dragStart.current.vh - (dy / window.innerHeight) * 100;
    setDragVh(Math.max(12, Math.min(92, vh)));
  }
  function onPointerUp() {
    if (dragVh != null) {
      const nearest = (["peek", "half", "full"] as Snap[]).reduce((best, s) =>
        Math.abs(SNAP_VH[s] - dragVh) < Math.abs(SNAP_VH[best] - dragVh) ? s : best
      );
      setSnap(nearest);
    }
    dragStart.current = null;
    setDragVh(null);
  }

  return (
    <div
      className="pointer-events-none fixed inset-x-0 bottom-0 z-[1000] md:inset-y-0 md:right-auto md:left-0 md:w-96"
      style={{ height: undefined }}
    >
      <div
        className="pointer-events-auto absolute inset-x-0 bottom-0 flex flex-col rounded-t-2xl border-t border-zinc-800 bg-zinc-950/95 backdrop-blur md:inset-0 md:h-full md:rounded-none md:border-r md:border-t-0"
        style={{ height: `${heightVh}vh` }}
      >
        {/* Drag handle (hidden on desktop, where the sheet is a static panel) */}
        <div
          className="flex shrink-0 cursor-grab touch-none flex-col items-center py-2 md:hidden"
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
        >
          <div className="h-1.5 w-10 rounded-full bg-zinc-700" />
        </div>

        <div className="shrink-0 px-4 pb-2 pt-1 md:pt-4">
          <h1 className="text-lg font-bold">Courier board</h1>
          <p className="text-xs text-zinc-400">
            {orders.length === 0
              ? "No open orders right now."
              : `${orders.length} open order${orders.length === 1 ? "" : "s"} · tap a pin or card`}
          </p>
          {!canAccept && (
            <p className="mt-2 rounded bg-amber-950 px-3 py-2 text-xs text-amber-300">
              Add a USDT address in{" "}
              <a href="/settings" className="underline">
                settings
              </a>{" "}
              before accepting orders.
            </p>
          )}
        </div>

        <div className="min-h-0 flex-1 space-y-2 overflow-y-auto px-4 pb-6">
          {sorted.map((o) => {
            const selected = o.id === selectedId;
            const dist = me ? `${haversineKm(me, o).toFixed(1)} km` : "—";
            return (
              <div
                key={o.id}
                ref={(el) => {
                  cardRefs.current[o.id] = el;
                }}
                onClick={() => onSelect(o.id)}
                className={`cursor-pointer rounded-lg border p-4 transition-colors ${
                  selected
                    ? "border-emerald-500 bg-emerald-950/30"
                    : "border-zinc-800 bg-zinc-900/50 hover:border-emerald-700"
                }`}
              >
                <div className="flex items-center justify-between">
                  <span className="text-lg font-semibold">
                    {parseFloat(o.fiat_amount).toLocaleString()} {o.fiat_currency}
                  </span>
                  <span className="text-emerald-400">
                    {parseFloat(o.usdt_amount).toLocaleString()} USDT
                  </span>
                </div>
                <div className="mt-1 flex justify-between text-sm text-zinc-400">
                  <span>{impliedRate(o)}</span>
                  <span>{dist}</span>
                </div>

                {selected && (
                  <div className="mt-3 border-t border-zinc-800 pt-3">
                    <p className="text-xs text-zinc-500">
                      Approximate area (~500 m). Exact address shared once you accept.
                    </p>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        onAccept(o.id);
                      }}
                      disabled={!canAccept || accepting === o.id}
                      className="mt-3 w-full rounded bg-emerald-600 px-4 py-2 font-medium text-white hover:bg-emerald-500 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      {accepting === o.id ? "Accepting…" : "Accept order"}
                    </button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Type-check**

Run: `cd web && npx tsc --noEmit`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add web/src/components/OrderSheet.tsx
git commit -m "feat(web): add draggable OrderSheet with inline accept"
```

---

## Task 5: Wire the map-first courier page

**Files:**
- Modify: `web/src/app/courier/page.tsx` (full rewrite)

**Interfaces:**
- Consumes: `CourierMap`, `OrderSheet`, `useUser`, `api`, `Order`, `LatLng`.
- Produces: the `/courier` route as a full-bleed map + sheet.

- [ ] **Step 1: Rewrite the page**

Replace the entire contents of `web/src/app/courier/page.tsx`:

```tsx
"use client";

import { useEffect, useState } from "react";
import dynamic from "next/dynamic";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";
import type { Order } from "@/lib/types";
import type { LatLng } from "@/lib/geo";
import { useUser } from "@/lib/useUser";
import OrderSheet from "@/components/OrderSheet";

// Leaflet needs `window`; load the map only on the client.
const CourierMap = dynamic(() => import("@/components/CourierMap"), {
  ssr: false,
  loading: () => <div className="h-full w-full bg-zinc-900" />,
});

export default function CourierPage() {
  const { user } = useUser();
  const router = useRouter();
  const [orders, setOrders] = useState<Order[] | null>(null);
  const [me, setMe] = useState<LatLng | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [accepting, setAccepting] = useState<string | null>(null);

  const canAccept = !!(user?.usdt_trc20 || user?.usdt_bep20 || user?.usdt_erc20);

  useEffect(() => {
    if (!user) return;
    const load = () =>
      api<Order[]>("/orders/open")
        .then((next) => {
          setOrders(next);
          // Clear a selection that is no longer open.
          setSelectedId((cur) => (cur && next.some((o) => o.id === cur) ? cur : null));
        })
        .catch(() => setOrders([]));
    load();
    const t = setInterval(load, 10000);
    navigator.geolocation?.getCurrentPosition(
      (p) => setMe({ lat: p.coords.latitude, lng: p.coords.longitude }),
      () => {}
    );
    return () => clearInterval(t);
  }, [user]);

  async function onAccept(id: string) {
    setAccepting(id);
    try {
      await api(`/orders/${id}/accept`, { method: "POST" });
      router.push(`/orders/${id}`);
    } catch {
      setAccepting(null);
      // Refresh so an already-taken order drops off the board.
      api<Order[]>("/orders/open").then(setOrders).catch(() => {});
    }
  }

  return (
    // Escape <main>'s max-w-3xl/padding: fixed, below the ~49px nav.
    <div className="fixed inset-x-0 bottom-0 top-[49px] z-0">
      {orders && (
        <CourierMap orders={orders} me={me} selectedId={selectedId} onSelect={setSelectedId} />
      )}
      {!orders && <div className="flex h-full items-center justify-center text-zinc-500">Loading…</div>}
      {orders && (
        <OrderSheet
          orders={orders}
          me={me}
          selectedId={selectedId}
          onSelect={setSelectedId}
          onAccept={onAccept}
          canAccept={canAccept}
          accepting={accepting}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Type-check the whole web app**

Run: `cd web && npx tsc --noEmit`
Expected: PASS.

- [ ] **Step 3: Production build**

Run: `cd web && npm run build`
Expected: build succeeds; `/courier` compiles as a client route with no SSR errors from Leaflet.

- [ ] **Step 4: Manual verification checklist**

Run `cd web && npm run dev` (with the API running per README: `docker compose -f compose.dev.yml up -d` and `cd api && cargo run`). Log in, create an open order from a second account, then on `/courier` confirm:
- Map fills the screen below the nav; a money-labelled pin + faint circle appears for the open order.
- "You are here" dot shows when geolocation is granted (distances render on cards); denying geolocation still fits the map to orders and shows "—" distances.
- Tapping a pin highlights it and expands its card in the sheet; tapping a card pans/opens its pin.
- Dragging the sheet handle snaps between peek/half/full on a mobile viewport; on a desktop width the sheet is a static left panel.
- Accept is disabled with the amber banner when the courier has no USDT address; with one set, Accept moves the order and navigates to `/orders/{id}`.
- Leaving the tab ~10 s and adding/removing an order updates pins and cards without resetting the map view or the current selection.

- [ ] **Step 5: Commit**

```bash
git add web/src/app/courier/page.tsx
git commit -m "feat(web): map-first courier board (full-bleed map + bottom sheet)"
```

---

## Self-Review Notes

- **Spec coverage:** map-first `/courier` (Task 5), ~500 m coarsening + circle (Tasks 1, 3), money pins (Task 3), draggable sheet + desktop panel (Task 4), pin↔card sync (Tasks 3–5), inline accept (Tasks 4–5), 10 s polling + selection-clear + geolocation fallbacks (Task 5), onboarding/empty states (Task 4). Out-of-scope items (clustering, landing, order creation) intentionally absent.
- **Type consistency:** `onSelect(id: string | null)`, `selectedId: string | null`, `me: LatLng | null`, `accepting: string | null`, and `formatFiatChip(fiatAmount, currency)` are used identically across Tasks 2–5.
- **No placeholders:** every code step is complete; the only conditional is the react-leaflet `Fragment` vs `<div>` child, resolved to `Fragment` as the default.
- **Testing realism:** backend logic is TDD'd against the existing Rust suite; pure frontend helpers are vitest-tested; component/interaction behavior is verified via `tsc`, `next build`, and an explicit manual checklist (no jsdom/RTL added — YAGNI).
```

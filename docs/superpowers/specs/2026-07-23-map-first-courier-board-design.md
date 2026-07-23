# Map-first courier board — design

Date: 2026-07-23
Status: Approved

## Summary

Rebuild the `/courier` route from a list into a **map-first, ride-hailing-style
experience**: a full-bleed map of open cash-delivery orders with a draggable
bottom sheet of order cards. Couriers see nearby cash requests spatially, judge
value at a glance from money-labelled pins, and accept without leaving the map.

Nothing else on the site changes: landing, order creation (form + map picker),
order detail, and settings are untouched. The only backend change is tightening
the existing open-order location coarsening from ~1 km to ~500 m (see below);
no schema, endpoint, or auth changes.

## Goals

- Make the courier's primary surface a live map instead of a list.
- Show open orders as **~500 m coarsened** pins labelled with the fiat amount,
  drawn with a radius circle so the imprecision is honest.
- Ride-hailing feel: full map + draggable bottom sheet (peek / half / full).
- Two-way sync between pins and cards (tap one, the other responds).
- Accept an order inline from the sheet, then continue to the existing order
  detail page to coordinate the meetup.
- Preserve existing behaviour: 10s polling, geolocation distance, the
  "add a USDT address" onboarding guard, and empty/error states.

## Non-goals (YAGNI for v1)

- Marker clustering (revisit once dozens of concurrent orders are common).
- Any change to how customers create orders.
- Any change to the landing page or order detail page.
- Any schema, endpoint, or auth change (the one backend edit is a constant in
  `redact_location`).

## Location privacy decision

Open-order pins are **coarsened server-side to a ~500 m grid**. The backend
already does this at ~1 km via `redact_location` in `api/src/orders/mod.rs`
(rounds lat/lng to 2 decimals, blanks the address); we tighten the grid from
2-decimal rounding to **snap-to-nearest-0.005°** (`(x * 200.0).round() / 200.0`),
which is ~500 m. The exact address and pin remain hidden until a courier accepts
(unchanged party-only reveal in `get_detail`).

The map honestly represents this imprecision: each open order is drawn as a
**~500 m circle** centred on the coarsened point, with the money-labelled marker
at that centre — not a false pinpoint. This is the only backend change; it
strengthens (not weakens) an existing protection relative to raw coordinates,
while giving couriers a tighter area than today.

## Architecture

### Data flow

`/orders/open` (auth-gated, existing) → polled every 10s → list of `Order`.
Each `Order` carries `lat`, `lng` (now ~500 m coarsened), `fiat_currency`,
`fiat_amount`, `usdt_amount`, `created_at`, `status`, and a blank `address_text`
for non-parties. No new fields, no new endpoints.

Courier geolocation comes from `navigator.geolocation` (as today). Distance is
computed client-side with the existing `haversineKm` helper.

### Components

**`app/courier/page.tsx`** (rewritten — thin container)
- Responsibilities: auth/onboarding guard, fetch + poll open orders, get courier
  geolocation, own `selectedOrderId` state, lay out map + sheet.
- Renders `CourierMap` via Next `dynamic(() => ..., { ssr: false })` because
  Leaflet requires `window`.
- Depends on: `useUser`, `api`, `CourierMap`, `OrderSheet`.

**`components/CourierMap.tsx`** (new)
- Responsibilities: render the Leaflet map, one money-labelled marker per order
  at its coarsened centre plus a **~500 m `Circle`** around it, and a distinct
  "you are here" marker. Emit selection when a pin/circle is clicked. Fit bounds
  to courier + orders on first load; when `selectedOrderId` changes from outside
  (a card tap), pan/open that pin and emphasise its circle.
- Props: `{ orders, me, selectedOrderId, onSelect }`.
- Depends on: `react-leaflet`, `leaflet`. Reuses the `L.divIcon` pattern from
  `MapPicker.tsx`.

**`components/OrderSheet.tsx`** (new)
- Responsibilities: draggable bottom sheet with three snap points
  (peek / half / full). Render order cards sorted by distance. Highlight and
  inline-expand the selected card (amount, implied rate, distance, address,
  Accept button). Emit selection on card tap; emit accept.
- Props: `{ orders, me, selectedOrderId, onSelect, onAccept, canAccept }`.
- Depends on: `impliedRate` + `haversineKm` (existing helpers, promoted to a
  shared location), `StatusBadge` optional.
- On wide screens (`md+`) the sheet docks as a fixed left-hand panel; on small
  screens it is a true bottom sheet. Same card content in both.

**`components/MoneyPin.tsx` icon helper** (may live inside `CourierMap`)
- Builds an `L.divIcon` showing a small amount chip (e.g. `€50`, `2.1M IDR`)
  with a selected/unselected style.

### Shared helpers

`haversineKm` (currently private in `courier/page.tsx`) and `impliedRate`
(currently exported from `OrderCard.tsx`) move to `lib/geo.ts` / stay in place
respectively so both the map and sheet import them without duplication.

## Interaction & states

- **Selection sync:** a single `selectedOrderId` in the container. Tap pin →
  card scrolls into view + expands + sheet raises to at least half. Tap card →
  pin opens/centres. Tap empty map → deselect.
- **Accept:** the sheet's Accept button calls the same accept endpoint the order
  detail page uses (`POST /orders/:id/accept` per existing API), then routes to
  `/orders/:id`. Guarded by `canAccept` (courier has a USDT address).
- **Live updates:** 10s polling continues. Order diffing must not reset the map
  viewport or drop the current selection; a selected order that disappears
  (accepted by someone else / cancelled) clears the selection with a small
  "no longer available" note.
- **Geolocation granted:** centre on courier, show per-order distances.
- **Geolocation denied:** fit bounds to open orders (fallback to a default
  region if none), distances render as "—".
- **Not onboarded (no USDT address):** show the existing amber
  "add a USDT address in settings" banner as an overlay; pins are viewable but
  Accept is disabled.
- **Empty:** "No open orders right now" in the sheet header; map still shows the
  courier's location.
- **Error / not logged in:** preserve current behaviour (redirect/loading via
  `useUser`, empty list on fetch failure).

## Testing

- Rust unit: `redact_location` snaps to the 0.005° grid (e.g. a coordinate is
  moved to the nearest ~500 m point) and blanks `address_text`; a party viewer
  in `get_detail` still sees exact coords + address.
- Unit: `haversineKm` correctness (known city-pair distances); `impliedRate`
  formatting; money-chip formatting for large IDR vs small EUR.
- Component: pin↔card selection sync; Accept disabled when no USDT address;
  selected order removed on next poll clears selection; each order renders a
  ~500 m circle.
- Manual: geolocation granted vs denied; sheet drag snap points on mobile
  viewport; desktop docked-panel layout; live add/remove of an order.

## Rollout / risk

- Single route rewrite + new components + one backend constant change in
  `redact_location`; no schema, endpoint, or deploy change.
- Existing `/courier` list behaviour is fully replaced, so the main risk is the
  Leaflet SSR boundary — mitigated by `dynamic(..., { ssr: false })`, already
  proven by `MapPicker.tsx`.
- The `redact_location` edit is behind the existing party/non-party split, so it
  cannot leak exact coordinates to non-parties; a unit test locks this in.

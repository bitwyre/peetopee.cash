"use client";

import { Fragment, useEffect, useMemo } from "react";
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
  // Where to open when there is no location or order to anchor on (locale-derived).
  defaultView?: { center: LatLng; zoom: number };
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

export default function CourierMap({ orders, me, selectedId, onSelect, defaultView }: CourierMapProps) {
  const anchor = me ?? orders[0] ?? null;
  // Anchor on the courier/first order; otherwise open at the locale-derived view.
  const center: L.LatLngExpression = anchor
    ? [anchor.lat, anchor.lng]
    : defaultView
    ? [defaultView.center.lat, defaultView.center.lng]
    : [25, 15];
  const zoom = anchor ? 13 : defaultView?.zoom ?? 3;

  return (
    <MapContainer center={center} zoom={zoom} className="h-full w-full" zoomControl={false}>
      <TileLayer
        url="https://tile.openstreetmap.org/{z}/{x}/{y}.png"
        attribution="&copy; OpenStreetMap contributors"
      />
      {me && <Marker position={[me.lat, me.lng]} icon={meIcon()} interactive={false} />}
      {orders.map((o) => {
        const selected = o.id === selectedId;
        return (
          <Fragment key={o.id}>
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
          </Fragment>
        );
      })}
      <MapController orders={orders} me={me} selectedId={selectedId} />
    </MapContainer>
  );
}

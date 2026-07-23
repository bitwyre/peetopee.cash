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
          onPointerCancel={onPointerUp}
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

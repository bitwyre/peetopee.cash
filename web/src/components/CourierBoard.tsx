"use client";

import { useEffect, useMemo, useState } from "react";
import dynamic from "next/dynamic";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";
import type { Order } from "@/lib/types";
import type { LatLng } from "@/lib/geo";
import { DEFAULT_VIEW, detectMapView, type MapView } from "@/lib/locale";
import { useUser } from "@/lib/useUser";
import OrderSheet from "@/components/OrderSheet";

// Leaflet needs `window`; load the map only on the client.
const CourierMap = dynamic(() => import("@/components/CourierMap"), {
  ssr: false,
  loading: () => <div className="h-full w-full bg-zinc-900" />,
});

type GeoState = "idle" | "locating" | "granted" | "denied";

export default function CourierBoard() {
  // Public surface — never bounce logged-out visitors to /login.
  const { user, loading } = useUser({ redirect: false });
  const router = useRouter();
  const [orders, setOrders] = useState<Order[]>([]);
  const [me, setMe] = useState<LatLng | null>(null);
  const [geo, setGeo] = useState<GeoState>("idle");
  const [view, setView] = useState<MapView>(DEFAULT_VIEW);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [accepting, setAccepting] = useState<string | null>(null);

  const canAccept = !!(user?.usdt_trc20 || user?.usdt_bep20 || user?.usdt_erc20);

  // Locale-derived opening view (Jakarta, London, …) so the map is never a blank globe.
  useEffect(() => setView(detectMapView()), []);

  const locate = useMemo(
    () => () => {
      if (typeof navigator === "undefined" || !navigator.geolocation) {
        setGeo("denied");
        return;
      }
      setGeo("locating");
      navigator.geolocation.getCurrentPosition(
        (p) => {
          setMe({ lat: p.coords.latitude, lng: p.coords.longitude });
          setGeo("granted");
        },
        () => setGeo("denied"),
        { enableHighAccuracy: true, timeout: 10000, maximumAge: 60000 }
      );
    },
    []
  );

  // Ask for location once on mount.
  useEffect(() => locate(), [locate]);

  // Live orders require auth (/orders/open); only poll when signed in.
  useEffect(() => {
    if (!user) {
      setOrders([]);
      return;
    }
    const load = () =>
      api<Order[]>("/orders/open")
        .then((next) => {
          setOrders(next);
          setSelectedId((cur) => (cur && next.some((o) => o.id === cur) ? cur : null));
        })
        .catch(() => setOrders([]));
    load();
    const t = setInterval(load, 10000);
    return () => clearInterval(t);
  }, [user]);

  async function onAccept(id: string) {
    setAccepting(id);
    try {
      await api(`/orders/${id}/accept`, { method: "POST" });
      router.push(`/orders/${id}`);
    } catch {
      setAccepting(null);
      api<Order[]>("/orders/open").then(setOrders).catch(() => {});
    }
  }

  return (
    // Escape <main>'s max-w-3xl/padding: fixed, below the ~49px nav.
    <div className="fixed inset-x-0 bottom-0 top-[49px] z-0">
      <CourierMap
        orders={orders}
        me={me}
        selectedId={selectedId}
        onSelect={setSelectedId}
        defaultView={view}
      />

      {/* Locate control — floats above the sheet, only while we don't have a fix. */}
      {geo !== "granted" && (
        <button
          onClick={locate}
          disabled={geo === "locating"}
          className="absolute right-3 top-3 z-[1100] flex items-center gap-2 rounded-full border border-zinc-700 bg-zinc-900/90 px-3 py-2 text-sm font-medium text-zinc-100 shadow-lg backdrop-blur hover:border-emerald-600 disabled:opacity-60 md:left-[25rem] md:right-auto"
        >
          <span aria-hidden>📍</span>
          {geo === "locating" ? "Locating…" : geo === "denied" ? "Enable location" : "Use my location"}
        </button>
      )}

      {user ? (
        <OrderSheet
          orders={orders}
          me={me}
          selectedId={selectedId}
          onSelect={setSelectedId}
          onAccept={onAccept}
          canAccept={canAccept}
          accepting={accepting}
        />
      ) : (
        !loading && (
          <div className="pointer-events-none absolute inset-x-0 bottom-0 z-[1000] p-4">
            <div className="pointer-events-auto mx-auto max-w-md rounded-2xl border border-zinc-800 bg-zinc-950/95 p-5 shadow-2xl backdrop-blur">
              <h2 className="text-lg font-bold">
                Cash, delivered. <span className="text-emerald-400">Paid in USDT.</span>
              </h2>
              <p className="mt-1 text-sm text-zinc-400">
                A courier meets you, you send USDT, the chain confirms it, you get your cash.
                Log in to see live orders near you.
              </p>
              <div className="mt-4 flex gap-3">
                <Link
                  href="/orders/new"
                  className="flex-1 rounded bg-emerald-600 px-4 py-2.5 text-center font-medium hover:bg-emerald-500"
                >
                  Get cash
                </Link>
                <Link
                  href="/login"
                  className="flex-1 rounded border border-zinc-700 px-4 py-2.5 text-center font-medium hover:border-emerald-600"
                >
                  Log in to deliver
                </Link>
              </div>
            </div>
          </div>
        )
      )}
    </div>
  );
}

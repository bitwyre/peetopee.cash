"use client";

import { useEffect, useState } from "react";
import dynamic from "next/dynamic";
import Link from "next/link";
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

export default function LandingPage() {
  // Landing is public — don't bounce logged-out visitors to /login.
  const { user, loading } = useUser({ redirect: false });
  const router = useRouter();
  const [orders, setOrders] = useState<Order[]>([]);
  const [me, setMe] = useState<LatLng | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [accepting, setAccepting] = useState<string | null>(null);

  const canAccept = !!(user?.usdt_trc20 || user?.usdt_bep20 || user?.usdt_erc20);

  // Everyone gets the map centered on their location, if they allow it.
  useEffect(() => {
    navigator.geolocation?.getCurrentPosition(
      (p) => setMe({ lat: p.coords.latitude, lng: p.coords.longitude }),
      () => {}
    );
  }, []);

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
          // Clear a selection that is no longer open.
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
      // Refresh so an already-taken order drops off the board.
      api<Order[]>("/orders/open").then(setOrders).catch(() => {});
    }
  }

  return (
    // Escape <main>'s max-w-3xl/padding: fixed, below the ~49px nav.
    <div className="fixed inset-x-0 bottom-0 top-[49px] z-0">
      <CourierMap orders={orders} me={me} selectedId={selectedId} onSelect={setSelectedId} />

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
            <div className="pointer-events-auto mx-auto max-w-md rounded-2xl border border-zinc-800 bg-zinc-900/95 p-5 shadow-2xl backdrop-blur">
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

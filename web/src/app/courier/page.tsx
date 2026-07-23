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

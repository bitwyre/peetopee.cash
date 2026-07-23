"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api";
import type { Order } from "@/lib/types";
import { impliedRate } from "@/components/OrderCard";
import { useUser } from "@/lib/useUser";

function haversineKm(a: { lat: number; lng: number }, b: { lat: number; lng: number }): number {
  const R = 6371;
  const dLat = ((b.lat - a.lat) * Math.PI) / 180;
  const dLng = ((b.lng - a.lng) * Math.PI) / 180;
  const s =
    Math.sin(dLat / 2) ** 2 +
    Math.cos((a.lat * Math.PI) / 180) * Math.cos((b.lat * Math.PI) / 180) * Math.sin(dLng / 2) ** 2;
  return 2 * R * Math.asin(Math.sqrt(s));
}

export default function CourierPage() {
  const { user } = useUser();
  const [orders, setOrders] = useState<Order[] | null>(null);
  const [me, setMe] = useState<{ lat: number; lng: number } | null>(null);
  const hasAddress = !!(user?.usdt_trc20 || user?.usdt_bep20 || user?.usdt_erc20);

  useEffect(() => {
    if (!user) return;
    const load = () => api<Order[]>("/orders/open").then(setOrders).catch(() => setOrders([]));
    load();
    const t = setInterval(load, 10000);
    navigator.geolocation?.getCurrentPosition((p) => setMe({ lat: p.coords.latitude, lng: p.coords.longitude }), () => {});
    return () => clearInterval(t);
  }, [user]);

  if (!orders) return <p className="text-zinc-500">Loading...</p>;

  return (
    <div>
      <h1 className="mb-2 text-2xl font-bold">Courier board</h1>
      <p className="mb-6 text-sm text-zinc-400">Open cash-delivery requests. Accept one, meet the customer, receive USDT, hand over cash.</p>
      {!hasAddress && (
        <p className="mb-4 rounded bg-amber-950 px-3 py-2 text-sm text-amber-300">
          Add a USDT address in <Link href="/settings" className="underline">settings</Link> before accepting orders.
        </p>
      )}
      {orders.length === 0 && <p className="text-zinc-400">No open orders right now.</p>}
      <div className="space-y-3">
        {orders.map((o) => (
          <Link key={o.id} href={`/orders/${o.id}`}
            className="block rounded-lg border border-zinc-800 bg-zinc-900/50 p-4 hover:border-emerald-700">
            <div className="flex items-center justify-between">
              <span className="text-lg font-semibold">{parseFloat(o.fiat_amount).toLocaleString()} {o.fiat_currency}</span>
              <span className="text-emerald-400">{parseFloat(o.usdt_amount).toLocaleString()} USDT</span>
            </div>
            <div className="mt-1 flex justify-between text-sm text-zinc-400">
              <span>{impliedRate(o)}</span>
              <span>{me ? `${haversineKm(me, o).toFixed(1)} km away` : new Date(o.created_at).toLocaleString()}</span>
            </div>
          </Link>
        ))}
      </div>
    </div>
  );
}

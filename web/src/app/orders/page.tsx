"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api";
import type { Order } from "@/lib/types";
import OrderCard from "@/components/OrderCard";
import { useUser } from "@/lib/useUser";

const ACTIVE = ["OPEN", "ACCEPTED", "AWAITING_PAYMENT", "PAID"];

export default function OrdersPage() {
  const { user } = useUser();
  const [orders, setOrders] = useState<Order[] | null>(null);

  useEffect(() => {
    if (user) api<Order[]>("/orders/mine").then(setOrders).catch(() => setOrders([]));
  }, [user]);

  if (!orders) return <p className="text-zinc-500">Loading...</p>;

  const active = orders.filter((o) => ACTIVE.includes(o.status));
  const past = orders.filter((o) => !ACTIVE.includes(o.status));

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-2xl font-bold">My orders</h1>
        <Link href="/orders/new" className="rounded bg-emerald-600 px-4 py-2 text-sm font-medium hover:bg-emerald-500">Get cash</Link>
      </div>
      {orders.length === 0 && <p className="text-zinc-400">No orders yet. Post one to get cash delivered.</p>}
      {active.length > 0 && (
        <section className="space-y-3">
          {active.map((o) => <OrderCard key={o.id} order={o} />)}
        </section>
      )}
      {past.length > 0 && (
        <section className="mt-8">
          <h2 className="mb-3 text-sm font-medium uppercase tracking-wide text-zinc-500">Past</h2>
          <div className="space-y-3">{past.map((o) => <OrderCard key={o.id} order={o} />)}</div>
        </section>
      )}
    </div>
  );
}

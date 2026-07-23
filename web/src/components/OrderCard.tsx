import Link from "next/link";
import type { Order } from "@/lib/types";
import StatusBadge from "./StatusBadge";

export function impliedRate(order: Order): string {
  const fiat = parseFloat(order.fiat_amount);
  const usdt = parseFloat(order.usdt_amount);
  if (!usdt) return "—";
  return `${(fiat / usdt).toLocaleString(undefined, { maximumFractionDigits: 2 })} ${order.fiat_currency}/USDT`;
}

export default function OrderCard({ order }: { order: Order }) {
  return (
    <Link
      href={`/orders/${order.id}`}
      className="block rounded-lg border border-zinc-800 bg-zinc-900/50 p-4 hover:border-emerald-700"
    >
      <div className="flex items-center justify-between">
        <span className="text-lg font-semibold">
          {parseFloat(order.fiat_amount).toLocaleString()} {order.fiat_currency}
        </span>
        <StatusBadge status={order.status} />
      </div>
      <div className="mt-1 flex justify-between text-sm text-zinc-400">
        <span>{parseFloat(order.usdt_amount).toLocaleString()} USDT · {impliedRate(order)}</span>
        <span>{new Date(order.created_at).toLocaleDateString()}</span>
      </div>
    </Link>
  );
}

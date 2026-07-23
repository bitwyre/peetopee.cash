"use client";

import { use, useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { OrderDetail } from "@/lib/types";
import StatusBadge from "@/components/StatusBadge";
import PaymentPanel from "@/components/PaymentPanel";
import { impliedRate } from "@/components/OrderCard";
import { useUser } from "@/lib/useUser";

const STEPS = ["OPEN", "ACCEPTED", "AWAITING_PAYMENT", "PAID", "COMPLETED"] as const;

export default function OrderDetailPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = use(params);
  const { user } = useUser();
  const [detail, setDetail] = useState<OrderDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api<OrderDetail>(`/orders/${id}`).then(setDetail).catch((e: Error) => setError(e.message));
  }, [id]);

  useEffect(() => {
    if (!user) return;
    load();
    const t = setInterval(load, 5000);
    return () => clearInterval(t);
  }, [user, load]);

  async function action(name: string) {
    setBusy(true);
    setError(null);
    try {
      await api(`/orders/${id}/${name}`, { method: "POST" });
      load();
    } catch (e) {
      setError(e instanceof Error ? e.message : "action failed");
    } finally {
      setBusy(false);
    }
  }

  if (error && !detail) return <p className="text-red-400">{error}</p>;
  if (!detail) return <p className="text-zinc-500">Loading...</p>;

  const stepIdx = STEPS.indexOf(detail.status as (typeof STEPS)[number]);
  const otherTelegram = detail.is_customer ? detail.courier_telegram : detail.customer_telegram;
  const canCancel =
    (detail.status === "OPEN" && detail.is_customer) ||
    (detail.status === "ACCEPTED" && (detail.is_customer || detail.is_courier)) ||
    (detail.status === "AWAITING_PAYMENT" &&
      (detail.is_customer || detail.is_courier) &&
      detail.payment_requested_at !== null &&
      Date.now() - new Date(detail.payment_requested_at).getTime() > 2 * 3600_000);

  return (
    <div className="mx-auto max-w-lg space-y-5">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">
          {parseFloat(detail.fiat_amount).toLocaleString()} {detail.fiat_currency}
        </h1>
        <StatusBadge status={detail.status} />
      </div>
      <p className="text-zinc-400">
        {parseFloat(detail.usdt_amount).toLocaleString()} USDT · {impliedRate(detail)}
      </p>

      {detail.status !== "CANCELLED" && (
        <ol className="flex gap-1">
          {STEPS.map((s, i) => (
            <li key={s} className={`h-1.5 flex-1 rounded ${i <= stepIdx ? "bg-emerald-500" : "bg-zinc-800"}`} title={s} />
          ))}
        </ol>
      )}

      <div className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-4 text-sm">
        <p className="text-zinc-400">Delivery address</p>
        {detail.address_text ? (
          <p className="mt-1">{detail.address_text}</p>
        ) : (
          <p className="mt-1 text-zinc-500">Exact address and pin are shared once you accept the order.</p>
        )}
      </div>

      {otherTelegram && (
        <a
          href={`https://t.me/${otherTelegram}`}
          target="_blank"
          rel="noreferrer"
          className="block rounded-lg border border-sky-900 bg-sky-950/40 p-4 text-sm hover:border-sky-700"
        >
          💬 Coordinate the meetup on Telegram: <span className="font-medium text-sky-300">@{otherTelegram}</span>
        </a>
      )}

      {detail.status === "OPEN" && !detail.is_customer && (
        <button onClick={() => action("accept")} disabled={busy}
          className="w-full rounded bg-emerald-600 px-4 py-2 font-medium hover:bg-emerald-500 disabled:opacity-50">
          Accept this delivery
        </button>
      )}

      {detail.is_courier && detail.status === "ACCEPTED" && (
        <button onClick={() => action("request-payment")} disabled={busy}
          className="w-full rounded bg-amber-600 px-4 py-2 font-medium text-black hover:bg-amber-500 disabled:opacity-50">
          I&apos;ve arrived — request USDT payment
        </button>
      )}

      {detail.is_customer && detail.status === "AWAITING_PAYMENT" && <PaymentPanel detail={detail} />}

      {detail.is_courier && detail.status === "AWAITING_PAYMENT" && (
        <p className="animate-pulse text-sm text-amber-400">Waiting for the customer&apos;s USDT to land on-chain…</p>
      )}

      {detail.status === "PAID" && (
        <div className="rounded-lg border border-emerald-900 bg-emerald-950/40 p-4 text-sm">
          <p className="text-emerald-300">✅ USDT received on-chain{detail.payment_txid ? ` (tx ${detail.payment_txid.slice(0, 10)}…)` : ""}.</p>
          {detail.is_courier && <p className="mt-1 text-zinc-300">Hand over the cash now.</p>}
          {detail.is_customer && (
            <button onClick={() => action("confirm-cash")} disabled={busy}
              className="mt-3 w-full rounded bg-emerald-600 px-4 py-2 font-medium hover:bg-emerald-500 disabled:opacity-50">
              I received the cash
            </button>
          )}
        </div>
      )}

      {detail.status === "COMPLETED" && <p className="text-emerald-400">Order completed. 🎉</p>}
      {detail.status === "CANCELLED" && <p className="text-zinc-400">This order was cancelled.</p>}

      {canCancel && (
        <button onClick={() => action("cancel")} disabled={busy}
          className="w-full rounded border border-red-900 px-4 py-2 text-sm text-red-400 hover:bg-red-950 disabled:opacity-50">
          Cancel order
        </button>
      )}
      {error && <p className="text-sm text-red-400">{error}</p>}
    </div>
  );
}

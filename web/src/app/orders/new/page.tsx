"use client";

import { useEffect, useState } from "react";
import dynamic from "next/dynamic";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";
import { CURRENCIES, type Currency, type Order } from "@/lib/types";
import { useUser } from "@/lib/useUser";

const MapPicker = dynamic(() => import("@/components/MapPicker"), { ssr: false });

// Default: Denpasar, Bali
const DEFAULT_POS = { lat: -8.6705, lng: 115.2126 };

export default function NewOrderPage() {
  const router = useRouter();
  const { user, loading } = useUser();
  const [currency, setCurrency] = useState<Currency>("IDR");
  const [fiatAmount, setFiatAmount] = useState("");
  const [usdtAmount, setUsdtAmount] = useState("");
  const [address, setAddress] = useState("");
  const [pos, setPos] = useState(DEFAULT_POS);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    navigator.geolocation?.getCurrentPosition(
      (p) => setPos({ lat: p.coords.latitude, lng: p.coords.longitude }),
      () => {},
    );
  }, []);

  if (loading || !user) return <p className="text-zinc-500">Loading...</p>;

  const rate =
    parseFloat(fiatAmount) > 0 && parseFloat(usdtAmount) > 0
      ? (parseFloat(fiatAmount) / parseFloat(usdtAmount)).toLocaleString(undefined, { maximumFractionDigits: 2 })
      : null;

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const order = await api<Order>("/orders", {
        method: "POST",
        body: JSON.stringify({
          fiat_currency: currency,
          fiat_amount: fiatAmount,
          usdt_amount: usdtAmount,
          address_text: address,
          lat: pos.lat,
          lng: pos.lng,
        }),
      });
      router.push(`/orders/${order.id}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed to create order");
      setSubmitting(false);
    }
  }

  return (
    <div className="mx-auto max-w-lg">
      <h1 className="mb-6 text-2xl font-bold">Get cash delivered</h1>
      <form onSubmit={submit} className="space-y-4">
        <div className="flex gap-3">
          <label className="block w-32">
            <span className="mb-1 block text-sm text-zinc-400">Currency</span>
            <select
              value={currency}
              onChange={(e) => setCurrency(e.target.value as Currency)}
              className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2"
            >
              {CURRENCIES.map((c) => (
                <option key={c} value={c}>{c}</option>
              ))}
            </select>
          </label>
          <label className="block flex-1">
            <span className="mb-1 block text-sm text-zinc-400">Cash amount you want</span>
            <input
              type="number" step="any" min="0" required
              value={fiatAmount}
              onChange={(e) => setFiatAmount(e.target.value)}
              placeholder="1500000"
              className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2"
            />
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-sm text-zinc-400">USDT you will pay</span>
          <input
            type="number" step="any" min="0" required
            value={usdtAmount}
            onChange={(e) => setUsdtAmount(e.target.value)}
            placeholder="95"
            className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2"
          />
        </label>
        {rate && <p className="text-sm text-zinc-400">Implied rate: <span className="text-zinc-200">{rate} {currency}/USDT</span> — couriers see this when deciding to accept.</p>}
        <label className="block">
          <span className="mb-1 block text-sm text-zinc-400">Delivery address</span>
          <textarea
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            required maxLength={500} rows={2}
            placeholder="Street, number, notes for the courier..."
            className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2"
          />
        </label>
        <div>
          <span className="mb-1 block text-sm text-zinc-400">Pin your location (drag the pin or click the map)</span>
          <MapPicker value={pos} onChange={setPos} />
        </div>
        {error && <p className="text-sm text-red-400">{error}</p>}
        <button disabled={submitting} className="w-full rounded bg-emerald-600 px-4 py-2 font-medium hover:bg-emerald-500 disabled:opacity-50">
          {submitting ? "Posting..." : "Post order"}
        </button>
      </form>
    </div>
  );
}

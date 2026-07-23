"use client";

import { useState } from "react";
import { QRCodeSVG } from "qrcode.react";
import type { Network, OrderDetail } from "@/lib/types";

const NETWORK_LABELS: Record<Network, string> = {
  trc20: "TRC20 · Tron",
  bep20: "BEP20 · BNB Chain",
  erc20: "ERC20 · Ethereum",
};

export default function PaymentPanel({ detail }: { detail: OrderDetail }) {
  const usdt = detail.courier_usdt;
  const available = (Object.keys(NETWORK_LABELS) as Network[]).filter((n) => usdt?.[n]);
  const [network, setNetwork] = useState<Network | null>(available[0] ?? null);

  if (!usdt || !network) return null;
  const address = usdt[network]!;

  return (
    <div className="rounded-lg border border-amber-900 bg-amber-950/30 p-4">
      <h3 className="font-semibold text-amber-300">
        Send exactly {parseFloat(detail.usdt_amount).toLocaleString()} USDT
      </h3>
      <p className="mt-1 text-sm text-zinc-400">
        Payment is detected automatically on-chain — usually within a minute of confirmation.
      </p>
      <div className="mt-3 flex gap-2">
        {available.map((n) => (
          <button
            key={n}
            onClick={() => setNetwork(n)}
            className={`rounded px-3 py-1 text-xs font-medium ${n === network ? "bg-amber-600 text-black" : "bg-zinc-800 text-zinc-300"}`}
          >
            {NETWORK_LABELS[n]}
          </button>
        ))}
      </div>
      <div className="mt-4 flex items-center gap-4">
        <div className="rounded bg-white p-2">
          <QRCodeSVG value={address} size={112} />
        </div>
        <div className="min-w-0">
          <p className="text-xs text-zinc-500">Courier&apos;s {NETWORK_LABELS[network]} address</p>
          <p className="break-all font-mono text-sm">{address}</p>
          <button
            onClick={() => navigator.clipboard.writeText(address)}
            className="mt-2 rounded bg-zinc-800 px-3 py-1 text-xs hover:bg-zinc-700"
          >
            Copy address
          </button>
        </div>
      </div>
      <p className="mt-3 animate-pulse text-sm text-amber-400">Waiting for payment on-chain…</p>
    </div>
  );
}

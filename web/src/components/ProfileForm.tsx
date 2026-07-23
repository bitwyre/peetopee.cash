"use client";

import { useState } from "react";
import { api } from "@/lib/api";
import type { User } from "@/lib/types";

const FIELDS = [
  { key: "usdt_trc20", label: "USDT address — TRC20 (Tron)", placeholder: "T..." },
  { key: "usdt_bep20", label: "USDT address — BEP20 (BNB Smart Chain)", placeholder: "0x..." },
  { key: "usdt_erc20", label: "USDT address — ERC20 (Ethereum)", placeholder: "0x..." },
] as const;

export default function ProfileForm({ initial, onSaved }: { initial: User; onSaved: (u: User) => void }) {
  const [telegram, setTelegram] = useState(initial.telegram_handle ?? "");
  const [addrs, setAddrs] = useState({
    usdt_trc20: initial.usdt_trc20 ?? "",
    usdt_bep20: initial.usdt_bep20 ?? "",
    usdt_erc20: initial.usdt_erc20 ?? "",
  });
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function save(e: React.FormEvent) {
    e.preventDefault();
    setSaving(true);
    setError(null);
    try {
      const updated = await api<User>("/me", {
        method: "PATCH",
        body: JSON.stringify({ telegram_handle: telegram, ...addrs }),
      });
      onSaved(updated);
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed to save");
    } finally {
      setSaving(false);
    }
  }

  return (
    <form onSubmit={save} className="space-y-4">
      <label className="block">
        <span className="mb-1 block text-sm text-zinc-400">Telegram handle (required — the other party contacts you here)</span>
        <input
          value={telegram}
          onChange={(e) => setTelegram(e.target.value)}
          placeholder="@yourhandle"
          required
          className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2"
        />
      </label>
      {FIELDS.map((f) => (
        <label key={f.key} className="block">
          <span className="mb-1 block text-sm text-zinc-400">{f.label}</span>
          <input
            value={addrs[f.key]}
            onChange={(e) => setAddrs({ ...addrs, [f.key]: e.target.value })}
            placeholder={f.placeholder}
            className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2 font-mono text-sm"
          />
        </label>
      ))}
      <p className="text-xs text-zinc-500">At least one USDT address is required to accept orders as a courier.</p>
      {error && <p className="text-sm text-red-400">{error}</p>}
      <button disabled={saving} className="rounded bg-emerald-600 px-4 py-2 font-medium hover:bg-emerald-500 disabled:opacity-50">
        {saving ? "Saving..." : "Save"}
      </button>
    </form>
  );
}

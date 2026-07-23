"use client";

import { Suspense, useState } from "react";
import { useSearchParams } from "next/navigation";
import { api } from "@/lib/api";

function LoginForm() {
  const [email, setEmail] = useState("");
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const params = useSearchParams();

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    try {
      await api("/auth/request-link", { method: "POST", body: JSON.stringify({ email }) });
      setSent(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed");
    }
  }

  if (sent) {
    return (
      <div className="mx-auto max-w-sm pt-16 text-center">
        <h1 className="text-2xl font-bold">Check your inbox</h1>
        <p className="mt-3 text-zinc-400">We sent a login link to <span className="text-zinc-200">{email}</span>. It expires in 15 minutes.</p>
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-sm pt-16">
      <h1 className="text-2xl font-bold">Log in or sign up</h1>
      <p className="mt-2 text-sm text-zinc-400">Enter your email and we&apos;ll send you a magic link. No password needed.</p>
      {params.get("error") === "expired" && (
        <p className="mt-3 rounded bg-amber-950 px-3 py-2 text-sm text-amber-300">That link was expired or already used — request a new one.</p>
      )}
      <form onSubmit={submit} className="mt-6 space-y-3">
        <input
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          placeholder="you@example.com"
          required
          className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2"
        />
        {error && <p className="text-sm text-red-400">{error}</p>}
        <button className="w-full rounded bg-emerald-600 px-4 py-2 font-medium hover:bg-emerald-500">Send magic link</button>
      </form>
    </div>
  );
}

export default function LoginPage() {
  return (
    <Suspense>
      <LoginForm />
    </Suspense>
  );
}

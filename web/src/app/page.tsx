import Link from "next/link";
import { CURRENCIES } from "@/lib/types";

const STEPS = [
  { title: "Post your order", body: "Say how much cash you need, what you'll pay in USDT, and where you are." },
  { title: "A courier accepts", body: "Couriers see your offer and rate. You coordinate the meetup on Telegram." },
  { title: "Swap at your door", body: "Send USDT to the courier's wallet — we verify it on-chain — and take your cash." },
];

export default function LandingPage() {
  return (
    <div className="py-10">
      <section className="text-center">
        <h1 className="text-4xl font-extrabold tracking-tight">
          Cash, delivered. <span className="text-emerald-400">Paid in USDT.</span>
        </h1>
        <p className="mx-auto mt-4 max-w-xl text-zinc-400">
          peetopee.cash brings physical cash to your door. A courier meets you, you send USDT
          (TRC20, BEP20 or ERC20), the chain confirms it, you get your cash. No bank, no queue.
        </p>
        <div className="mt-8 flex justify-center gap-3">
          <Link href="/orders/new" className="rounded bg-emerald-600 px-6 py-3 font-medium hover:bg-emerald-500">Get cash</Link>
          <Link href="/courier" className="rounded border border-zinc-700 px-6 py-3 font-medium hover:border-emerald-600">Deliver cash</Link>
        </div>
      </section>
      <section className="mx-auto mt-16 grid max-w-2xl gap-6 sm:grid-cols-3">
        {STEPS.map((s, i) => (
          <div key={s.title} className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-5">
            <div className="text-2xl font-bold text-emerald-400">{i + 1}</div>
            <h3 className="mt-2 font-semibold">{s.title}</h3>
            <p className="mt-1 text-sm text-zinc-400">{s.body}</p>
          </div>
        ))}
      </section>
      <section className="mt-16 text-center">
        <p className="text-sm uppercase tracking-wide text-zinc-500">Supported currencies</p>
        <div className="mt-3 flex flex-wrap justify-center gap-2">
          {CURRENCIES.map((c) => (
            <span key={c} className="rounded-full border border-zinc-700 px-3 py-1 text-sm">{c}</span>
          ))}
        </div>
      </section>
    </div>
  );
}

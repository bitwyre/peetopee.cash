"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";
import { useUser } from "@/lib/useUser";

export default function Nav() {
  const { user } = useUser({ redirect: false });
  const router = useRouter();

  async function logout() {
    await api("/auth/logout", { method: "POST" }).catch(() => {});
    router.push("/login");
    router.refresh();
  }

  return (
    <nav className="border-b border-zinc-800 bg-zinc-900/60">
      <div className="mx-auto flex max-w-3xl items-center gap-5 px-4 py-3 text-sm">
        <Link href="/" className="font-bold text-emerald-400">peetopee.cash</Link>
        {user ? (
          <>
            <Link href="/orders" className="hover:text-emerald-300">My orders</Link>
            <Link href="/orders/new" className="hover:text-emerald-300">Get cash</Link>
            <Link href="/courier" className="hover:text-emerald-300">Courier board</Link>
            <span className="ml-auto flex items-center gap-4">
              <Link href="/settings" className="text-zinc-400 hover:text-emerald-300">Settings</Link>
              <button onClick={logout} className="text-zinc-400 hover:text-red-400">Log out</button>
            </span>
          </>
        ) : (
          <Link href="/login" className="ml-auto hover:text-emerald-300">Log in</Link>
        )}
      </div>
    </nav>
  );
}

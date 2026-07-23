"use client";

import { useRouter } from "next/navigation";
import ProfileForm from "@/components/ProfileForm";
import { useUser } from "@/lib/useUser";

export default function OnboardingPage() {
  const { user, loading } = useUser();
  const router = useRouter();

  if (loading || !user) return <p className="text-zinc-500">Loading...</p>;

  return (
    <div className="mx-auto max-w-md">
      <h1 className="text-2xl font-bold">Welcome 👋</h1>
      <p className="mb-6 mt-2 text-sm text-zinc-400">
        Set your Telegram handle so customers and couriers can coordinate meetups with you.
      </p>
      <ProfileForm initial={user} onSaved={() => router.push("/orders")} />
    </div>
  );
}

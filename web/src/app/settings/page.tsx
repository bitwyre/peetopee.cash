"use client";

import { useState } from "react";
import ProfileForm from "@/components/ProfileForm";
import { useUser } from "@/lib/useUser";

export default function SettingsPage() {
  const { user, loading } = useUser();
  const [saved, setSaved] = useState(false);

  if (loading || !user) return <p className="text-zinc-500">Loading...</p>;

  return (
    <div className="mx-auto max-w-md">
      <h1 className="mb-6 text-2xl font-bold">Settings</h1>
      <ProfileForm initial={user} onSaved={() => setSaved(true)} />
      {saved && <p className="mt-3 text-sm text-emerald-400">Saved.</p>}
    </div>
  );
}

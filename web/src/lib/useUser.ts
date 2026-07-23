"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "./api";
import type { User } from "./types";

export function useUser(options: { redirect?: boolean } = {}) {
  const { redirect = true } = options;
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);
  const router = useRouter();

  useEffect(() => {
    api<User>("/me")
      .then(setUser)
      .catch((e: unknown) => {
        if (redirect && e instanceof ApiError && e.status === 401) router.push("/login");
      })
      .finally(() => setLoading(false));
  }, [redirect, router]);

  return { user, loading };
}

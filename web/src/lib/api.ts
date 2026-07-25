export class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
  }
}

export async function api<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api${path}`, {
    ...init,
    headers: { "Content-Type": "application/json", ...(init?.headers ?? {}) },
    credentials: "same-origin",
  });
  if (!res.ok) {
    let msg = res.statusText;
    try {
      msg = ((await res.json()) as { error?: string }).error ?? msg;
    } catch {}
    throw new ApiError(res.status, msg);
  }
  // Some endpoints (auth/request-link, logout, order transitions) return an empty
  // 2xx body. Parsing "" as JSON throws "Unexpected end of JSON input", so only
  // parse when there is actually a body to parse.
  const text = await res.text();
  return (text ? JSON.parse(text) : undefined) as T;
}

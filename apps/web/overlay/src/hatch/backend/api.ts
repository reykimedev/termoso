// The Labterm server's REST API (`/api/v1`), as a native client uses it. Self-hosted servers must
// list Hatch's origin in LABTERM_CORS_ORIGINS; the browser refuses the calls otherwise.
import { fail } from "./errors";

export interface ApiErrorBody {
  code?: string;
  message?: string;
  details?: unknown;
}

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message);
  }
}

/** `https://labterm.example.com/` or `…/api/v1` → `https://labterm.example.com`. */
export function normalizeServer(url: string): string {
  let u = url.trim();
  if (!/^https?:\/\//i.test(u)) u = "https://" + u;
  u = u.replace(/\/+$/, "").replace(/\/api\/v1$/, "");
  return u;
}

export async function call<T>(
  server: string,
  method: string,
  path: string,
  body?: unknown,
  token?: string | null,
): Promise<T> {
  let res: Response;
  try {
    res = await fetch(`${server}/api/v1/${path}`, {
      method,
      headers: {
        ...(body !== undefined ? { "content-type": "application/json" } : {}),
        ...(token ? { authorization: `Bearer ${token}` } : {}),
      },
      body: body !== undefined ? JSON.stringify(body) : undefined,
      credentials: "omit",
    });
  } catch {
    fail(
      "network",
      `Cannot reach ${server}. If it is your own server, add this page's origin (${location.origin}) to LABTERM_CORS_ORIGINS.`,
    );
  }
  if (res.status === 204) return undefined as T;
  const text = await res.text();
  const json = text ? (JSON.parse(text) as unknown) : undefined;
  if (!res.ok) {
    const b = (json ?? {}) as ApiErrorBody;
    throw new ApiError(res.status, b.code ?? "http_error", b.message ?? `HTTP ${res.status}`);
  }
  return json as T;
}

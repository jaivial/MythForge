/** Thin fetch wrapper over the MythForge API. */
const BASE = '/api/v1';

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string
  ) {
    super(message);
  }
}

function token(): string | null {
  return localStorage.getItem('mf_token');
}

export function setAuth(t: string | null) {
  if (t) localStorage.setItem('mf_token', t);
  else localStorage.removeItem('mf_token');
}

async function req<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  const t = token();
  if (t) headers.Authorization = `Bearer ${t}`;
  const res = await fetch(`${BASE}${path}`, { ...init, headers });
  if (res.status === 401 || res.status === 403) {
    // Session expired or invalid: drop it and send the user back to login.
    localStorage.removeItem('mf_token');
    localStorage.removeItem('mf_session');
    if (typeof location !== 'undefined' && !location.pathname.startsWith('/login')) {
      location.href = '/login';
    }
  }
  if (!res.ok) {
    let code = 'error';
    let message = res.statusText;
    try {
      const body = await res.json();
      code = body.error ?? code;
      message = body.message ?? message;
    } catch {
      /* non-json */
    }
    throw new ApiError(res.status, code, message);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export const api = {
  get: <T>(p: string) => req<T>(p),
  post: <T>(p: string, body?: unknown) =>
    req<T>(p, { method: 'POST', body: JSON.stringify(body ?? {}) }),
  patch: <T>(p: string, body?: unknown) =>
    req<T>(p, { method: 'PATCH', body: JSON.stringify(body ?? {}) }),
  del: <T>(p: string) => req<T>(p, { method: 'DELETE' })
};

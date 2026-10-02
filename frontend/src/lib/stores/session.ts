/** Session state: token + user + company, persisted to localStorage. */
import { writable } from 'svelte/store';
import type { Session } from '$lib/api/types';
import { setAuth } from '$lib/api/client';

const KEY = 'mf_session';

function load(): Session | null {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? (JSON.parse(raw) as Session) : null;
  } catch {
    return null;
  }
}

export const session = writable<Session | null>(load());

export function setSession(s: Session | null) {
  if (s) {
    localStorage.setItem(KEY, JSON.stringify(s));
    setAuth(s.token);
  } else {
    localStorage.removeItem(KEY);
    setAuth(null);
  }
  session.set(s);
}

export function logout() {
  setSession(null);
}

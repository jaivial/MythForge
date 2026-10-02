/** Domain services, one function per API surface. */
import { api } from './client';
import type {
  Agent, Automation, Blueprint, Mascot, Module, RecordPage, Session, TurnResult
} from './types';

export const auth = {
  signup: (email: string, password: string, name: string, companyName: string, template: string) =>
    api.post<Session>('/auth/signup', {
      email, password, name, company_name: companyName, template
    }),
  login: (email: string, password: string) =>
    api.post<Session & { company_id: string }>('/auth/login', { email, password }),
  me: () => api.get<Record<string, unknown>>('/me')
};

export const workspace = {
  blueprint: () => api.get<Blueprint>('/blueprint'),
  build: (prompt: string) =>
    api.post<{ summary: string; applied: Module[]; blueprint: Blueprint }>('/build', { prompt }),
  catalog: () => api.get<Record<string, unknown>>('/catalog')
};

export const data = {
  list: (module: string, entity: string, params: Record<string, string | number> = {}) => {
    const qs = new URLSearchParams();
    for (const [k, v] of Object.entries(params)) qs.set(k, String(v));
    return api.get<RecordPage>(`/data/${module}/${entity}${qs.size ? `?${qs}` : ''}`);
  },
  get: (module: string, entity: string, id: string) =>
    api.get<Record_>(`/data/${module}/${entity}/${id}`),
  create: (module: string, entity: string, body: Record<string, unknown>) =>
    api.post<Record_>(`/data/${module}/${entity}`, body),
  update: (module: string, entity: string, id: string, body: Record<string, unknown>) =>
    api.patch<Record_>(`/data/${module}/${entity}/${id}`, body),
  remove: (module: string, entity: string, id: string) =>
    api.del<{ deleted: boolean }>(`/data/${module}/${entity}/${id}`)
};

export const mascots = {
  list: () => api.get<{ items: Mascot[] }>('/mascots'),
  create: (m: Partial<Mascot>) => api.post<Mascot>('/mascots', m)
};

export const agents = {
  list: () => api.get<{ items: Agent[] }>('/agents'),
  compose: (prompt: string) => api.post<{ draft: Record<string, unknown>; tools: string[] }>('/agents/compose', { prompt }),
  create: (a: Partial<Agent> & { system_prompt?: string }) => api.post<Agent>('/agents', a),
  run: (slug: string, message: string) => api.post<TurnResult>(`/agents/${slug}/run`, { message })
};

export const automations = {
  list: () => api.get<{ items: Automation[] }>('/automations'),
  compose: (prompt: string) =>
    api.post<{
      draft: {
        name: string;
        description: string;
        trigger: Record<string, unknown> & { kind?: string };
        action: Record<string, unknown> & { prompt?: string };
        agent_id: string | null;
      };
      agents: { id: string; name: string }[];
    }>('/automations/compose', { prompt }),
  create: (a: Partial<Automation>) => api.post<Automation>('/automations', a),
  remove: (id: string) => api.del<{ deleted: boolean }>(`/automations/${id}`),
  runNow: (id: string) => api.post<{ ran: boolean }>(`/automations/${id}/run`)
};

export const chat = {
  send: (message: string) => api.post<TurnResult>('/chat', { message })
};

export const google = {
  status: () =>
    api.get<{ connected: boolean; email?: string | null; configured: boolean }>('/google/status'),
  connect: () => api.get<{ client_id: string; redirect_url: string; scope: string }>('/google/connect')
};

import type { Record_ } from './types';

/** API shapes the UI depends on. */
export type FieldType =
  | 'text' | 'textarea' | 'number' | 'money' | 'date' | 'datetime'
  | 'bool' | 'email' | 'phone' | 'select' | 'multiselect' | 'ref' | 'uuid' | 'attachment';

export interface Field {
  slug: string;
  name: string;
  type: FieldType;
  required: boolean;
  ref_entity?: string | null;
  choices?: string[];
}

export interface View {
  slug: string;
  name: string;
  kind: 'table' | 'kanban' | 'calendar' | 'form' | 'detail' | 'chart' | 'pipeline' | 'stat';
  config?: { group_by?: string | null };
}

export interface Entity {
  slug: string;
  name: string;
  description?: string | null;
  fields: Field[];
  views: View[];
}

export interface Module {
  slug: string;
  name: string;
  icon?: string | null;
  entities: Entity[];
}

export interface Blueprint {
  modules: Module[];
}

export interface Record_ {
  id: string;
  data: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface RecordPage {
  items: Record_[];
  total: number;
  limit: number;
  offset: number;
}

export interface Mascot {
  id: string;
  slug: string;
  name: string;
  tagline?: string | null;
  persona?: string;
  greeting?: string;
  glyph?: string;
  accent?: string;
}

export interface Agent {
  id: string;
  mascot_id?: string | null;
  slug: string;
  name: string;
  description: string;
  tools: string[];
  is_active: boolean;
}

export interface Automation {
  id: string;
  agent_id?: string | null;
  name: string;
  description: string;
  trigger: { kind?: string; module?: string; entity?: string; interval_seconds?: number } &
    Record<string, unknown>;
  action: { kind?: string; prompt?: string } & Record<string, unknown>;
  is_active: boolean;
  last_run_at?: string | null;
  run_count: number;
}

export interface TurnResult {
  text: string;
  tool_calls: Array<{ id: string; name: string; ok: boolean; input: unknown; output: unknown }>;
  rounds: number;
}

export interface Session {
  token: string;
  user: { id: string; email: string; name: string };
  company?: { id: string; name: string; slug: string; template: string; database: string };
  company_id?: string;
}

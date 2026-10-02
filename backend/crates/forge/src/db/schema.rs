//! Control-plane schema (the `forge` database).

/// Applied with `sqlx::raw_sql` at boot; idempotent.
pub const CONTROL_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS app_user (
    id            uuid PRIMARY KEY,
    email         text NOT NULL UNIQUE,
    name          text NOT NULL,
    password_hash text NOT NULL,
    google_sub    text UNIQUE,
    avatar_url    text,
    is_platform_admin boolean NOT NULL DEFAULT false,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS company (
    id            uuid PRIMARY KEY,
    name          text NOT NULL,
    slug          text NOT NULL UNIQUE,
    template      text NOT NULL DEFAULT 'blank',
    database_name text,
    created_by    uuid REFERENCES app_user(id) ON DELETE SET NULL,
    created_at    timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS membership (
    id         uuid PRIMARY KEY,
    user_id    uuid NOT NULL REFERENCES app_user(id) ON DELETE CASCADE,
    company_id uuid NOT NULL REFERENCES company(id) ON DELETE CASCADE,
    role       text NOT NULL DEFAULT 'member',
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (user_id, company_id)
);

-- Mascots: the persona layer. Each mascot can host many agents.
CREATE TABLE IF NOT EXISTS mascot (
    id         uuid PRIMARY KEY,
    company_id uuid NOT NULL REFERENCES company(id) ON DELETE CASCADE,
    slug       text NOT NULL,
    name       text NOT NULL,
    tagline    text,
    persona    text NOT NULL DEFAULT '',
    greeting   text NOT NULL DEFAULT '',
    glyph      text NOT NULL DEFAULT '\u25c6',
    accent     text NOT NULL DEFAULT 'silver',
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (company_id, slug)
);

-- Agents belong to a mascot and carry a system prompt + tool bindings.
CREATE TABLE IF NOT EXISTS agent (
    id            uuid PRIMARY KEY,
    company_id    uuid NOT NULL REFERENCES company(id) ON DELETE CASCADE,
    mascot_id     uuid REFERENCES mascot(id) ON DELETE CASCADE,
    slug          text NOT NULL,
    name          text NOT NULL,
    description   text NOT NULL DEFAULT '',
    system_prompt text NOT NULL DEFAULT '',
    model         text,
    tools         jsonb NOT NULL DEFAULT '[]'::jsonb,
    is_active     boolean NOT NULL DEFAULT true,
    created_from  jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz NOT NULL DEFAULT now(),
    UNIQUE (company_id, slug)
);

-- Automations: schedule or record trigger -> agent run.
CREATE TABLE IF NOT EXISTS automation (
    id          uuid PRIMARY KEY,
    company_id  uuid NOT NULL REFERENCES company(id) ON DELETE CASCADE,
    agent_id    uuid REFERENCES agent(id) ON DELETE CASCADE,
    name        text NOT NULL,
    description text NOT NULL DEFAULT '',
    trigger     jsonb NOT NULL DEFAULT '{}'::jsonb,
    action      jsonb NOT NULL DEFAULT '{}'::jsonb,
    is_active   boolean NOT NULL DEFAULT true,
    last_run_at timestamptz,
    run_count   integer NOT NULL DEFAULT 0,
    created_at  timestamptz NOT NULL DEFAULT now()
);

-- Assistant chat threads (one per company by default, more on demand).
CREATE TABLE IF NOT EXISTS chat (
    id         uuid PRIMARY KEY,
    company_id uuid NOT NULL REFERENCES company(id) ON DELETE CASCADE,
    user_id    uuid REFERENCES app_user(id) ON DELETE SET NULL,
    title      text NOT NULL DEFAULT 'Workspace chat',
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS chat_message (
    id          uuid PRIMARY KEY,
    chat_id     uuid NOT NULL REFERENCES chat(id) ON DELETE CASCADE,
    role        text NOT NULL,
    content     text NOT NULL,
    tool_calls  jsonb NOT NULL DEFAULT '[]'::jsonb,
    created_at  timestamptz NOT NULL DEFAULT now()
);

-- Google OAuth tokens, per user+company.
CREATE TABLE IF NOT EXISTS google_token (
    id            uuid PRIMARY KEY,
    user_id       uuid NOT NULL REFERENCES app_user(id) ON DELETE CASCADE,
    company_id    uuid NOT NULL REFERENCES company(id) ON DELETE CASCADE,
    access_token  text NOT NULL,
    refresh_token text,
    expires_at    timestamptz,
    scope         text,
    email         text,
    created_at    timestamptz NOT NULL DEFAULT now(),
    UNIQUE (user_id, company_id)
);

CREATE TABLE IF NOT EXISTS audit_log (
    id         uuid PRIMARY KEY,
    company_id uuid,
    user_id    uuid,
    action     text NOT NULL,
    detail     jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS audit_company_idx ON audit_log (company_id, created_at DESC);
"#;

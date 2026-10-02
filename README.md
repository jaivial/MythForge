# MythForge

AI-generated ERP & CRM platform. Users prompt the assistant; MythForge builds the
frontend, backend and database of their own ERP/CRM out of reusable components â
per company, at runtime, with no service restarts. Business model reference: Odoo.

- **Backend:** Rust 100% (axum + sqlx/PostgreSQL + Anthropic client SDK via `misanthropy`)
- **Frontend:** SvelteKit + Tailwind + shadcn-svelte style reusable components
- **Database:** PostgreSQL, one database per company, provisioned at runtime
- **Agents:** mascots â agents â custom tools bound to generated endpoints/tables
- **Automations:** prompt-created schedules and record triggers
- **Google:** OAuth2 (Calendar + Gmail)
- **QA:** e2e.tester.army (`mini-agent-rs e2e`) with `glm-5.3-flash`

Core logic ported from `~/SAGE/MythAgent`, `~/SAGE/MythCortex`, `~/code/Neural.Workspace`.

## How it works

```
prompt ââ¶ /api/v1/build ââ¶ generator (Anthropic Messages API) ââ¶ blueprint JSON
                                                                 â
                        validated against the component catalog     â¼
                                                                 â
   tenant DB (co_<slug>_<hex>) ââ apply_blueprint ââ mf_module / mf_entity /
   mf_field / mf_view rows + a real table per entity (jsonb mf_record fallback)

   every request ââ¶ generic /api/v1/data/:module/:entity ââº the tenant pool
```

- **No restarts.** The blueprint lives in the tenant database; the generic data
  plane resolves any module/entity for any company from the same binary.
- **Reusable backend paths.** One route set serves every company: the JWT
  carries the company, the pool cache resolves its database.
- **Reusable frontend.** The SPA renders whatever blueprint it fetches â table,
  kanban, calendar, form, detail, chart, pipeline and stat views come from the
  component catalog, not from per-customer code.
- **Agents and tools.** Mascots are personas; agents live under them. Agents get
  tools generated from the same operation registry the assistant uses, so an
  agent can read/write exactly the endpoints and tables of its company.
- **Automations.** `schedule` (interval) and `record_created`/`record_updated`
  triggers run an agent turn and log into `mf_automation_run`.

## Repository layout

| Path | What |
|---|---|
| `backend/crates/misanthropy` | Vendored Anthropic client SDK fork (custom base URL) |
| `backend/crates/forge` | The API: auth, tenant provisioning, generator, data plane, agents, automations, Google |
| `frontend/src/lib/components/ui` | Reusable shadcn-style atoms (dark/silver palette) |
| `frontend/src/routes` | Login, app shell, build, module/entity views, agents, settings |
| `e2e/` + `e2e.yaml` | AI e2e tests (e2e.tester.army, glm-5.3-flash) |
| `deploy/` | docker compose (postgres + api + web nginx), edge vhost docs |

## Running locally

```bash
# backend
cd backend && cp .env.example .env    # fill ANTHROPIC_API_KEY (z.ai works)
~/.cargo/bin/cargo +1.96.1 run --bin forge-server

# frontend (dev proxy to :18092)
cd frontend && npm ci && npm run dev

# e2e against the deployed app
~/mini-tui/agent-rs/target/release/mini-agent-rs e2e run -j 2
```

## Production

Served at **https://forge.myth.services** â Cloudflare (proxied A record, flexible
SSL) â host nginx `:80` â `mythforge-web` container â static SPA + `/api` proxy to
`mythforge-api` â PostgreSQL. See `deploy/README.md`.

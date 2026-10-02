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

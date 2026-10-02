# MythForge backend (Rust)

100% Rust workspace.

## Crates

| Crate | Purpose |
|---|---|
| `crates/misanthropy` | Vendored fork of the Anthropic client SDK (`misanthropy` 0.0.8, MIT) with `with_base_url()` so the Messages API client can point at any Anthropic-compatible gateway (z.ai GLM, MiniMax, api.anthropic.com). |
| `crates/forge` | The API server: axum + sqlx + PostgreSQL + agent loop. |

## Run (docker compose)

```bash
cp .env.example .env   # fill ANTHROPIC_API_KEY
docker compose -f deploy/docker-compose.yml up -d --build
curl -s localhost:8090/healthz
```

## Run (bare metal)

```bash
cargo run --bin forge-server
```

## Layout

```
crates/forge/src
âââ ai/            Anthropic client wrapper + agent tool-use loop
âââ agents/        operation registry (custom tools), assistant chat
âââ api/           axum routes, extractors, state
âââ auth/          argon2id + JWT
âââ automations/   scheduler + trigger engine
âââ db/            control-plane schema, per-company DB provisioning
âââ forge/         component catalog, blueprint generator, generic runtime
âââ google/        OAuth2 + Calendar/Gmail
âââ error.rs       one error type -> HTTP mapping
```

## The no-restart architecture

Every company gets its own PostgreSQL database, created at runtime
(`db/tenant.rs::provision`). Generated modules are served by ONE set of
reusable HTTP paths (`forge/runtime.rs`) that resolve the company's own
metadata at request time, so adding a module/entity/field never requires a
deploy or a restart â for any company, for any user request.

# Deploying MythForge

## Stack

```
Cloudflare (flexible SSL, proxied A record forge.myth.services)
  â host nginx :80 (sites-available/forge.myth.services)
    â docker mythforge-web (nginx :18443)
      â static SvelteKit SPA (frontend/build)
      â /api/* â mythforge-api (forge-server :8080) â postgres :5432
```

One database is provisioned per company at signup (`co_<slug>_<hex>`), created
by the API through `ADMIN_DATABASE_URL`. Nothing restarts when a company builds
new modules: the blueprint lives in the tenant database and the generic
`/data/:module/:entity` routes serve every company from the same binary.

## Build and run

```bash
# Requires Node 20+ (npm ci uses the committed package-lock.json).
cd frontend && npm ci && npm run build     # static bundle mounted by web
cd ../deploy
cp .env.docker .env                        # fill ANTHROPIC_API_KEY, JWT_SECRET
docker compose up -d --build
curl http://127.0.0.1:18443/healthz
```

Services:

| service   | container        | host port | purpose                         |
|-----------|------------------|-----------|---------------------------------|
| postgres  | mythforge-postgres | 55440   | control DB + one DB per company |
| api       | mythforge-api    | 18093     | Rust `forge-server`             |
| web       | mythforge-web    | 18443     | nginx: SPA + `/api` proxy       |

## Google OAuth

`GOOGLE_REDIRECT_URL` must be the public URL, e.g.
`https://forge.myth.services/api/v1/google/callback`, and must be registered in
the Google Cloud console. `APP_URL` is used for links; it does not derive the
redirect automatically.

- DNS: `forge.myth.services` A â host IP, proxied (Cloudflare zone
  `myth.services`, SSL mode flexible).
- Host nginx vhost: `/etc/nginx/sites-available/forge.myth.services` proxying
  to `127.0.0.1:18443` with 300 s read timeouts (AI turns are slow).

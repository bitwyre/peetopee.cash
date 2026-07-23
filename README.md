# peetopee.cash

P2P cash delivery: order cash in your local currency (IDR, AED, EUR, GBP, RUB,
INR, USD, CAD), a courier brings it to you, you pay the courier USDT
(TRC20/BEP20/ERC20) at the meetup — verified on-chain.

## Stack

- `api/` — Rust (Axum + SQLx + Postgres) + USDT chain watcher
- `web/` — Next.js (App Router, Tailwind, Leaflet)
- `deploy/`, `compose.yml` — Caddy + Docker Compose for a single droplet

## Local development

    docker compose -f compose.dev.yml up -d          # Postgres on :5433
    cd api && DATABASE_URL=postgres://peetopee:peetopee@localhost:5433/peetopee cargo run
    cd web && npm install && npm run dev             # http://localhost:3000, /api proxied to :8080

Without RESEND_API_KEY set, magic-link URLs are printed to the API logs.

    cd api && DATABASE_URL=postgres://peetopee:peetopee@localhost:5433/peetopee cargo test
    cd web && npm run build

## Deploy

See [docs/deploy.md](docs/deploy.md). Spec and plan live in `docs/superpowers/`.

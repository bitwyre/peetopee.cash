# peetopee.cash — P2P Cash Delivery Design

**Date:** 2026-07-17
**Status:** Approved by Dendi (brainstorming session)

## What it is

A web service where a customer orders physical cash delivered to their location by a
courier. At the meetup the customer pays the courier in USDT (on-chain, verified by the
backend), and the courier hands over the cash. Peer-to-peer: any registered user can act
as customer or courier. No KYC (orders are under the 10,000 USD threshold — the app
enforces nothing about limits in MVP; this is a policy note, not a feature).

- **Domain:** peetopee.cash (DNS on Cloudflare)
- **Hosting:** one DigitalOcean droplet under Bitwyre's DO team
- **Stack:** Next.js frontend, Rust backend (Axum + SQLx), Postgres, Docker Compose, Caddy

## Product decisions (from brainstorming)

| Decision | Choice |
|---|---|
| Courier side | In-app courier role; open signup, no admin vetting |
| Supported fiat | IDR, AED, EUR, GBP, RUB, INR, USD, CAD |
| Pricing | Customer proposes cash amount + USDT ask; courier sees implied rate and accepts |
| USDT payment | Customer's own wallet → courier's address; backend verifies on-chain |
| Networks | TRC20 (Tron), BEP20 (BSC), ERC20 (Ethereum) |
| Auth | Email magic links (no passwords), sent via Resend |
| Profile | Telegram handle required; shared between parties once an order is accepted |
| Location | Free-text address + draggable Leaflet/OpenStreetMap pin (lat/lng stored) |
| Escrow | None — trust is face-to-face; app verifies payment landed |

## Architecture

```
Cloudflare DNS (proxied) → Droplet

┌─ Droplet (Docker Compose) ─────────────┐
│  caddy :443  (TLS via CF DNS challenge)│
│   ├─ /api/* → rust-api :8080           │
│   └─ /*     → nextjs   :3000           │
│  rust-api ─→ postgres :5432            │
│  rust-api ─→ chain watcher (Tokio task)│
│              → TronGrid (TRC20)        │
│              → Etherscan V2 (ERC20,    │
│                BEP20 — one API key)    │
└────────────────────────────────────────┘
```

## Data model (Postgres)

**users**
- `id` uuid PK, `email` text unique, `telegram_handle` text (required after onboarding)
- `usdt_trc20`, `usdt_bep20`, `usdt_erc20` — nullable text. A user must have ≥1 set
  before they can accept orders as a courier.
- `created_at`

**login_tokens** — magic links
- `id`, `email`, `token_hash` (SHA-256 of the random token), `expires_at` (15 min),
  `used_at` nullable. Verifying a token for an unknown email creates the user.

**sessions**
- `id`, `user_id`, `token_hash`, `expires_at` (30 days). Backs an
  `HttpOnly; Secure; SameSite=Lax` cookie.

**orders**
- `id` uuid PK, `customer_id` FK, `courier_id` FK nullable
- `fiat_currency` — enum: IDR, AED, EUR, GBP, RUB, INR, USD, CAD
- `fiat_amount` numeric, `usdt_amount` numeric
- `address_text` text, `lat` / `lng` double
- `status` — enum: OPEN, ACCEPTED, AWAITING_PAYMENT, PAID, COMPLETED, CANCELLED
- `payment_network` (trc20|bep20|erc20, set when payment requested),
  `payment_txid` nullable, `paid_at` nullable
- `payment_requested_at` nullable — anchors chain matching
- `created_at`, `accepted_at`, `completed_at`, `cancelled_at`

## Order lifecycle

1. **OPEN** — customer submits: fiat currency + amount, USDT ask, address + pin.
   Order appears on the courier board.
2. **ACCEPTED** — a courier (with ≥1 USDT address, not the customer themself) accepts.
   Both parties now see each other's Telegram handle (deep link `t.me/<handle>`) and
   coordinate the meetup off-app.
3. **AWAITING_PAYMENT** — at the meetup the courier taps "Request payment".
   `payment_requested_at` is stamped. Customer sees the courier's USDT address per
   available network, with QR codes, and picks a network to pay on.
4. **PAID** — the chain watcher observes an incoming USDT transfer to the courier's
   address with `amount ≥ usdt_amount` and block time > `payment_requested_at`.
   Status flips automatically; `payment_txid` stored.
5. **COMPLETED** — customer taps "Cash received".

**Cancel rules:** customer may cancel while OPEN; either party may cancel while
ACCEPTED. While AWAITING_PAYMENT, cancellation is blocked for the first 2 hours after
`payment_requested_at` (payment may be in flight); after that, either party may cancel.
Orders PAID or later cannot be cancelled.

## Rust backend (Axum + SQLx, one binary)

### HTTP API (`/api`)

| Route | Auth | Purpose |
|---|---|---|
| `POST /api/auth/request-link` | — | Body `{email}`. Creates login_token, emails link via Resend. Rate-limited per email + per IP. Always returns 200 (no user enumeration). |
| `GET /api/auth/verify?token=` | — | Validates token, creates user if new, sets session cookie, redirects to `/onboarding` (new, or missing telegram) or `/orders`. |
| `POST /api/auth/logout` | ✓ | Clears session. |
| `GET /api/me` | ✓ | Profile. |
| `PATCH /api/me` | ✓ | Update telegram_handle, USDT addresses (format-validated per network: T… base58 for TRC20, 0x…40-hex for BEP20/ERC20). |
| `POST /api/orders` | ✓ | Create order (validates currency ∈ 8, amounts > 0). |
| `GET /api/orders/mine` | ✓ | Orders where user is customer or courier. |
| `GET /api/orders/open` | ✓ | Courier board: all OPEN orders not owned by caller. |
| `GET /api/orders/:id` | ✓ | Detail. Telegram handles and USDT addresses only revealed to the two parties, post-acceptance. |
| `POST /api/orders/:id/accept` | ✓ | Courier accepts. Guards: order OPEN, caller ≠ customer, caller has ≥1 USDT address. |
| `POST /api/orders/:id/request-payment` | ✓ | Courier only, ACCEPTED → AWAITING_PAYMENT. |
| `POST /api/orders/:id/confirm-cash` | ✓ | Customer only, PAID → COMPLETED. |
| `POST /api/orders/:id/cancel` | ✓ | Per cancel rules above. |

State transitions are enforced in SQL (`UPDATE … WHERE status = $expected`) so
concurrent accepts can't double-assign a courier.

### Chain watcher

Tokio background task, every ~20s:

1. Load orders in AWAITING_PAYMENT.
2. Per order, query the payment network for USDT transfers **to** the courier's address:
   - **TRC20:** TronGrid `GET /v1/accounts/{addr}/transactions/trc20?contract_address=<USDT>` 
   - **ERC20/BEP20:** Etherscan V2 multichain `module=account&action=tokentx` with
     `chainid` 1 / 56 and the USDT contract for each chain.
3. Match: `amount ≥ usdt_amount` (in token decimals — 6 on Tron/Ethereum, 18 on BSC)
   and timestamp > `payment_requested_at`, and txid not already claimed by another order.
4. On match: atomically AWAITING_PAYMENT → PAID with txid.

Matching logic is a pure function (inputs: transfer list, order) → unit-testable with
canned API fixtures. API failures log and retry next tick; the watcher never crashes
the process.

## Frontend (Next.js App Router)

| Page | Purpose |
|---|---|
| `/` | Landing: what the service is, CTA to sign up. |
| `/login` | Email input → "check your inbox". |
| `/onboarding` | First login: set Telegram handle (required), optionally USDT addresses. |
| `/orders/new` | Order form: currency select (8), cash amount, USDT ask (implied rate shown live), address text + Leaflet map with draggable pin (browser geolocation pre-fills). |
| `/orders` | My orders, both roles, grouped by active/past. |
| `/orders/[id]` | Status timeline, other party's Telegram deep-link, payment panel (courier: "Request payment"; customer: addresses + QR per network, live status poll), confirm/cancel actions. |
| `/courier` | Open-orders board: amount, currency, implied rate, rough distance (if geolocation granted). Accept button. |
| `/settings` | Edit Telegram handle + USDT addresses. |

Client polls order status (~5s on the order detail page) — no websockets in MVP.
QR codes rendered client-side (address-only payloads).

## Deployment

- **Droplet:** Ubuntu 24.04, 1 vCPU / 2 GB (`s-1vcpu-2gb`), created under Bitwyre's DO
  team. `doctl` commands documented; Dendi runs them (needs his DO auth).
- **Compose services:** `caddy` (TLS via Cloudflare DNS-01 challenge — needs a CF API
  token scoped to peetopee.cash), `web` (Next.js standalone), `api` (Rust, multi-stage
  distroless-ish build), `db` (postgres:16, volume-backed).
- **DNS:** Cloudflare A record `peetopee.cash` → droplet IP, proxied.
- **Env (`.env` on droplet, never committed):** `DATABASE_URL`, `SESSION_SECRET`,
  `RESEND_API_KEY`, `ETHERSCAN_API_KEY`, `TRONGRID_API_KEY` (optional but recommended),
  `CF_API_TOKEN`, `BASE_URL=https://peetopee.cash`.
- **Deploy:** `deploy.sh` — git pull on droplet, `docker compose build && up -d`,
  sqlx migrations run on API startup.

## Testing

- **Rust:** integration tests against a test Postgres (auth flow, full order lifecycle,
  transition guards, authorization — non-party can't read an order); unit tests for the
  chain-matching function with canned TronGrid/Etherscan JSON fixtures.
- **Frontend:** `next build` + TypeScript strict as the gate. No browser test suite in MVP.

## Out of scope (MVP)

Escrow/custody, KYC, order limits enforcement, ratings/reputation, disputes flow,
push/email notifications beyond magic links, websockets, admin dashboard, courier
vetting, fee/spread collection, mobile apps.

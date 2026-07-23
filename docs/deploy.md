# Deploying peetopee.cash

## 1. Create the droplet (Bitwyre DO team)

    doctl auth init --context bitwyre       # one-time
    doctl compute ssh-key list --context bitwyre
    doctl compute droplet create peetopee \
      --context bitwyre \
      --region sgp1 \
      --size s-1vcpu-2gb \
      --image ubuntu-24-04-x64 \
      --ssh-keys <YOUR_KEY_ID> \
      --wait
    doctl compute droplet list --context bitwyre --format Name,PublicIPv4

## 2. Cloudflare DNS

- A record: `peetopee.cash` → droplet IP, **proxied** (orange cloud).
- SSL/TLS mode: **Full (strict)**.
- Create an API token: Zone → DNS → Edit, scoped to `peetopee.cash` → goes in `.env` as `CF_API_TOKEN` (used by Caddy for DNS-01 certificates).

## 3. Provision the droplet

    ssh root@<IP>
    apt-get update && apt-get install -y git docker.io docker-compose-v2
    git clone <REPO_URL> /opt/peetopee
    cd /opt/peetopee
    cp .env.example .env && nano .env    # fill all secrets
    docker compose up -d --build

First boot: the API runs sqlx migrations automatically; Caddy fetches certs via
Cloudflare DNS-01.

## 4. Secrets

- `RESEND_API_KEY`: resend.com → verify domain peetopee.cash (DNS records in Cloudflare) → create key.
- `ETHERSCAN_API_KEY`: etherscan.io account → API key (V2 key covers Ethereum + BSC).
- `TRONGRID_API_KEY`: trongrid.io account (optional but avoids rate limits).

## 5. Redeploys

    DROPLET_IP=<IP> ./deploy.sh

## 6. Logs

    ssh root@<IP> 'cd /opt/peetopee && docker compose logs -f --tail 100 api'

#!/usr/bin/env bash
# Redeploy peetopee.cash: usage: DROPLET_IP=x.x.x.x ./deploy.sh
set -euo pipefail
: "${DROPLET_IP:?set DROPLET_IP}"
# NOTE: `docker image prune -f` only removes dangling images. Do NOT use
# `docker system prune` here — it wipes the BuildKit cache and forces a cold
# Rust recompile (~13 min) on every deploy.
ssh "root@${DROPLET_IP}" 'set -e; cd /opt/peetopee; git pull --ff-only; docker compose build; docker compose up -d; docker image prune -f'
echo "deployed to https://peetopee.cash"

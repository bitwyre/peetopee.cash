#!/usr/bin/env bash
# Redeploy peetopee.cash: usage: DROPLET_IP=x.x.x.x ./deploy.sh
set -euo pipefail
: "${DROPLET_IP:?set DROPLET_IP}"
ssh "root@${DROPLET_IP}" 'set -e; cd /opt/peetopee; git pull; docker compose build; docker compose up -d; docker system prune -f'
echo "deployed to https://peetopee.cash"

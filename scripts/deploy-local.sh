#!/usr/bin/env bash
# Deploys the backend and frontend to a local icp-cli network. The network
# already runs the ICP ledger (ICRC-2 enabled) and seeds local identities with
# ICP; sign-in uses mainnet Internet Identity, which the local network trusts.
# Usage: ./scripts/deploy-local.sh   (starts the network if it isn't running)
set -euo pipefail
cd "$(dirname "$0")/.."

icp network status >/dev/null 2>&1 || icp network start -d
icp deploy

echo
echo "Frontend: http://$(icp canister status poker_frontend --id-only).localhost:8000"
echo "Fund a player with: ./scripts/fund.sh <principal> <icp>"

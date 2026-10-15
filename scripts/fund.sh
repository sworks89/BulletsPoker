#!/usr/bin/env bash
# Sends local test ICP from your dfx identity to a principal (e.g. your
# Internet Identity principal shown on the Wallet page).
# Usage: ./scripts/fund.sh <principal> [icp=10]
set -euo pipefail
PRINCIPAL=$1
AMOUNT=${2:-10}
dfx ledger transfer --icp "$AMOUNT" --memo 0 "$(dfx ledger account-id --of-principal "$PRINCIPAL")"

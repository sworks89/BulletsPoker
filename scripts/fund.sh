#!/usr/bin/env bash
# Sends local test ICP from your icp identity to a principal (e.g. your
# Internet Identity principal shown on the Wallet page).
# Usage: ./scripts/fund.sh <principal> [icp=10]
set -euo pipefail
PRINCIPAL=$1
AMOUNT=${2:-10}
icp token transfer "$AMOUNT" "$PRINCIPAL"

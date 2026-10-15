#!/usr/bin/env bash
# Deploys everything to a local replica: a local ICP ledger (with ICRC-2),
# Internet Identity, the poker backend and the frontend.
# Usage: dfx start --background --clean && ./scripts/deploy-local.sh
set -euo pipefail
cd "$(dirname "$0")/.."

if ! dfx identity list | grep -q '^minter$'; then
  dfx identity new minter --storage-mode plaintext
fi
MINTER_ACCOUNT=$(dfx ledger account-id --identity minter)
ME_ACCOUNT=$(dfx ledger account-id)

dfx deploy icp_ledger --argument "(variant { Init = record {
  minting_account = \"$MINTER_ACCOUNT\";
  initial_values = vec { record { \"$ME_ACCOUNT\"; record { e8s = 100_000_000_000 : nat64 } } };
  send_whitelist = vec {};
  transfer_fee = opt record { e8s = 10_000 : nat64 };
  token_symbol = opt \"ICP\";
  token_name = opt \"Local ICP\";
  feature_flags = opt record { icrc2 = true };
} })"

dfx deploy internet_identity
dfx deploy poker_backend --argument '(opt record { ledger = opt principal "ryjl3-tyaaa-aaaaa-aaaba-cai"; ledger_fee = opt 10_000 })'
dfx deploy poker_frontend

echo
echo "Frontend: http://$(dfx canister id poker_frontend).localhost:4943"
echo "Fund a player with: ./scripts/fund.sh <principal> <icp>"

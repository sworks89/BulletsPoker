import { Actor, HttpAgent, type ActorSubclass, type Identity } from '@icp-sdk/core/agent';
import { safeGetCanisterEnv } from '@icp-sdk/core/agent/canister-env';
import { AuthClient } from '@icp-sdk/auth/client';
import { type IDL } from '@icp-sdk/core/candid';
import { Principal } from '@icp-sdk/core/principal';
import { idlFactory } from './declarations/poker_backend/poker_backend.did.js';
import type { _SERVICE } from './declarations/poker_backend/poker_backend.did';

export type Backend = ActorSubclass<_SERVICE>;
export type * from './declarations/poker_backend/poker_backend.did';

// The frontend canister (or the Vite dev server) sets the `ic_env` cookie with
// the canister ids of this environment and the network's root key, so one build
// works locally and on mainnet. Without the cookie, the agent falls back to the
// mainnet root key.
const canisterEnv = safeGetCanisterEnv();
const rootKey = canisterEnv?.IC_ROOT_KEY;

export const backendId: string = canisterEnv?.['PUBLIC_CANISTER_ID:poker_backend'] ?? '';
// The ICP ledger has the same id on mainnet and on an icp-cli local network.
export const ledgerId = 'ryjl3-tyaaa-aaaaa-aaaba-cai';

// Mainnet Internet Identity (id.ai) everywhere: a local network trusts mainnet
// signatures, so its delegations work against a locally deployed backend too.
// The client mints delegations by calling the II canister, which only exists on
// mainnet, so its agent always targets mainnet (with the mainnet root key) even
// when the page itself is served by a local network.
export const authClient = new AuthClient({ agentOptions: { host: 'https://icp-api.io' } });

// Minimal ICRC-1/ICRC-2 interface of the ICP ledger, used to approve deposits.
const ledgerIdl: IDL.InterfaceFactory = ({ IDL }) => {
  const Account = IDL.Record({ owner: IDL.Principal, subaccount: IDL.Opt(IDL.Vec(IDL.Nat8)) });
  const ApproveArgs = IDL.Record({
    fee: IDL.Opt(IDL.Nat),
    memo: IDL.Opt(IDL.Vec(IDL.Nat8)),
    from_subaccount: IDL.Opt(IDL.Vec(IDL.Nat8)),
    created_at_time: IDL.Opt(IDL.Nat64),
    amount: IDL.Nat,
    expected_allowance: IDL.Opt(IDL.Nat),
    expires_at: IDL.Opt(IDL.Nat64),
    spender: Account,
  });
  const ApproveError = IDL.Variant({
    GenericError: IDL.Record({ message: IDL.Text, error_code: IDL.Nat }),
    TemporarilyUnavailable: IDL.Null,
    Duplicate: IDL.Record({ duplicate_of: IDL.Nat }),
    BadFee: IDL.Record({ expected_fee: IDL.Nat }),
    AllowanceChanged: IDL.Record({ current_allowance: IDL.Nat }),
    CreatedInFuture: IDL.Record({ ledger_time: IDL.Nat64 }),
    TooOld: IDL.Null,
    Expired: IDL.Record({ ledger_time: IDL.Nat64 }),
    InsufficientFunds: IDL.Record({ balance: IDL.Nat }),
  });
  return IDL.Service({
    icrc1_balance_of: IDL.Func([Account], [IDL.Nat], ['query']),
    icrc2_approve: IDL.Func([ApproveArgs], [IDL.Variant({ Ok: IDL.Nat, Err: ApproveError })], []),
  });
};

type Account = { owner: Principal; subaccount: [] | [Uint8Array | number[]] };
export interface Ledger {
  icrc1_balance_of: (a: Account) => Promise<bigint>;
  icrc2_approve: (args: {
    fee: [] | [bigint];
    memo: [] | [Uint8Array];
    from_subaccount: [] | [Uint8Array];
    created_at_time: [] | [bigint];
    amount: bigint;
    expected_allowance: [] | [bigint];
    expires_at: [] | [bigint];
    spender: Account;
  }) => Promise<{ Ok: bigint } | { Err: Record<string, unknown> }>;
}

export async function makeActors(identity?: Identity): Promise<{ backend: Backend; ledger: Ledger }> {
  if (!backendId) throw new Error('No backend canister id: serve the app from its frontend canister (icp deploy) or the Vite dev server.');
  // No host: the agent resolves the page's own gateway (localhost or IC
  // boundary nodes). Never fetch a root key at runtime.
  const agent = await HttpAgent.create({ identity, rootKey });
  const backend = Actor.createActor<_SERVICE>(idlFactory, { agent, canisterId: backendId });
  const ledger = Actor.createActor(ledgerIdl, { agent, canisterId: ledgerId }) as unknown as Ledger;
  return { backend, ledger };
}

/** Unwraps a Candid `Result`, throwing the error text. */
export function unwrap<T>(r: { Ok: T } | { Err: string }): T {
  if ('Err' in r) throw new Error(r.Err);
  return r.Ok;
}

export { Principal };

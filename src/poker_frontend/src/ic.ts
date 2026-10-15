import { Actor, HttpAgent, type ActorSubclass, type Identity } from '@icp-sdk/core/agent';
import { AuthClient } from '@icp-sdk/auth/client';
import { type IDL } from '@icp-sdk/core/candid';
import { Principal } from '@icp-sdk/core/principal';
import { idlFactory } from './declarations/poker_backend/poker_backend.did.js';
import type { _SERVICE } from './declarations/poker_backend/poker_backend.did';

export type Backend = ActorSubclass<_SERVICE>;
export type * from './declarations/poker_backend/poker_backend.did';

const env = import.meta.env;
export const network: string = env.DFX_NETWORK ?? 'local';
const isLocal = network !== 'ic';
const host = isLocal ? 'http://127.0.0.1:4943' : 'https://icp-api.io';

export const backendId: string = env.CANISTER_ID_POKER_BACKEND ?? env.VITE_BACKEND_CANISTER_ID ?? '';
export const ledgerId: string = env.CANISTER_ID_ICP_LEDGER ?? 'ryjl3-tyaaa-aaaaa-aaaba-cai';
const iiId: string | undefined = env.CANISTER_ID_INTERNET_IDENTITY;

export const authClient = new AuthClient(
  isLocal && iiId
    ? {
        identityProvider: {
          authorizeUrl: env.VITE_II_URL ?? `http://${iiId}.localhost:4943/authorize`,
          canisterId: iiId,
        },
      }
    : {},
);

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
  const agent = await HttpAgent.create({ host, identity, shouldFetchRootKey: isLocal });
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

//! ICP ledger integration (ICRC-1 / ICRC-2) and deposit account helpers.

use candid::{Nat, Principal};
use crate::wallet::{LedgerOp, Outcome};
use ic_cdk::call::{Call, CallFailed, RejectCode};
use icrc_ledger_types::icrc1::account::{Account, Subaccount};
use icrc_ledger_types::icrc1::transfer::{Memo, TransferArg, TransferError};
use icrc_ledger_types::icrc2::transfer_from::{TransferFromArgs, TransferFromError};
use sha2::{Digest, Sha224};

/// The per-user deposit subaccount of this canister: `[len, principal bytes, 0...]`.
pub fn deposit_subaccount(user: &Principal) -> Subaccount {
    let bytes = user.as_slice();
    let mut sub = [0u8; 32];
    sub[0] = bytes.len() as u8;
    sub[1..1 + bytes.len()].copy_from_slice(bytes);
    sub
}

/// Legacy ICP account identifier (hex), still used by exchanges and some wallets.
pub fn account_identifier(owner: &Principal, sub: &Subaccount) -> String {
    let mut h = Sha224::new();
    h.update(b"\x0Aaccount-id");
    h.update(owner.as_slice());
    h.update(sub);
    let hash = h.finalize();
    let mut out = crc32(&hash).to_be_bytes().to_vec();
    out.extend_from_slice(&hash);
    out.iter().map(|b| format!("{b:02x}")).collect()
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn to_u64(n: &Nat) -> Result<u64, String> {
    n.0.clone().try_into().map_err(|_| "Amount too large".to_string())
}

pub async fn balance_of(ledger: Principal, account: Account) -> Result<u64, String> {
    let res = Call::bounded_wait(ledger, "icrc1_balance_of")
        .with_arg(account)
        .await
        .map_err(|e| format!("Ledger unavailable: {e}"))?;
    let n: Nat = res.candid().map_err(|e| e.to_string())?;
    to_u64(&n)
}

/// Classifies a failed inter-canister call. With unbounded waits, every reject
/// except `SysUnknown` means the ledger never executed the request.
fn call_error(e: CallFailed) -> Outcome {
    match e {
        CallFailed::CallRejected(r) => match r.reject_code() {
            Ok(RejectCode::SysUnknown) | Err(_) => Outcome::Unknown(format!("Ledger reply unknown: {r}")),
            Ok(_) => Outcome::Failed(format!("Ledger rejected the call: {r}")),
        },
        other => Outcome::Failed(format!("Ledger call could not be sent: {other}")),
    }
}

fn block(n: &Nat) -> u64 {
    to_u64(n).unwrap_or(u64::MAX)
}

/// Sends an ICRC-1 transfer for `op` (withdrawals, sweeps, treasury). Re-sending
/// the same op is safe: the ledger deduplicates on `created_at_time` + memo.
pub async fn transfer_op(ledger: Principal, op: &LedgerOp) -> Outcome {
    let arg = TransferArg {
        from_subaccount: op.from_subaccount,
        to: op.to,
        fee: Some(Nat::from(op.fee)),
        created_at_time: Some(op.created_at_time),
        memo: Some(Memo(op.memo().into())),
        amount: Nat::from(op.amount),
    };
    let res = match Call::unbounded_wait(ledger, "icrc1_transfer").with_arg(arg).await {
        Ok(r) => r,
        Err(e) => return call_error(e),
    };
    match res.candid::<Result<Nat, TransferError>>() {
        // The ledger answered but we can't read it: the transfer may have happened.
        Err(e) => Outcome::Unknown(format!("Could not decode the ledger reply: {e}")),
        Ok(Ok(n)) => Outcome::Ok(block(&n)),
        // A retry of a transfer that already went through.
        Ok(Err(TransferError::Duplicate { duplicate_of })) => Outcome::Ok(block(&duplicate_of)),
        // Outside the dedup window the ledger can't tell us; an admin must check.
        Ok(Err(TransferError::TooOld)) => Outcome::Unknown("Too old for the ledger to deduplicate".into()),
        Ok(Err(e)) => Outcome::Failed(format!("Transfer failed: {e}")),
    }
}

/// Pulls an ICRC-2 deposit for `op` from the player's wallet.
pub async fn transfer_from_op(ledger: Principal, op: &LedgerOp) -> Outcome {
    let Some(from) = op.from else {
        return Outcome::Failed("Deposit has no source account".into());
    };
    let arg = TransferFromArgs {
        spender_subaccount: None,
        from,
        to: op.to,
        amount: Nat::from(op.amount),
        fee: Some(Nat::from(op.fee)),
        memo: Some(Memo(op.memo().into())),
        created_at_time: Some(op.created_at_time),
    };
    let res = match Call::unbounded_wait(ledger, "icrc2_transfer_from").with_arg(arg).await {
        Ok(r) => r,
        Err(e) => return call_error(e),
    };
    match res.candid::<Result<Nat, TransferFromError>>() {
        Err(e) => Outcome::Unknown(format!("Could not decode the ledger reply: {e}")),
        Ok(Ok(n)) => Outcome::Ok(block(&n)),
        Ok(Err(TransferFromError::Duplicate { duplicate_of })) => Outcome::Ok(block(&duplicate_of)),
        Ok(Err(TransferFromError::TooOld)) => Outcome::Unknown("Too old for the ledger to deduplicate".into()),
        Ok(Err(TransferFromError::InsufficientAllowance { .. })) => {
            Outcome::Failed("Approve the deposit in your wallet first".into())
        }
        Ok(Err(TransferFromError::InsufficientFunds { .. })) => Outcome::Failed("Not enough ICP in your wallet".into()),
        Ok(Err(e)) => Outcome::Failed(format!("Deposit failed: {e:?}")),
    }
}

pub async fn raw_rand() -> Result<Vec<u8>, String> {
    let res = Call::bounded_wait(Principal::management_canister(), "raw_rand")
        .await
        .map_err(|e| e.to_string())?;
    res.candid::<Vec<u8>>().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn account_id_known_value() {
        // Account identifier of the anonymous principal's default account.
        let id = account_identifier(&Principal::anonymous(), &[0; 32]);
        assert_eq!(id, "1c7a48ba6a562aa9eaa2481a9049cdf0433b9738c992d698c31d8abf89cadc79");
    }

    #[test]
    fn subaccount_roundtrip() {
        let p = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
        let sub = deposit_subaccount(&p);
        let len = sub[0] as usize;
        assert_eq!(Principal::from_slice(&sub[1..1 + len]), p);
    }
}

//! ICP ledger integration (ICRC-1 / ICRC-2) and deposit account helpers.

use candid::{Nat, Principal};
use ic_cdk::call::Call;
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

pub async fn transfer(
    ledger: Principal,
    from_subaccount: Option<Subaccount>,
    to: Account,
    amount: u64,
    fee: u64,
    memo: &[u8],
) -> Result<u64, String> {
    let arg = TransferArg {
        from_subaccount,
        to,
        fee: Some(Nat::from(fee)),
        created_at_time: None,
        memo: Some(Memo(memo.to_vec().into())),
        amount: Nat::from(amount),
    };
    // Unbounded wait: we must learn the outcome to keep balances correct.
    let res = Call::unbounded_wait(ledger, "icrc1_transfer")
        .with_arg(arg)
        .await
        .map_err(|e| format!("Ledger call failed: {e}"))?;
    let r: Result<Nat, TransferError> = res.candid().map_err(|e| e.to_string())?;
    let block = r.map_err(|e| format!("Transfer failed: {e}"))?;
    to_u64(&block)
}

pub async fn transfer_from(ledger: Principal, from: Account, to: Account, amount: u64, fee: u64) -> Result<u64, String> {
    let arg = TransferFromArgs {
        spender_subaccount: None,
        from,
        to,
        amount: Nat::from(amount),
        fee: Some(Nat::from(fee)),
        memo: Some(Memo(b"deposit".to_vec().into())),
        created_at_time: None,
    };
    let res = Call::unbounded_wait(ledger, "icrc2_transfer_from")
        .with_arg(arg)
        .await
        .map_err(|e| format!("Ledger call failed: {e}"))?;
    let r: Result<Nat, TransferFromError> = res.candid().map_err(|e| e.to_string())?;
    let block = r.map_err(|e| match e {
        TransferFromError::InsufficientAllowance { .. } => "Approve the deposit in your wallet first".to_string(),
        TransferFromError::InsufficientFunds { .. } => "Not enough ICP in your wallet".to_string(),
        other => format!("Deposit failed: {other:?}"),
    })?;
    to_u64(&block)
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

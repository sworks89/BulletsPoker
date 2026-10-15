import { useEffect, useState } from 'react';
import { useApp } from '../context';
import { Principal, backendId, unwrap, type DepositInfo } from '../ic';
import { icp, parseIcp } from '../format';

export function Wallet() {
  const { backend, ledger, principal, profile, run, refreshProfile } = useApp();
  const [info, setInfo] = useState<DepositInfo | null>(null);
  const [walletBalance, setWalletBalance] = useState<bigint | null>(null);
  const [depositAmt, setDepositAmt] = useState('');
  const [withdrawAmt, setWithdrawAmt] = useState('');
  const [withdrawTo, setWithdrawTo] = useState('');
  const [busy, setBusy] = useState(false);

  const loadWallet = async () => {
    if (!principal) return;
    setWalletBalance(await ledger.icrc1_balance_of({ owner: principal, subaccount: [] }));
  };

  useEffect(() => {
    backend.get_deposit_info().then(setInfo);
    loadWallet().catch(() => setWalletBalance(null));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [backend, principal?.toText()]);

  if (!profile || !principal) return <div className="panel">Sign in to manage your wallet.</div>;
  const fee = info?.fee ?? 10_000n;

  const deposit = async () => {
    setBusy(true);
    await run(async () => {
      const amount = parseIcp(depositAmt);
      // ICRC-2: allow the poker canister to pull `amount` (+ the transfer_from fee).
      const approve = await ledger.icrc2_approve({
        spender: { owner: Principal.fromText(backendId), subaccount: [] },
        amount: amount + fee,
        fee: [],
        memo: [],
        from_subaccount: [],
        created_at_time: [],
        expected_allowance: [],
        expires_at: [BigInt(Date.now() + 10 * 60_000) * 1_000_000n],
      });
      if ('Err' in approve) throw new Error(`Approve failed: ${Object.keys(approve.Err)[0]}`);
      unwrap(await backend.deposit(amount));
      setDepositAmt('');
    }, 'Deposit complete');
    await Promise.all([refreshProfile(), loadWallet()]);
    setBusy(false);
  };

  const claim = async () => {
    setBusy(true);
    await run(async () => {
      const amt = unwrap(await backend.claim_deposit());
      return amt;
    }, 'Deposit credited');
    await refreshProfile();
    setBusy(false);
  };

  const withdraw = async () => {
    setBusy(true);
    await run(async () => {
      const amount = parseIcp(withdrawAmt);
      const owner = Principal.fromText(withdrawTo.trim());
      unwrap(await backend.withdraw({ owner, subaccount: [] }, amount));
      setWithdrawAmt('');
    }, 'Withdrawal sent');
    await Promise.all([refreshProfile(), loadWallet()]);
    setBusy(false);
  };

  return (
    <div className="page-grid">
      <div className="panel">
        <div className="panel-title">Poker balance</div>
        <div className="big">{icp(profile.balance)} ICP</div>
        <p className="muted small">Chips you are not playing with. Buy-ins come from here and cash-outs return here when you stand up.</p>
      </div>

      <div className="panel">
        <div className="panel-title">Deposit from your Internet Identity wallet</div>
        <p className="muted small">
          Wallet principal: <code>{principal.toText()}</code>
          <br />
          ICP in wallet: <b>{walletBalance === null ? '…' : icp(walletBalance)}</b>
        </p>
        <div className="row gap">
          <input value={depositAmt} onChange={(e) => setDepositAmt(e.target.value)} placeholder="Amount in ICP" />
          <button className="btn" disabled={busy || !depositAmt} onClick={deposit}>
            Deposit
          </button>
        </div>
        <p className="muted small">Approves the poker canister for the amount, then pulls it in (ICRC-2). Ledger fees of {icp(fee)} ICP apply.</p>
      </div>

      <div className="panel">
        <div className="panel-title">Deposit from an exchange or other wallet</div>
        {info && (
          <>
            <p className="small">Send ICP to your personal deposit address:</p>
            <code className="addr">{info.account_id}</code>
            <p className="small">
              ICRC-1 account: owner <code>{info.account.owner.toText()}</code> with your deposit subaccount.
            </p>
            <button className="btn" disabled={busy} onClick={claim}>
              I've sent it — credit my balance
            </button>
          </>
        )}
      </div>

      <div className="panel">
        <div className="panel-title">Withdraw</div>
        <input value={withdrawTo} onChange={(e) => setWithdrawTo(e.target.value)} placeholder="Destination principal" />
        <div className="row gap">
          <input value={withdrawAmt} onChange={(e) => setWithdrawAmt(e.target.value)} placeholder="Amount in ICP" />
          <button className="btn ghost" onClick={() => setWithdrawTo(principal.toText())}>
            My wallet
          </button>
          <button className="btn" disabled={busy || !withdrawAmt || !withdrawTo} onClick={withdraw}>
            Withdraw
          </button>
        </div>
        <p className="muted small">A ledger fee of {icp(fee)} ICP is deducted from your poker balance on top of the amount.</p>
      </div>
    </div>
  );
}

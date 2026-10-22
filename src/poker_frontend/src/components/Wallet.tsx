import { useEffect, useState } from 'react';
import { useApp } from '../context';
import { Principal, backendId, unwrap, type DepositInfo, type LedgerOp } from '../ic';
import { usePoll } from '../context';
import { agoText, icp, parseIcp, timeAgo } from '../format';
import { prizeIcp } from '../rewards';
import { SUPPORT_CONTACT } from '../support';
import { depositCost, icrcAccountText, parseDestination, subaccountHex, sweepNet, unknownOpState, withdrawReview } from '../walletMath';
import { WelcomeBanner, WelcomeCard, useWelcome } from './WelcomeBonus';
import { lastAnnounced, markAnnounced, toastFor, welcomeStatus } from '../welcome';

type Review = { owner: Principal; subaccount: Uint8Array | null; amount: bigint; total: bigint };

export function Wallet() {
  const { backend, ledger, principal, profile, run, refreshProfile, notify } = useApp();
  const { view: welcome, reload: reloadWelcome } = useWelcome();
  const [info, setInfo] = useState<DepositInfo | null>(null);
  const [walletBalance, setWalletBalance] = useState<bigint | null>(null);
  const [depositAmt, setDepositAmt] = useState('');
  const [withdrawAmt, setWithdrawAmt] = useState('');
  const [withdrawTo, setWithdrawTo] = useState('');
  const [busy, setBusy] = useState(false);
  const [review, setReview] = useState<Review | null>(null);
  const [formError, setFormError] = useState('');
  const [ops, setOps] = useState<LedgerOp[]>([]);
  usePoll(async () => setOps(await backend.get_my_ledger_ops()), 10000, [backend, principal?.toText()]);

  const loadWallet = async () => {
    if (!principal) return;
    setWalletBalance(await ledger.icrc1_balance_of({ owner: principal, subaccount: [] }));
  };

  useEffect(() => {
    backend.get_deposit_info().then(setInfo);
    loadWallet().catch(() => setWalletBalance(null));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [backend, principal?.toText()]);

  const who = principal?.toText();
  const status = welcome ? welcomeStatus(welcome) : null;
  useEffect(() => {
    if (!who || !welcome || !status || status === 'offer' || status === 'none') return;
    const prev = lastAnnounced(who);
    if (prev === status) return;
    markAnnounced(who, status);
    const toast = toastFor(prev, status);
    if (toast === 'active') notify(`Welcome bonus: ${prizeIcp(welcome.locked)} ICP is locked for you. Play to release it.`);
    else if (toast === 'done') notify('Your welcome bonus is fully released.');
    else if (toast === 'expired') notify('Your welcome bonus has expired.');
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [who, status]);

  if (!profile || !principal) return <div className="panel">Sign in to manage your wallet.</div>;
  const fee = info?.fee ?? 10_000n;

  // The transfer already happened; a failed refresh must not look like a failed transfer.
  const refreshAfterChange = async (f: () => Promise<unknown>) => {
    try {
      await f();
    } catch {
      notify('Done, but the screen could not refresh. Reload the page to see your latest balance.');
    }
  };

  const deposit = async () => {
    setBusy(true);
    try {
      const done = await run(async () => {
        const amount = parseIcp(depositAmt);
        const cost = depositCost(amount, fee);
        if (walletBalance !== null && cost.total > walletBalance) {
          throw new Error(`Your wallet needs ${icp(cost.total)} ICP (the amount plus two ledger fees) but holds ${icp(walletBalance)} ICP`);
        }
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
        return true;
      }, 'Deposit complete');
      if (done) {
        setDepositAmt('');
        await refreshAfterChange(() => Promise.all([refreshProfile(), loadWallet(), reloadWelcome()]));
      }
    } finally {
      setBusy(false);
    }
  };

  const claim = async () => {
    setBusy(true);
    try {
      const credited = await run(async () => unwrap(await backend.claim_deposit()), 'Deposit credited');
      if (credited !== undefined) await refreshAfterChange(() => Promise.all([refreshProfile(), reloadWelcome()]));
    } finally {
      setBusy(false);
    }
  };

  /** Validates the form and opens the review step (nothing is sent yet). */
  const startReview = () => {
    setFormError('');
    const dest = parseDestination(withdrawTo);
    if ('error' in dest) return setFormError(dest.error);
    let amount: bigint;
    try {
      amount = parseIcp(withdrawAmt);
    } catch (e) {
      return setFormError(e instanceof Error ? e.message : String(e));
    }
    const check = withdrawReview(amount, fee, profile.balance);
    if (!check.ok) return setFormError(check.error);
    setReview({ owner: dest.owner, subaccount: dest.subaccount, amount, total: check.total });
  };

  const withdraw = async () => {
    if (!review) return;
    setBusy(true);
    try {
      const done = await run(async () => {
        unwrap(await backend.withdraw({ owner: review.owner, subaccount: review.subaccount ? [review.subaccount] : [] }, review.amount));
        return true;
      }, 'Withdrawal sent');
      if (done) {
        setReview(null);
        setWithdrawAmt('');
        await refreshAfterChange(() => Promise.all([refreshProfile(), loadWallet()]));
      }
    } finally {
      setBusy(false);
    }
  };

  let depositAmount: bigint | null = null;
  try {
    if (depositAmt) depositAmount = parseIcp(depositAmt);
  } catch {
    depositAmount = null;
  }
  const quote = depositAmount === null ? null : depositCost(depositAmount, fee);

  const depositAccount = info ? icrcAccountText(info.account.owner, info.account.subaccount[0] ? new Uint8Array(info.account.subaccount[0]) : null) : '';
  return (
    <div className="page-grid">
      <div className="panel">
        <div className="panel-title">Poker balance</div>
        <div className="big">{icp(profile.balance)} ICP</div>
        <p className="muted small">Chips you are not playing with. Buy-ins come from here and cash-outs return here when you stand up.</p>
      </div>
      {status === 'offer' && <WelcomeBanner />}
      {status === 'active' && welcome && <WelcomeCard view={welcome} />}

      <div className="panel">
        <div className="panel-title">Deposit from your Internet Identity wallet</div>
        <p className="muted small">
          Wallet principal: <code>{principal.toText()}</code>
          <br />
          ICP in wallet: <b>{walletBalance === null ? '…' : icp(walletBalance)}</b>
        </p>
        <div className="row gap">
          <label className="field">
            <span className="small muted">Deposit amount (ICP)</span>
            <input value={depositAmt} onChange={(e) => setDepositAmt(e.target.value)} placeholder="Amount in ICP" inputMode="decimal" />
          </label>
          <button className="btn" disabled={busy || !depositAmt} onClick={deposit}>
            Deposit
          </button>
        </div>
        <p className="muted small">
          Approves the poker canister for the amount, then pulls it in (ICRC-2). Two ledger fees of {icp(fee)} ICP each (approve and transfer) are paid from
          your wallet on top of the amount.
        </p>
        {quote && depositAmount !== null && (
          <p className="small">
            You deposit <b>{icp(depositAmount)} ICP</b> · fees {icp(quote.approveFee)} + {icp(quote.pullFee)} ICP · your wallet needs{' '}
            <b>{icp(quote.total)} ICP</b>.
          </p>
        )}
      </div>

      <div className="panel">
        <div className="panel-title">Deposit from an exchange or other wallet</div>
        {info && (
          <>
            <p className="small">Send ICP to your personal deposit address:</p>
            <code className="addr">{info.account_id}</code>
            <CopyButton text={info.account_id} label="Copy account ID" what="Account ID" />
            <p className="small">ICRC-1 account (complete, for wallets that support it):</p>
            <code className="addr">{depositAccount}</code>
            <CopyButton text={depositAccount} label="Copy ICRC-1 account" what="ICRC-1 account" />
            <p className="muted small">
              One ledger fee of {icp(fee)} ICP is taken when your deposit is moved in, so you are credited what you sent minus {icp(fee)} ICP (sending 10 ICP
              credits {icp(sweepNet(1_000_000_000n, fee))} ICP). Amounts of {icp(fee)} ICP or less are not credited.
            </p>
            <button className="btn" disabled={busy} onClick={claim}>
              I've sent it — credit my balance
            </button>
          </>
        )}
      </div>

      <div className="panel">
        <div className="panel-title">Withdraw</div>
        {!review ? (
          <>
            <label className="field">
              <span className="small muted">Destination</span>
              <input
              value={withdrawTo}
              onChange={(e) => setWithdrawTo(e.target.value)}
              placeholder="Destination principal (optionally with subaccount)"
              />
            </label>
            <div className="row gap">
              <label className="field">
                <span className="small muted">Withdraw amount (ICP)</span>
                <input value={withdrawAmt} onChange={(e) => setWithdrawAmt(e.target.value)} placeholder="Amount in ICP" inputMode="decimal" />
              </label>
              <button className="btn ghost" onClick={() => setWithdrawTo(principal.toText())}>
                My wallet
              </button>
              <button className="btn" disabled={busy || !withdrawAmt || !withdrawTo} onClick={startReview}>
                Review
              </button>
            </div>
            {formError && <p className="small neg" role="alert">{formError}</p>}
            <p className="muted small">
              Send to a principal, a principal plus a 64-character hex subaccount, or an ICRC-1 account. Old-style ICP account IDs are not supported. A ledger
              fee of {icp(fee)} ICP is deducted from your poker balance on top of the amount.
            </p>
          </>
        ) : (
          <>
            <p className="small">Check the details. A withdrawal cannot be undone.</p>
            <div className="small">
              To: <code className="addr">{icrcAccountText(review.owner, review.subaccount)}</code>{' '}
              <CopyButton text={icrcAccountText(review.owner, review.subaccount)} label="Copy destination" what="Destination" />
              {review.subaccount && (
                <>
                  <br />
                  Subaccount: <code className="addr">{subaccountHex(review.subaccount)}</code>
                </>
              )}
              <br />
              Network: ICP ledger
              <br />
              Amount: <b>{icp(review.amount)} ICP</b>
              <br />
              Ledger fee: {icp(fee)} ICP
              <br />
              Total debited from your poker balance: <b>{icp(review.total)} ICP</b>
            </div>
            <div className="row gap">
              <button className="btn ghost" disabled={busy} onClick={() => setReview(null)}>
                Back
              </button>
              <button className="btn" disabled={busy} onClick={withdraw}>
                Confirm withdrawal
              </button>
            </div>
          </>
        )}
      </div>
      <div className="panel">
        <div className="panel-title">Transactions</div>
        {ops.length === 0 && <p className="muted small">Deposits and withdrawals show up here.</p>}
        {ops.map((op) => (
          <OpRow key={String(op.id)} op={op} />
        ))}
      </div>
    </div>
  );
}

const KIND_LABEL: Record<string, string> = { Deposit: 'Deposit', Sweep: 'Deposit (transfer)', Withdraw: 'Withdrawal', Treasury: 'Treasury' };

/** Copies text to the clipboard; if that is blocked, shows the text so it can be selected by hand. */
function CopyButton({ text, label, what }: { text: string; label: string; what: string }) {
  const { notify } = useApp();
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      notify(`${what} copied`);
    } catch {
      notify(`${what}: ${text}`);
    }
  };
  return (
    <button className="btn ghost small" onClick={copy}>
      {label}
    </button>
  );
}

function OpRow({ op }: { op: LedgerOp }) {
  const kind = Object.keys(op.kind)[0];
  const status = Object.keys(op.status)[0];
  const incoming = kind === 'Deposit' || kind === 'Sweep';
  const needsSupport = 'Unknown' in op.status && unknownOpState(op, Date.now()) === 'needs_support';
  const label: Record<string, string> = {
    Completed: 'Done',
    Failed: 'Failed',
    Pending: 'Sending…',
    Unknown: needsSupport ? 'Needs support' : 'Confirming with the ledger',
  };
  const detail =
    'Completed' in op.status
      ? `block #${String(op.status.Completed.block)}`
      : 'Failed' in op.status
        ? op.status.Failed.reason
        : 'Unknown' in op.status
          ? needsSupport
            ? `Needs support review — contact ${SUPPORT_CONTACT} with operation #${String(op.id)}`
            : 'Confirming with the ledger — usually within minutes. Your balance stays reserved until then.'
          : '';
  return (
    <div className="op-row">
      <div className="grow">
        <div className="small">
          <b>{KIND_LABEL[kind] ?? kind}</b> <span className="muted">· {timeAgo(op.created_at_time)}</span>
        </div>
        <div className="muted small">{detail}</div>
        {'Unknown' in op.status && (
          <div className="muted small">
            Operation #{String(op.id)}{' '}
            <CopyButton text={String(op.id)} label="Copy ID" what="Operation ID" />{' '}
            · started {agoText(op.created_at_time)} · last attempt {agoText(op.updated_at)} · {op.status.Unknown.attempts}{' '}
            {op.status.Unknown.attempts === 1 ? 'attempt' : 'attempts'}
          </div>
        )}
      </div>
      <div className={status === 'Failed' ? 'muted strike' : incoming ? 'pos' : ''}>
        {incoming ? '+' : '−'}
        {icp(op.amount)} ICP
      </div>
      <span className={`tag op-${status.toLowerCase()}`}>{label[status] ?? status}</span>
    </div>
  );
}

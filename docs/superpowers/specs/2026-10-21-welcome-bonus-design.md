# Welcome bonus: double the first deposit

## Why

A welcome bonus gives new players a reason to make their first ICP deposit and
keep playing. The user wants the first deposit doubled, up to 100 ICP. The bonus
is locked and released in steps as the player pays rake, so, like the rewards
and referral programs, it is funded from rake the player has already paid and
never costs the house money.

## Goals

- A player's first successful ICP deposit earns a matching bonus: 100% of the
  deposit, up to **100 ICP**.
- The bonus starts **locked**. Locked bonus can't be played or withdrawn.
- As the player pays rake, pieces of the bonus are **released** into their normal
  ICP balance, where they are ordinary money.
- The player always sees how much is locked, how much has been released, and how
  long they have left.

## Non-goals

- The player's own deposit is never locked or touched. They can play with it and
  withdraw it at any time.
- No bonus on later deposits, for play chips, or for players who had already
  deposited before this ships.
- No bonus for the referrer. The referral program already pays them.

## Anti-goals

- A release that pays out more than the house kept from the rake that produced
  it.
- Farming: opening fresh accounts, depositing and withdrawing to collect bonuses
  at a profit.
- Locked bonus counted as player money anywhere: in balances, withdrawals or
  solvency.

## How it works

- **Granting the bonus.**
  - On a player's first successful deposit, through either path (an ICRC-2
    deposit or a sweep from the deposit subaccount), they get a welcome bonus of
    `min(amount, 100 ICP)`. The hook is the money effect of a successful deposit
    in `wallet.rs` (`apply_success`).
  - The grant is recorded once and never repeats, even if a deposit is retried or
    settled late. Deposit settlement is already exactly-once per operation.
  - *My call:* players who already have a successful deposit when this ships are
    not eligible. The builder decides how to detect that reliably, for example a
    one-time pass on upgrade over the existing ledger operations, or marking
    existing accounts.
- **What counts toward release.** The same house-share rake the referral cut uses:
  - cash-hand rake attributed to the player, minus the host's cut in hosted
    rooms;
  - the player's ICP tournament entry fees, at the moment they become house
    income.

  In the code this is the `house_e8s` passed to `earn_rake`.
- **Release rate.** Each time the player pays house-share rake `h`, the amount
  released is `min(locked, 30% of h)`, with remainders under 1 e8s carried forward.
  - The house keeps 60% of the house share, the pools take the other 40%, and a
    referred player's referrer takes 20% of the house share from the treasury.
  - At 30%, the house still keeps at least 10% of the house share even for a
    referred player with an active bonus, which covers that player's cycle cost.
    The user chose this after the profit analysis; it was 40%.
  - In other words, releasing the full 100 ICP takes about 333 ICP of house-share
    rake.
- **Where released money comes from.** The treasury portion of the same rake that
  triggered it, taken right after the referral cut. It is capped so the treasury
  can never go negative. Released money goes straight into the player's ICP
  balance.
- **Release cadence.** *My call:* release continuously, with each rake event
  adding its piece. Every bit of rake moves the progress bar, and there is no
  separate payout job.
- **Expiry.** *My call:* 90 days after the grant, any bonus still locked expires.
  It was never counted as money, so nothing moves.
- **Locked bonus is not a liability.**
  - It is a promise, kept outside balances, withdrawals and the solvency check.
  - Only released amounts are real, and they enter balances through `credit`, so
    the existing conservation rules cover them.
- **Suspended players.** *My call:* nothing is released while the player has an
  active ban. Their locked bonus keeps counting down to expiry.

## Why farming doesn't pay

To get bonus out, a player must first pay at least 3.3× that amount in
house-share rake, which they lose to the house. A ring of fresh accounts depositing,
playing each other and withdrawing ends up with less than it put in, even after
adding Fish Buffet cashback and referral cuts.

## What the player sees

- **On the Wallet page,** before the first deposit: "Welcome bonus: we double your
  first deposit, up to 100 ICP."
- **After the grant,** a Welcome bonus card on the Wallet page and on the Rewards
  page's Fish Buffet tab shows:
  - locked
  - released so far
  - a progress bar
  - days left until it expires
  - one line: "Every 1 ICP of rake you pay releases 0.3 ICP of bonus."
- **A toast when the bonus is granted,** and a quiet note when it is fully
  released or has expired.

## Constraints

- **Persistence** follows the existing rules: new `User` and heap fields are
  `Option<T>`, and per-user data is bounded.
- **Calls:** state changes go through `caller()` on update methods, and reads are
  queries. The bonus state can travel in an existing profile or rewards view, or
  in a new query (the builder's call).
- **Candid:** regenerate it with `UPDATE_CANDID=1 cargo test`.
- **Money conservation:** wallets + tables + treasury + tournaments + jackpot +
  pools + referral owed must still equal the ICP put in, after grants, releases
  and expiries. A grant adds no money anywhere.
- **Where it builds:** on the `referrals` branch (PR #5), which is stacked on
  `rewards` (PR #4). It reuses `earn_rake`, the referral cut order and the
  treasury cap pattern.

## Left to the builder

- Data structures and method names.
- How existing depositors are detected.
- Where the bonus state is exposed to the frontend.
- The exact placement and styling of the bonus card.

## Also in this change: return unused Fish Buffet money to the house

The profit analysis found that the Fish Buffet pool receives 30% of the house
share, but at a typical tier mix (mostly Plankton to Crab) the wheel pays out
about 12%. The rest would pile up in the pool forever, counted as owed but never
paid. The user chose to fix this here.

- **Weekly sweep.** At each weekly close (Monday 00:00 UTC, on the same timer and
  with the same idempotency as the leaderboards and referrals), any Fish Buffet
  pool balance above a **reserve** moves back to the treasury.
- **The reserve** is the largest of:
  - the wheel payouts of the last 13 weeks;
  - twice the expected value of all banked, unspun spins (the sum of stake ×
    tier rate over every banked spin, kept as a running total);
  - a floor of 10 ICP, so early spins always pay.
- The pool share stays at 30%, so Shark-heavy weeks are still fully funded. Only
  money beyond the reserve returns to the house.
- Leaderboard pools are untouched. Unpaid prizes already roll into later prizes.

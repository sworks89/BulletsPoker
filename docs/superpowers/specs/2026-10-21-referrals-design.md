# Referral program: an ongoing cut of your friends' rake

## Why

Players bring friends. The user wants every successful referral to pay the
referrer, in the style of crypto poker sites (CoinPoker, Stake): an ongoing cut
of the rake each referred friend pays.

Like the rewards program (`2026-10-20-rewards-design.md`), the cut is funded
from rake, so it never costs the house more than the friend's play brought in.

## Goals

- Every player has a personal referral code and link.
- A new player who signs up through a link, or enters a code soon after signing
  up, is attached to that referrer for good.
- For 12 months from attaching, the referrer earns 20% of the house's share of
  the rake their friend pays at ICP tables and in ICP tournament fees. It is paid
  weekly into the referrer's ICP balance.
- A Referrals page shows your link, your friends and what you've earned.

## Non-goals

- No signup bonus or one-time bounty for either side. Accounts on ICP cost
  almost nothing to create, so bounties are easy to farm.
- No reward for the friend. *My call:* the friend already has the Fish Buffet.
  This can be added later.
- No cut from play-chip tables or chip tournaments, because they take no rake.
- No multi-level schemes: only the direct referrer earns.

## Anti-goals

- A referral that pays out more than the referred play brought in to the house.
- Farming with self-referral or with fake friends that turns a profit.
- Touching the Fish Buffet or leaderboard pools. Those keep their 30% / 10%.

## How the money flows

The rewards program splits the house's share of rake as follows. The house's
share is cash rake after the host's cut, plus ICP tournament fees:
- Fish Buffet pool 30%
- leaderboard pool 10%
- treasury 60%

For a referred player within their 12 months:
- **The cut.** 20% of the house's share of that player's attributed rake moves
  from the treasury's portion to the referrer. The treasury nets 40% on that
  player, and the pools are unchanged.
  - Cash hands use the same per-player attribution the rewards program uses (by
    chips committed, with the host's cut removed in hosted rooms).
  - For tournaments, the friend's entry fee counts in full, including late-entry
    fees, at the moment the fee becomes house income: at the start, or at late
    registration.
- **Accrual, then a weekly payout.**
  - The cut builds up as *owed* to the referrer, held as a liability (counted in
    the solvency check and house stats like the pools).
  - It is paid into the referrer's ICP balance when the week closes (Monday 00:00
    UTC, on the same timer as the leaderboards).
  - The weekly close must be idempotent, as the leaderboard close is.
- **Amounts below 1 e8s** carry forward per referrer.
- **The treasury can never go negative.** The cut is always taken from the
  treasury portion of the very rake that produced it.
- **Suspended referrers** (an active ban at payout time): what they are owed
  returns to the treasury at the weekly close. *My call*, mirroring how
  leaderboards skip suspended players.

## Attaching a referrer

- **The code.** Each player's referral code is their username. Usernames can't be
  changed, so the code is stable. Matching ignores case.
- **The link.** The link is the frontend URL with `?ref=<code>`.
  - The frontend remembers the code in the browser until signup.
  - It passes the code to registration.
- **Entering a code later.** A player can also enter a code on the Referrals
  page within 24 hours of creating their account. After that, or once a referrer
  is set, the field is gone.
- **Rules,** all enforced by the backend:
  - You can't refer yourself.
  - The referrer must already exist and must have created their account before
    the friend's account.
  - The referrer can't be someone the friend referred, so there are no loops.
  - A referrer is set at most once and never changes.
  - An unknown code is an error. At signup, an unknown code must not block
    registration: the player registers without a referrer and the UI says the
    code wasn't found.
- **The 12 months** run from the moment the referrer is attached (365 days).

## Referrals page

A new page in the main navigation, "Referrals", shows:
- **Your link,** with a copy button, and your code.
- **Totals:**
  - friends signed up
  - friends active in the last 7 days (earned you something)
  - owed this week (paid Monday)
  - paid in total
- **Friends:**
  - username
  - when they joined
  - when the cut ends
  - what they've earned you in total
- **Entering a code** while you're within your 24 hours and have no referrer:
  "Got a code from a friend?"
- **The rules in plain words.** "You get 20% of the house's cut of your friends'
  rake at ICP tables and ICP tournaments for 12 months. Paid every Monday."

## Constraints

- **Persistence follows the existing rules:**
  - New `User` fields and heap fields are `Option<T>`.
  - Per-user data must not grow without limit. A referrer can have many friends,
    so the friends list must not live inside the `User` record without a bound.
    Use a stable-memory structure (see `crate::store`) or a bounded record plus
    counters.
- **Calls:** state changes go through `caller()` on update methods, and reads are
  queries.
- **Candid:** regenerate it with `UPDATE_CANDID=1 cargo test`.
- **Money conservation:** wallets + tables + treasury + tournaments + jackpot +
  pools + referral owed must still equal the ICP put in. The existing
  conservation tests are extended to cover this.
- **Where it builds:** on the `rewards` branch (PR #4). It uses the rewards
  program's attribution, its funding split and the weekly period clock.

## Left to the builder

- Data structures and method names.
- How the code is looked up. There may be a username index already; if not, add
  one.
- How the registration endpoint takes the optional code. Prefer adding it
  without breaking existing callers.
- Layout of the Referrals page within the existing style.

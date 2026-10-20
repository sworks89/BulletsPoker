# Rewards: Fish Buffet tiers and leaderboards

## Why

BulletsPoker's ICP tables need a reason to come back every week. Big sites
answer this with two separate systems: loyalty tiers that reward *volume*
(GGPoker's playful "Fish Buffet") and leaderboards that reward *competition*.
We want both, funded entirely from rake, so they never cost the house money.

## Goals

- **Fish Buffet:** players earn points from the rake they pay at ICP tables and
  ICP tournament fees, climb seven sea-creature tiers, and spin a wheel for ICP
  cashback as they progress. Higher tiers spin richer wheels.
- **Leaderboards:** weekly and monthly boards for points and for winnings, each
  with an ICP prize pool, medals and a lobby announcement for the winners.
- **Zero cost:** every reward is paid from a pool filled by a fixed share of the
  house's rake. A pool can run short; the treasury never tops it up.

## Non-goals

- Play-chip tables and chip tournaments earn nothing (they take no rake).
  ICP only, for now.
- No change to rake rates, the host share or the bad-beat jackpot.
- Store sales do not fund rewards.
- No tournament-specific leaderboards (tournaments only feed the four boards).

## Anti-goals

- A reward that can be paid from the treasury, or any fixed-amount prize that
  could exceed what its pool holds.
- A board someone can top profitably by chip-dumping between their own accounts.

## Feasibility (agreed with the user)

- Today, cash rake is 2.5% of the pot, capped at 3 BB and taken only when a flop
  is dealt. Hosted rooms pay 20% of their rake to the host, and the rest goes to
  the treasury. ICP tournament fees (about 5%) go to the treasury when the event
  starts.
- The bad-beat jackpot is a separate 1% drop paid by players into its own pool.
  It never touches the treasury and is unchanged here.
- Estimates:
  - About 3 BB of rake per player per hour.
  - A 10-hour-a-week regular pays about 0.006 ICP a week at Penny Lane and about
    6 ICP a week at High Roller.
  - The fixed costs are the 1-second timer (roughly $5–10 a month in cycles) and
    the update calls per hand.
  - Penny Lane is about break-even on cycles, and higher stakes are profitable.
- These estimates are not measured. The plan should include measuring the cycles
  per hand locally, so the split below can be checked against real numbers.

## Funding split

Every time the house's share of ICP rake comes in, it is split three ways. The
house's share is the cash-hand rake after the host's cut, plus each tournament
fee when the event starts:

| Pool | Share | Pays for |
|---|---|---|
| Fish Buffet pool | 30% | Wheel spins |
| Leaderboard pool | 10% | The four boards, split into four equal quarters (one per board) |
| Treasury | 60% | Cycles and profit (unchanged meaning) |

- The pools hold ICP that belongs to players-to-be. They count as **liabilities**
  in the existing solvency check and house stats, in the same way the jackpot
  does. The treasury can never withdraw them.
- Constants for these shares live next to `HOST_RAKE_SHARE_BPS`.

## Points

- **Attribution.** A cash hand's rake is split among the players in that hand,
  in proportion to the chips each committed to the raked pots (the "contributed"
  method).
  - A tournament fee counts entirely toward the player who paid it.
  - Rounding may leave a remainder, which goes to no one.
  - In hosted rooms, points are based on the full rake a player contributed, not
    just the house's 80%.
- **Rate.** 1 point per 1,000 e8s (0.00001 ICP) of attributed rake. Points are
  whole numbers, and the leftover e8s carry forward per player.
  - *My call:* this gives a Penny Lane regular about 60 points an hour and a High
    Roller about 60,000. That is enough for the low stakes to feel progress.
- **Two counters per player:**
  - *Tier points* decay (see below) and decide your tier.
  - *Lifetime points* never decay. They drive the wheel and the points
    leaderboards (per period).

## Tiers (Fish Buffet)

These seven tiers come from tier points. The thresholds and spin steps are *my
call*, as tunable constants. The cashback rates were chosen by the user so that
every tier pays for itself: each rate stays under the 30% that funds the pool.
Higher rates would leave top-tier spins short most of the time, because high
rollers reach Shark within about a day.

| Tier | From (tier points) | A spin every (points) | Cashback rate (wheel's expected value) |
|---|---|---|---|
| Plankton | 0 | 100 | 8% |
| Shrimp | 500 | 400 | 11% |
| Crab | 2,500 | 1,500 | 14% |
| Octopus | 10,000 | 6,000 | 17% |
| Dolphin | 40,000 | 20,000 | 20% |
| Orca | 150,000 | 70,000 | 24% |
| Shark | 500,000 | 200,000 | 28% |

- **Art.** Each tier gets a creature drawn in the existing outlined cartoon style
  (see `src/poker_frontend/src/art/`). Players see it on the Rewards page and on
  their profile.
- **Decay.** This is GG-style. After 7 days in a row without earning points, tier
  points drop by 5% a day until you earn again.
  - Your tier follows your points, so it can go down.
  - Spins you've already earned are never lost.
  - *My call on the numbers:* GG's exact figures aren't public in a form we can
    copy.
- **Tier-up.** Reaching a new tier shows a celebration and posts a chat line at
  your table.

## The wheel

- **The meter.** A spin meter fills with the points you earn (based on lifetime
  points, so decay never empties it). Each time it passes your current tier's
  spin step, you bank one spin. The spin records its *stake* and your tier at that moment.
  The stake is the house's share of the rake that filled it: the full rake at
  house tables, and the 80% left after the host's cut in hosted rooms (*my
  call*). Points still count the full rake. Because of this, a spin from a
  hosted room is funded at the same 30% as any other spin, so no tier costs more
  than it brings in.
- **Banking.** Banked spins wait until you spin them, up to 50. After that, the
  meter keeps filling but no new spins are banked.
- **Prize.** prize = stake × tier rate × wheel multiplier.
  - The multipliers are 0.5×, 1×, 1×, 1×, 1.5×, 2×, 5× (and a rare 10×). Their
    weights make the expected multiplier exactly 1.0, so the expected prize
    equals the tier rate.
  - The backend draws the result using the canister's existing randomness. The
    frontend only animates the result.
- **Pool cap.** The amount paid is min(prize, Fish Buffet pool), credited to the
  player's ICP balance.
  - If the pool is short, the spin pays what's there and the result says so.
  - Nothing is owed afterwards.
- **History.** Each player's last 20 spins are kept for display.

## Leaderboards

- **The four boards:** Weekly Points, Weekly Winnings, Monthly Points and
  Monthly Winnings.
  - A week runs Monday 00:00 UTC to the next Monday. A month runs from the 1st at
    00:00 UTC.
- **Points boards** rank by points earned in the period.
- **Winnings boards** rank by net ICP won in the period:
  - Cash-table results count as won minus committed, as in today's
    `net_winnings`.
  - ICP tournament results count as prize minus buy-in, rebuys, add-ons and
    fee, recorded when the player finishes.
  - You must play at least 200 cash or tournament hands in a week, or 500 in a
    month, to qualify.
  - Only players with a net result above zero appear on a winnings board and can
    be paid (*my call*). A player who lost ICP is not a winner.
- **Prizes.** When a period ends, its quarter of the leaderboard pool is paid out:
  - The split is 30 / 20 / 13 / 9 / 7 / 6 / 5 / 4 / 3 / 3% to the top 10 (*my
    call*).
  - Ties are broken by whoever reached the value first.
  - Any unpaid share (fewer than 10 qualifiers, or the guard below) stays in that
    board's quarter for the next period. It never goes to the house.
- **The chip-dumping guard (winnings boards only).** A prize can't exceed the
  rake that player paid in that period.
  - This makes dumping chips between your own accounts unprofitable at best.
  - An honest winner rarely hits the limit.
- **Bragging rights:**
  - The top 3 on each board get a profile medal naming the board and period, for
    example "Weekly Shark 🥇 · Wk 42 2026". Medals use the existing `medals`
    map, so a player can hold many.
  - The winners are announced in lobby chat, like a jackpot hit.
- **Display.**
  - A Leaderboards view shows each board's live top 10 (plus your own rank if
    you're outside it), the current prize pool and a countdown to the reset.
  - Past winners of the last period are shown under each board.
- **Closing a period** happens on the existing timer and must be idempotent:
  - An upgrade or a missed tick must not pay twice or skip a period.
  - Periods closed late still close in order.

## Frontend

- **The existing all-time Leaderboard** (its nav entry, `Leaderboard` in
  `components/Profile.tsx`, `get_leaderboard`) moves into the new Leaderboards
  tab as a fifth, prize-free "All time" board (*my call*). Its nav entry is
  replaced by Rewards, and the backend endpoint and `leaders` list stay as they
  are.

- **A new Rewards page** in the main navigation has two tabs:
  1. **Fish Buffet:**
     - Your creature, tier and points, with a progress bar to the next tier.
     - Your spin meter and banked spins.
     - A playful wheel to spin, with the outcome animated.
     - Recent spins.
     - A note when decay is close (days since you last earned).
  2. **Leaderboards:** the four boards, as above.
- The profile shows the tier creature and the leaderboard medals.
- Copy rule:
  - Everything shown is ICP play only.
  - Say so on the page ("Points come from rake paid at ICP tables and ICP
    tournament fees").

## Constraints

- Persistence follows the existing rules. New persisted fields are `Option<T>` in
  the candid heap snapshot (`state.rs`, `HeapSnapshot`). Per-user data must not
  grow without limit: keep only the spin history and period counters.
- All new state changes go through `caller()` on update methods. Reads are
  queries.
- Regenerate candid and the bindings with `UPDATE_CANDID=1 cargo test`.
- Money invariants must still hold in the existing solvency and
  liabilities tests, with the pools included.

## Left to the builder

- Data structures, method names and how period counters are stored and rolled.
- The wheel's exact segment weights, as long as the expected multiplier is 1.0
  and 10× stays rare.
- Animation details, the creature drawings themselves, and the layout of the
  Rewards page within the existing style.
- How the cycle measurement is done.

# Welcome Bonus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Match a player's first ICP deposit 100% (up to 100 ICP) as a locked bonus released at 30% of the house-share rake they pay, expiring after 90 days. Also return unused Fish Buffet pool money above a reserve to the treasury each week.

**Architecture:**
- A new backend module, `welcome.rs`, owns the bonus.
- The bonus is **granted** in the deposit success path (`wallet.rs` `apply_success`). A one-time upgrade pass marks players who already deposited as ineligible.
- It is **released** in the rewards program's `earn_rake`, right after the referral cut, taken from the treasury, and **expires** lazily.
- The frontend shows a banner before the first deposit and a progress card afterwards.

**Tech Stack:** Rust (ic-cdk, candid, ic-stable-structures), React + TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-10-21-welcome-bonus-design.md` (builds on the rewards and referral specs)

## Global Constraints

- **Grant:** `WELCOME_MAX = 100 * E8S`. The bonus is `min(deposit amount, WELCOME_MAX)`.
  - It is granted on the first successful `OpKind::Deposit` or `OpKind::Sweep` only, once per player, ever.
  - Players with a successful deposit before this ships are not eligible.
- **Release:** `WELCOME_RELEASE_BPS = 3_000`.
  - Each `earn_rake(p, rake_e8s, house_e8s, now)` releases `min(locked, house_e8s × 30%)`, with sub-e8s remainders carried forward in bps units.
  - The release is taken from `self.treasury`, capped at the treasury, and credited with `credit()`.
  - It runs after `referral_cut`.
  - Nothing is released while `is_banned(p, now)`.
- **Expiry:** `WELCOME_EXPIRY_NS = 90 days` from the grant. After that, the locked bonus is 0. It is applied lazily on every read and release; no timer job.
- **Locked bonus is not money.**
  - It is never in balances, `wallet_total`, withdrawals, `house_stats` or the solvency check.
  - Released amounts are ordinary balance.
- **Conservation:** wallets + tables + treasury + tournaments + jackpot + pools + referral owed must still equal the ICP put in, after grants, releases and expiries.
- **Persistence:** new `User` and heap fields are `Option<T>`. Regenerate candid with `UPDATE_CANDID=1 cargo test`.
- **Calls:** state changes go through `caller()` on update methods, and reads are queries.
- **Commands:** run them inside WSL, as `MSYS_NO_PATHCONV=1 wsl -d Ubuntu-24.04 -- bash -lc 'cd /mnt/c/BulletsPoker && …'`.
  - Backend: `cargo test`.
  - Frontend: `cd src/poker_frontend && npx vitest run && npm run build`.
- **Copy:**
  - "Welcome bonus: we double your first deposit, up to 100 ICP."
  - "Every 1 ICP of rake you pay releases 0.3 ICP of bonus."

## Review Focus

1. **The treasury never goes negative, and no release exceeds what the treasury got from the same rake.** This must hold for a referred player at the bonus cap, in a hosted room, and when a tournament fee is paid.
2. **Fish surplus sweep:** never sweeps below the reserve, never runs twice in a week, and money only moves between `fish_pool` and `treasury`.
3. **Exactly one grant.** A deposit op that is retried, finished twice, or settled late after an upgrade grants once. A second deposit never grants. A deposit that fails grants nothing.
4. **Existing depositors** stay ineligible after the upgrade pass. The pass is idempotent, and runs once even across further upgrades.
5. **Expiry:** at `granted_at + 90 days` the locked amount is 0, nothing further is released, and the boundary is exact.
6. **Old data:** `User` records and heap snapshots without the new fields decode as "no bonus, eligible". The upgrade pass then corrects players who already deposited.

---

### Task 1: Granting the bonus

**Files:**
- Create: `src/poker_backend/src/welcome.rs`, holding the module, the constants, the types, `grant_welcome`, `mark_existing_depositors` and their tests.
- Modify:
  - `src/poker_backend/src/lib.rs`: add `mod welcome;`, and in `post_upgrade` call `state.mark_existing_depositors()`.
  - `src/poker_backend/src/state.rs`:
    - Add `User.welcome: Option<WelcomeBonus>`, `None` at registration.
    - Add the heap field `welcome_marked: Option<bool>`.
  - `src/poker_backend/src/wallet.rs`: in `apply_success`, for `Deposit | Sweep` and only after the credit succeeds, call `self.grant_welcome(&op.user, op.amount, now)`. Thread `now` through if `apply_success` doesn't have it.

**Interfaces:**
- Produces:
  - `pub enum WelcomeBonus { Ineligible, Active { granted_at: u64, amount: u64, released: u64, carry_bps: u64 } }`. `amount` is the full bonus granted; locked = `amount - released`, or 0 once expired.
  - `pub(crate) fn grant_welcome(&mut self, p: &Principal, deposit: u64, now: u64)`. It is a no-op unless `User.welcome` is `None`.
    - On a grant: `Active { granted_at: now, amount: min(deposit, WELCOME_MAX), released: 0, carry_bps: 0 }`.
    - A zero amount sets `Ineligible`.
  - `pub fn mark_existing_depositors(&mut self) -> usize`.
    - If `welcome_marked == Some(true)`, return 0.
    - Otherwise scan `stores.ledger_ops` for settled-successful `Deposit`/`Sweep` ops and set each such user's `welcome` from `None` to `Ineligible`. Use the success status shape in wallet.rs.
    - Set `welcome_marked = Some(true)` and return the count.
  - `pub fn welcome_locked(b: &WelcomeBonus, now: u64) -> u64`: the locked amount after expiry.

- [ ] **Step 1: Write the failing tests** (in `welcome.rs`, using the state and wallet test helpers that drive a deposit op through `begin_deposit` and `finish_op` with a success outcome):
  - `first_deposit_grants_a_matching_bonus_capped_at_100`: 3 ICP → 3. 250 ICP → 100. The balance rises by the deposit only. `wallet_total` and `house_stats().liabilities_total` are unchanged by the grant.
  - `second_deposit_and_failed_deposit_grant_nothing`.
  - `finishing_the_same_op_twice_grants_once`.
  - `sweep_counts_as_a_first_deposit`.
  - `existing_depositors_are_marked_ineligible_once`: they deposit before the pass, the pass runs and sets `Ineligible`, a second pass returns 0, and a later deposit grants nothing.
  - `old_user_records_decode_without_welcome`: a mirror struct of the old `User` shape, following the existing decode tests.
- [ ] **Step 2: Run** `cargo test welcome`. Expected: compile failure.
- [ ] **Step 3: Implement.** Then run `UPDATE_CANDID=1 cargo test` if any candid-visible type changed.
- [ ] **Step 4: Run** `cargo test`. Expected: everything passes.
- [ ] **Step 5: Commit** "Grant a welcome bonus on the first deposit".

### Task 2: Releasing and expiring the bonus

**Files:**
- Modify:
  - `src/poker_backend/src/welcome.rs`.
  - `src/poker_backend/src/rewards.rs`: in `earn_rake`, immediately after the `referral_cut` call, add `self.release_welcome(p, house_e8s, now)`.

**Interfaces:**
- Consumes: `WelcomeBonus` and `welcome_locked` (Task 1); `is_banned`; `credit`; the treasury.
- Produces: `pub(crate) fn release_welcome(&mut self, p: &Principal, house_e8s: u64, now: u64)`:
  1. Return if the player isn't `Active`, the locked amount after expiry is 0, or `is_banned(p, now)`.
  2. Compute `want = house_e8s × 3_000` plus `carry_bps`. Whole e8s are `/ 10_000`, and the rest stays in `carry_bps`.
  3. `amount = want.min(locked).min(self.treasury)`.
  4. Take `amount` from the treasury and `credit` it to the player. If `credit` fails, put it back in the treasury.
  5. Add `amount` to `released`.

- [ ] **Step 1: Write the failing tests:**
  - `rake_releases_thirty_percent_of_the_house_share`: after ICP hands, `released == floor(Σ house_e8s × 0.3)` (exact with carry), and the balance and treasury moved by the same amount.
  - `referred_player_at_the_cap_never_overdraws_the_treasury`:
    - A referred player in a hosted room plays with the treasury starting at 0.
    - The treasury never underflows.
    - `referral cut + release ≤ treasury share` of each hand, and the treasury keeps at least 10% of the house share (`house_take` grows by ≥ 0.1 × Σ house_e8s beyond the pools).
  - `tournament_fee_releases_bonus`.
  - `release_stops_when_fully_released`.
  - `nothing_releases_after_ninety_days`: exact boundary at `granted_at + 90d - 1` against `+ 90d`.
  - `suspended_players_release_nothing`.
  - `money_is_conserved_with_welcome_bonus`: use the full recount, extending the existing conservation tests.
  - `chip_play_releases_nothing`.
- [ ] **Step 2: Run** these tests. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test`. Expected: everything passes.
- [ ] **Step 5: Commit** "Release the welcome bonus as rake is paid".

### Task 3: Showing the bonus

**Files:**
- Modify:
  - `src/poker_backend/src/welcome.rs` and `lib.rs`: add `#[query] get_welcome() -> WelcomeView`.
  - The `.did` and the bindings, regenerated.
- Create:
  - `src/poker_frontend/src/welcome.ts` and `welcome.test.ts`.
  - `src/poker_frontend/src/components/WelcomeBonus.tsx`, holding the card and the banner.
- Modify:
  - `components/Wallet.tsx`: the banner while `eligible`, and the card while `active`.
  - `components/Rewards.tsx`: the card on the Fish Buffet tab while `active`.
  - `styles.css`.

**Interfaces:**
- Produces:
  - Backend: `pub struct WelcomeView`:
    - `eligible: bool`: no bonus yet and not ineligible.
    - `active: bool`.
    - `amount`, `released`, `locked: u64`: locked after expiry.
    - `expires_at: u64`.
    - `expired: bool`.
    - An anonymous caller or a player with no profile gets `eligible: false`, with everything else zero.
  - Frontend (`welcome.ts`):
    - `welcomeProgress(v): number`: `released / amount`, from 0 to 1, and 0 when `amount` is 0.
    - `daysLeft(expiresAtNs: bigint, nowMs: number): number`: whole days, at least 0.
    - `welcomeStatus(v): 'offer' | 'active' | 'done' | 'expired' | 'none'`.

- [ ] **Step 1: Write the failing tests:**
  - Backend:
    - `welcome_view_states`: offer, active, fully released, expired and ineligible.
    - `welcome_view_zero_for_strangers`.
  - Frontend:
    - `welcomeProgress`: half released gives 0.5.
    - `daysLeft`: 0 after expiry, otherwise it rounds down.
    - `welcomeStatus` for each state.
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.**
  - Backend view: run `UPDATE_CANDID=1 cargo test`.
  - Card:
    - locked and released amounts, using `prizeIcp`;
    - a progress bar;
    - "N days left";
    - the release-rate copy line.
  - Banner: the offer copy line.
  - Toasts:
    - one when the Wallet page sees `active` for the first time after a deposit (remember the shown flag in localStorage, inside try/catch);
    - a quiet note when the status becomes `done` or `expired`.
  - Polling: use `usePoll` every 30 s, and refresh after a deposit completes.
- [ ] **Step 4: Run** `cargo test`, then `npx vitest run && npm run build`. Expected: everything passes.
- [ ] **Step 5: Commit** "Show the welcome bonus offer and progress".

### Task 4: Return unused Fish Buffet money to the treasury

**Files:**
- Modify:
  - `src/poker_backend/src/rewards.rs`:
    - `RewardsState` gains `fish_paid_weeks: Option<VecDeque<u64>>` (the wheel payout total of each of the last 13 closed weeks, newest last), `fish_paid_this_week: Option<u64>`, `banked_ev: Option<u64>` (the running Σ stake × rate_bps / 10_000 over banked spins) and `fish_week: Option<u64>`.
    - `spin_wheel` adds `paid` to `fish_paid_this_week` and subtracts that spin's EV from `banked_ev`.
    - Banking a spin in `earn_rake` adds its EV. Spins dropped at the 50 cap add nothing.
    - Add `pub(crate) fn sweep_fish_surplus(&mut self, now: u64)`.
  - `src/poker_backend/src/state.rs`: call `sweep_fish_surplus(now)` in `tick`, next to `close_periods` / `close_referral_week`.
  - The `.did` and the bindings, if `HouseStats` or a view changes.

**Interfaces:**
- Produces:
  - Constants: `FISH_RESERVE_WEEKS = 13`, `FISH_RESERVE_FLOOR = 10 * E8S`.
  - `sweep_fish_surplus`: if `week_of(now) > fish_week`:
    1. Push `fish_paid_this_week` onto `fish_paid_weeks` (keep 13), reset it, and set `fish_week = week_of(now)`, all *before* moving any money.
    2. Compute `reserve = max(Σ fish_paid_weeks, 2 × banked_ev, FISH_RESERVE_FLOOR)`.
    3. Move `fish_pool - reserve` (if positive) from `fish_pool` to `treasury`.

    It is idempotent: two ticks in one second, or a tick after weeks of downtime, sweep once.
  - On the first run, with no history, the reserve is the larger of the floor and 2 × `banked_ev`. That is intended.

- [ ] **Step 1: Write the failing tests** in `rewards.rs` tests, or in the state tests module:
  - `surplus_above_the_reserve_returns_to_the_treasury`: pool 50 ICP, no payouts, no banked spins. After the Monday tick the pool is 10 ICP, the treasury has risen by 40, and `house_take` is unchanged.
  - `reserve_covers_recent_payouts`: a 13-week payout history summing to 30 ICP keeps 30.
  - `reserve_covers_twice_the_banked_spins`.
  - `sweep_runs_once_per_week`: covers two ticks, downtime, and an old snapshot with the new fields `None`.
  - `banked_ev_tracks_banking_and_spinning`: bank then spin brings it back to 0.
  - Conservation: the full recount still holds.
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test`. Expected: everything passes.
- [ ] **Step 5: Commit** "Return unused Fish Buffet money above a reserve to the treasury".

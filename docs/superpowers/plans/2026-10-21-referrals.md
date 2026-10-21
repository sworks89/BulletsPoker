# Referral Program Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pay referrers 20% of the house's share of their referred friends' ICP rake for 12 months, settled weekly, with a Referrals page.

**Architecture:**
- A new backend module, `referrals.rs`, does three things:
  - attaches a referrer to a friend: a `User` field on the friend, plus a stable map of each referrer's friends;
  - takes the cut inside the rewards program's `earn_rake`, the one place every ICP rake and fee share already passes, moving it from the treasury into an owed balance;
  - pays owed balances at each weekly close.
- The frontend captures `?ref=` links, attaches the code right after signup, and adds a Referrals page.

**Tech Stack:** Rust (ic-cdk, candid, ic-stable-structures), React + TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-10-21-referrals-design.md` (builds on `docs/superpowers/specs/2026-10-20-rewards-design.md`)

## Global Constraints

- **The cut:** `REFERRAL_CUT_BPS = 2_000`, which is 20% of the friend's *house-share* rake (the `house_e8s` passed to `earn_rake`). It lasts `REFERRAL_DURATION_NS = 365 days` from when the referrer is attached.
- **Where the cut comes from:** the treasury only. The Fish Buffet and leaderboard pools are never touched.
  - The treasury must never underflow.
  - Remainders smaller than 1 e8s carry forward per referrer.
- **Owed balances:**
  - They are a liability, counted in `house_stats().liabilities_total` (and therefore in the solvency check).
  - They are paid into the referrer's ICP balance at the weekly close, Monday 00:00 UTC, using `crate::leaderboards::week_of`.
  - The close is idempotent and is also called before any cut is recorded, so a week is always closed before the next week's cut is written.
- **Suspended referrers:** a referrer with an active ban at payout gets nothing. What they're owed returns to the treasury.
- **Attaching a referrer:**
  - The code is the referrer's username, matched without regard to case through `stores.usernames`.
  - The friend's account must be at most 24 h old.
  - A friend can attach at most one referrer, and it never changes.
  - No self-referral.
  - The referrer's account must be older than the friend's.
  - The referrer can't be someone the friend referred.
  - An unknown code is an error.
- **ICP only.** Chip tables and chip tournaments never reach `earn_rake`, so they give no cut.
- **Persistence:**
  - New `User` and heap fields are `Option<T>`.
  - A referrer's friends live in a stable-memory map in `crate::store`, never inside `User`.
  - Regenerate candid with `UPDATE_CANDID=1 cargo test`.
- **Calls:** state changes go through `caller()` on update methods, and reads are queries.
- **Commands:** run them inside WSL, as `MSYS_NO_PATHCONV=1 wsl -d Ubuntu-24.04 -- bash -lc 'cd /mnt/c/BulletsPoker && …'`.
  - Backend: `cargo test`.
  - Frontend: `cd src/poker_frontend && npx vitest run && npm run build`.
- **Copy:** "You get 20% of the house's cut of your friends' rake at ICP tables and ICP tournaments for 12 months. Paid every Monday."

## Review Focus

1. **Money conservation.** Wallets + tables + treasury + tournaments + jackpot + pools + referral owed must equal the ICP put in, across cuts, payouts and payouts withheld from suspended referrers. The treasury never goes negative, even when a tournament fee and its cut land in the same call.
2. **Weekly close idempotency.** Two ticks in the same second, a tick after weeks of downtime, or an upgrade mid-week must never pay a referrer twice or lose an owed balance.
3. **The 12-month boundary.** A cut stops exactly at `attached_at + 365 days`, and the boundary hand gives no cut.
4. **Attach edge cases.**
   - A code that differs only in case.
   - Your own username.
   - A referral loop.
   - A referrer created after the friend.
   - An account older than 24 h.
   - A second attach attempt.
   - An unknown code.
   - An anonymous caller.
5. **A referrer with thousands of friends.** The `get_referrals` query must stay bounded: page it, or cap the list it returns while the totals stay exact.

---

### Task 1: Attaching a referrer

**Files:**
- Create: `src/poker_backend/src/referrals.rs`, holding the module, the constants, `set_referrer` and its tests.
- Modify:
  - `src/poker_backend/src/lib.rs`: add `mod referrals;` and `#[update] set_referrer(code: String) -> R<()>`.
  - `src/poker_backend/src/state.rs`: add `User.referral: Option<ReferralLink>` and `User.referrer_stats: Option<ReferrerStats>`, both `None` at registration.
  - `src/poker_backend/src/store.rs`: add a stable map `referrals: StableBTreeMap<(Principal, Principal), FriendEntry, Mem>` keyed by (referrer, friend), on a new memory id.
  - The `.did` and the bindings, regenerated.

**Interfaces:**
- Produces:
  - `pub struct ReferralLink { pub referrer: Principal, pub attached_at: u64 }`.
  - `pub struct ReferrerStats { pub friends: u32, pub paid_total: u64, pub carry_bps: u64 }`. `carry_bps` holds the remainder below 1 e8s, in units of e8s × 10_000.
  - `pub struct FriendEntry { pub attached_at: u64, pub earned: u64, pub last_earned_at: u64 }`.
    - It is candid-encoded and `Storable`, following how `store.rs` stores `User`.
    - The tuple key `(Principal, Principal)` must be `Storable`. If ic-stable-structures doesn't provide that for tuples, use a fixed-layout byte key that keeps all of one referrer's friends adjacent for a range scan, and say which you chose.
  - `impl State { pub fn set_referrer(&mut self, friend: Principal, code: &str, now: u64) -> R<()> }`. It enforces every rule in Global Constraints, using these error strings:
    - "No player with that code"
    - "You can't refer yourself"
    - "Referral codes can only be added in your first 24 hours"
    - "You already have a referrer"
    - "That player joined after you"
    - "That player was referred by you"

    On success it writes `ReferralLink`, inserts the `FriendEntry`, and increments the referrer's `ReferrerStats.friends`.

- [ ] **Step 1: Write the failing tests** in `referrals.rs`:
  - `attaches_a_referrer_by_username_ignoring_case`
  - `refuses_self_unknown_late_second_newer_and_loops`: one `assert_eq!` per error string above.
  - `anonymous_caller_is_refused`, through `set_referrer`'s profile check: the caller must have a profile.
- [ ] **Step 2: Run** `cargo test referrals`. Expected: compile failure, because `set_referrer` is missing.
- [ ] **Step 3: Implement** the code in the files listed above. Then run `UPDATE_CANDID=1 cargo test`.
- [ ] **Step 4: Run** `cargo test`. Expected: everything passes.
- [ ] **Step 5: Commit** "Let players attach a referrer by code".

### Task 2: The cut and the weekly payout

**Files:**
- Modify:
  - `src/poker_backend/src/referrals.rs`.
  - `src/poker_backend/src/rewards.rs`: at the end of `earn_rake(p, rake_e8s, house_e8s, now)`, call `self.referral_cut(p, house_e8s, now)`.
  - `src/poker_backend/src/state.rs`:
    - Add the heap field `referrals: Option<ReferralState>`.
    - Call `close_referral_week(now)` in `tick`.
    - In `house_stats`, add `referral_owed: u64` to `HouseStats` and to `liabilities_total`.
  - The `.did` and the bindings, regenerated.

**Interfaces:**
- Consumes: `set_referrer` and the types from Task 1; `fund_rewards` and `earn_rake` from the rewards program; `crate::leaderboards::week_of`; `State::is_banned(p, now)`; `credit(&p, amount)`.
- Produces:
  - `pub struct ReferralState { pub owed: BTreeMap<Principal, u64>, pub owed_total: u64, pub week: u64 }`.
    - `week` is the week whose cuts `owed` holds.
    - The map is bounded by the referrers who earned this week.
  - `pub(crate) fn referral_cut(&mut self, friend: &Principal, house_e8s: u64, now: u64)`:
    1. Return if the friend has no `ReferralLink` or `now >= attached_at + REFERRAL_DURATION_NS`.
    2. Call `close_referral_week(now)` first.
    3. Compute the cut as `house_e8s × 2_000`, plus the referrer's `carry_bps`. Whole e8s are `/ 10_000`, and the rest stays in `carry_bps`.
    4. Take `cut = cut.min(self.treasury)` out of the treasury.
    5. Add it to `owed[referrer]` and `owed_total`.
    6. Add it to the `FriendEntry`'s `earned` and set `last_earned_at`.
  - `pub(crate) fn close_referral_week(&mut self, now: u64)`. If `week_of(now) > state.week`:
    - Take `owed`, and set `week = week_of(now)` *before* paying.
    - For each referrer: if suspended, return the amount to the treasury; otherwise `credit` it and add it to `paid_total`.
    - Subtract from `owed_total` exactly what was taken.
- The call order guarantees the treasury already holds the rake behind each cut. `fund_rewards` runs before `earn_rake` at all three call sites:
  - the cash hand sweep in `after_table_change`;
  - the tournament start;
  - late registration.

  Verify that order with a test, not by assumption.

- [ ] **Step 1: Write the failing tests** in `referrals.rs`, reusing the state test helpers (`setup`, `house_take`):
  - `friends_icp_rake_owes_the_referrer_a_fifth_of_the_house_share`: after ICP hands, `owed[referrer] == Σ house_e8s × 20%` (exact, including carry), and the treasury fell by the same amount.
  - `cut_comes_only_from_the_treasury`: the pools are equal to a run without referrals.
  - `tournament_fee_cut_at_start_and_late_entry`.
  - `no_cut_after_twelve_months`: the boundary is exact.
  - `chip_play_gives_no_cut`.
  - `weekly_close_pays_once_and_is_idempotent`:
    - Two ticks at the same `now` pay once.
    - A tick 5 weeks later pays once.
    - A cut recorded in a new week before any tick closes the old week first.
  - `suspended_referrer_is_not_paid_and_it_returns_to_the_treasury`.
  - `money_is_conserved_with_referrals`: use the full-recount invariant, as in the existing conservation tests, plus `referral_owed`.
  - `liabilities_include_referral_owed`.
- [ ] **Step 2: Run** these tests. Expected: FAIL.
- [ ] **Step 3: Implement.** Then run `UPDATE_CANDID=1 cargo test`.
- [ ] **Step 4: Run** `cargo test`. Expected: everything passes.
- [ ] **Step 5: Commit** "Pay referrers a weekly cut of their friends' rake".

### Task 3: Referrals view

**Files:**
- Modify:
  - `src/poker_backend/src/referrals.rs`.
  - `lib.rs`: add `#[query] get_referrals(offset: u32) -> ReferralView`.
  - The `.did` and the bindings, regenerated.

**Interfaces:**
- Produces:
  - `pub struct ReferralView`:
    - `code: String`: your username.
    - `friends: u32`.
    - `active_7d: u32`.
    - `owed_this_week: u64`.
    - `paid_total: u64`.
    - `pays_at: u64`: ns of the next Monday 00:00 UTC.
    - `rows: Vec<FriendRow>`.
    - `can_enter_code: bool`: within 24 h and no referrer yet.
    - `my_referrer: Option<String>`: username.
  - `pub struct FriendRow { username, joined_at, ends_at, earned: u64, active: bool }`.
  - The rows come from a range scan over your entries in the stable map, newest first, at most 50 per page starting at `offset`.
  - `active_7d` must be exact without decoding every friend: keep it on `ReferrerStats`, or compute it from the same scan only up to a fixed bound and document it. Choose one and say which.
  - An anonymous caller or a player with no profile gets the zero view.

- [ ] **Step 1: Write the failing tests:**
  - `view_lists_friends_newest_first_and_pages`: 120 friends give 50, then 50, then 20.
  - `view_totals_and_can_enter_code`.
  - `active_means_earned_in_the_last_seven_days`.
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.** Then run `UPDATE_CANDID=1 cargo test`.
- [ ] **Step 4: Run** `cargo test`. Expected: everything passes.
- [ ] **Step 5: Commit** "Show referral earnings and friends".

### Task 4: Referral links and the Referrals page

**Files:**
- Create:
  - `src/poker_frontend/src/referrals.ts` and `referrals.test.ts`.
  - `src/poker_frontend/src/components/Referrals.tsx`.
- Modify:
  - `App.tsx`: add a NAV entry `referrals`, labelled "Referrals".
  - `context.tsx`: add the `View` entry.
  - `components/Profile.tsx`: in `Register`, after a successful `register`, attach the stored code.
  - `main.tsx` or the app entry: capture `?ref=` on load.
  - `styles.css`.

**Interfaces:**
- Consumes: `set_referrer`, `get_referrals` and the generated types.
- Produces (`referrals.ts`):
  - `readRefParam(search: string): string | null`, trimmed. It returns null for an empty value or one longer than 32 characters.
  - `rememberRef(code)` and `takeRememberedRef(): string | null`. They use localStorage key `bp.ref`, inside try/catch, and `take` clears the key.
  - `referralLink(origin: string, code: string): string`, giving `${origin}/?ref=${encodeURIComponent(code)}`.

- [ ] **Step 1: Write the failing tests** in `referrals.test.ts`:
  - `readRefParam('?ref=Mark')` → 'Mark'; `('?x=1')` → null; `('?ref=')` → null.
  - `referralLink('https://a.b', 'Mark Al')` → 'https://a.b/?ref=Mark%20Al'.
  - remember then take returns the code once, and null the second time. Stub localStorage in the test.
- [ ] **Step 2: Run** `npx vitest run`. Expected: FAIL, because the module is missing.
- [ ] **Step 3: Implement** the helpers and the page:
  - Capture `?ref=` on load and remember it.
  - After registering, `takeRememberedRef()`. If there is a code, call `set_referrer`. On Err, show a notice ("Referral code not found — you can enter one on the Referrals page within 24 hours"). Signup itself never fails because of the code.
  - Referrals page:
    - the link with a copy button (`navigator.clipboard`, with a select-text fallback);
    - the totals (owed this week, paid total, next payout countdown);
    - the friends table with "Load more";
    - a code entry box while `can_enter_code`;
    - the copy line from Global Constraints.
  - Amounts use the existing `prizeIcp` helper, so a small non-zero amount never shows as 0.
  - Poll with `usePoll` every 30 s.
- [ ] **Step 4: Run** `npx vitest run && npm run build`. Expected: everything passes.
- [ ] **Step 5: Commit** "Add referral links and the Referrals page".

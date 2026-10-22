# Launch Audit Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the twelve application defects from the independent pre-launch audit, plus the two launch-gate items that are code (the status banner wording and dependency advisory scanning).

**Architecture:** Each task maps to one or two audit findings and touches one area: table clock, betting revision, departures, seat tracking, wallet/table UI, deposit admission, startup, accessibility, CI. Backend tasks come first, because the seat-tracking and wallet UI tasks consume their new APIs.

**Tech Stack:** Rust (ic-cdk, candid, ic-stable-structures), React + TypeScript, vitest, GitHub Actions.

**Spec:** The audit report, `C:\Users\user\.claude\uploads\b6e3a36c-93de-438c-8e7e-4981b4f97b84\a1f2f1a1-BulletsPoker-launch-audit.md`. Its "Fix" paragraphs and "Release acceptance checks" are binding. Line references in it are to commit `e109da5`. This branch (`audit-fixes`, from `origin/referrals` at `26f9e05`) also contains the referral and welcome-bonus work, so line numbers differ.

## Global Constraints

- **Persistence:** new persisted fields (`Table`, `Seat`, `User`, heap) are `Option<T>` or otherwise decode from old data. Regenerate candid with `UPDATE_CANDID=1 cargo test`. New trailing endpoint arguments are `opt`, so existing callers keep working.
- **Calls:** state changes go through `caller()` on update methods, and reads are queries.
- **Money:** no change may break the existing conservation tests. Every chip a departing player owns is credited exactly once.
- **Commands:** run them inside WSL, as `MSYS_NO_PATHCONV=1 wsl -d Ubuntu-24.04 -- bash -lc 'cd /mnt/c/BulletsPoker && …'`.
  - Backend: `cargo test`, unfiltered at least once, because the candid check runs only in the full suite.
  - Frontend: `cd src/poker_frontend && npx vitest run && npm run build`. No `npm install`.
- **Out of scope:** the audit's real-money launch gates 1 (eligibility and responsible gambling), 2 (card confidentiality) and the operational verification in 3. They need the owner's decisions. Only the banner wording from gate 3 and the advisory scanning from gate 4 are in scope.

## Controller decisions

The audit leaves these choices open; these are the ones taken.
- **Finding 5:** standing up during a hand becomes "leave when this hand ends".
  - The player is marked as leaving and sitting out, so their turns check or fold automatically. An all-in player keeps their showdown entitlement.
  - When the hand ends they are stood up and credited.
  - There is no separate "fold and leave now" button.
- **Finding 4:** a table where you are seated can't be hidden. Its ✕ opens a leave flow ("Leave when this hand ends" / "Keep playing"). Watched-only tables close as before.
- **Finding 3 and 4 capacity:** seated tables (cash and tournament) always stay open. The four-table limit counts only tables you are watching.
- **Finding 8:** withdrawals support a principal plus an optional 32-byte subaccount, entered as hex or in ICRC-1 textual account format. Legacy hex account identifiers are not supported, and the form says so.
- **Finding 7:** settled failed deposit records with no money effect are pruned after 30 days. Unresolved and successful records are kept forever.

## Review Focus

1. **Turn clock:** no sequence of `sit_out` true/false calls, while the regular clock or the time bank is running, extends the turn or refunds time-bank time.
2. **Stale commands:** a command carrying an old revision never applies, whether it comes from an earlier street, an earlier hand or a duplicate delivery.
3. **Leave mid-hand:**
   - an all-in player keeps their main-pot and side-pot entitlement;
   - chips are credited once, at hand end;
   - it works for tournament tables, moderation removal and idle stand-ups;
   - a seated leaver can't be re-seated before the hand ends.
4. **Seats:** a player is never seated somewhere their client has stopped watching, after tournament balancing or a table merge, sign-in on a new device, or sign-out and back in.
5. **Busy flags:** no failed refresh leaves betting or wallet controls disabled.

---

### Task 1: Fixed turn clock (audit finding 1)

**Files:**
- Modify: `src/poker_backend/src/table.rs` (`set_sitting_out`, `set_deadline`, `handle_timeout`, the `Table` struct) and its tests.

**Interfaces:**
- Produces: `Table.turn_deadline: Option<u64>`, the regular-time deadline set when a turn starts (in `set_deadline` when the actor changes).
- **`set_sitting_out(p, out, now)`:**
  - A redundant update (the same value) is a no-op that changes nothing.
  - Sitting out during your own turn:
    - if the time bank is active, first charge it for the time used since `bank_started_at`, as `act` does;
    - then set `action_deadline = now` (auto-act).
  - Coming back during your own turn sets `action_deadline` to the remaining budget, never more than before sitting out:
    - if the time bank had not started, use `turn_deadline`;
    - if it had, use `now` plus the bank left.
  - `time_bank_active` stays as it was, or false if the bank was charged out.

- [ ] **Step 1: Write the failing tests** in `table.rs`:
  - `repeated_back_calls_do_not_move_the_deadline`
  - `sit_out_and_back_does_not_extend_the_turn`
  - `toggling_during_the_time_bank_charges_the_bank`
  - `coming_back_after_auto_sit_out_on_a_later_turn_gets_normal_time`, which keeps the existing behaviour on a new turn.
  - Update the existing test `coming_back_mid_hand_gets_a_fresh_clock` to the new rule: it must not get a fresh clock. Rename it accordingly.
- [ ] **Step 2: Run** `cargo test table`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test`. Expected: everything passes.
- [ ] **Step 5: Commit** "Never extend a turn by sitting out and coming back".

### Task 2: Betting commands carry the turn they were made for (audit finding 2)

**Files:**
- Modify:
  - `src/poker_backend/src/table.rs`: `Table.action_seq: Option<u64>`, incremented on every change that alters the actor or the amount to call: each action, each new street, each new hand, auto-acts, departures and top-ups during a hand.
  - `state.rs`: `TableView.action_seq: u64`, and `State::act` takes `expected: Option<u64>`.
  - `lib.rs`: `act(id: u64, action: Action, expected: Option<u64>)`.
  - `src/poker_frontend/src/components/Table.tsx`: send `[view.action_seq]` with every action.
  - The `.did` and the bindings, regenerated.

**Interfaces:**
- **`Table::act`:** when `expected` is `Some(n)` and `n != action_seq`, return `Err("The table moved on — check the new situation and act again")` before any mutation.
- **`None`** keeps the old behaviour, for old clients only. The frontend always sends `Some`.

- [ ] **Step 1: Write the failing tests:**
  - `stale_call_from_an_earlier_street_is_rejected`
  - `stale_fold_from_an_earlier_hand_is_rejected`
  - `duplicate_delivery_applies_once`
  - `current_revision_is_accepted`
  - `revision_changes_when_the_amount_to_call_changes`
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.** Then run `UPDATE_CANDID=1 cargo test`.
- [ ] **Step 4: Run** `cargo test`, `npx vitest run` and `npm run build`. Expected: everything passes.
- [ ] **Step 5: Commit** "Reject betting commands made for an earlier turn".

### Task 3: Leave when the hand ends (audit finding 5)

**Files:**
- Modify:
  - `src/poker_backend/src/table.rs` (`stand`, `Seat.leaving: Option<bool>`, hand-end handling).
  - `state.rs` (`leave_table`, the hand-end hook in `after_table_change`, and the moderation and idle stand-up paths).
  - `src/poker_frontend/src/components/Table.tsx` (the button and its confirmation).

**Interfaces:**
- **`Table::stand(p, now) -> Result<StandOutcome, String>`**, with `enum StandOutcome { Left(u64 /*chips*/), AfterHand }`.
  - Not in a live hand: leave at once, as today.
  - In a live hand: set `leaving` and `sitting_out`. If it's their turn, auto-act through the existing sitting-out path. The player is never folded just for leaving.
- **`Table::take_leavers(&mut self) -> Vec<(Principal, u64)>`:** called when the hand is over. It removes leaving seats and returns their stacks.
- **`State::leave_table(...) -> R<LeaveResult>`**, where `LeaveResult` is a candid variant `Left : nat64 | AfterHand`.
  - `after_table_change` credits the leavers at hand end, records `cashouts`, and posts "X left the table".
  - The moderation and idle stand-up paths use the same mechanism.
- **Re-joining:** a leaving player can't join or top up that table again until the hand ends.
- **Tournament tables:** keep whatever leaving rules they already have. Check them, and say what they are.
- **Frontend:**
  - The button reads "Leave table".
  - In a hand, it asks for confirmation: "You'll leave when this hand ends. Your remaining turns are checked or folded for you; an all-in hand still plays to showdown." It then shows "Leaving after this hand…".

- [ ] **Step 1: Write the failing tests:**
  - `all_in_player_who_leaves_still_wins_the_showdown`: covers the main pot and a side pot.
  - `leaver_is_credited_once_at_hand_end`
  - `leaver_whose_turn_it_is_is_checked_or_folded`
  - `leaving_between_hands_is_immediate`
  - `moderation_removal_keeps_all_in_entitlement`
  - `leaver_cannot_rejoin_until_the_hand_ends`
  - Conservation: the existing full-recount tests still pass.
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.** Then run `UPDATE_CANDID=1 cargo test`.
- [ ] **Step 4: Run** `cargo test`, `npx vitest run` and `npm run build`. Expected: everything passes.
- [ ] **Step 5: Commit** "Leave at the end of the hand instead of forfeiting it".

### Task 4: Never lose track of a seat (audit findings 3 and 4)

**Files:**
- Modify:
  - `src/poker_backend/src/state.rs` and `lib.rs`: `#[query] get_my_seats() -> Vec<SeatRef>`, where `SeatRef { table: u64, tournament: Option<u64>, leaving: bool }`. It covers every table where the caller holds a seat, cash or tournament.
  - `src/poker_frontend/src/tables.tsx`: reconcile against `get_my_seats` every 3 s and on sign-in.
  - `src/poker_frontend/src/tablesNav.ts` and its tests.
  - `src/poker_frontend/src/components/MultiTable.tsx` (the ✕).
  - `src/poker_frontend/src/context.tsx` (sign-out and sign-in).
  - The `.did` and the bindings, regenerated.

**Interfaces:**
- **`tablesNav.ts`:** `reconcileSeats(open: bigint[], seats: bigint[], watchLimit: number): { open: bigint[]; added: bigint[] }`.
  - Every seated table is in `open`.
  - Only watched tables count toward `watchLimit` (4).
  - Seats are never dropped.
- **Behaviour:**
  - A newly seated tournament table is added with a persistent notice: "You were moved to table N". It stays until dismissed or until the player opens that table.
  - The ✕ on a seated table opens the Task 3 leave flow instead of hiding the table.
  - Auto-sit cancellation on close shows its error and retries once; on a second failure it shows a persistent "Couldn't leave the waiting line — try again" notice.
  - Sign-in rebuilds the open tables from `get_my_seats`. Sign-out still clears local state, and the next sign-in restores it from the server.

- [ ] **Step 1: Write the failing tests:**
  - Backend: `my_seats_lists_cash_and_tournament_seats`, and `my_seats_follows_a_tournament_move`, which uses the existing balancing test setup.
  - Frontend (`tablesNav.test.ts`): `reconcileSeats` adds a seated table even at the watch limit, keeps seats, and evicts a watched table, not a seated one.
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.** Then run `UPDATE_CANDID=1 cargo test`.
- [ ] **Step 4: Run** `cargo test`, `npx vitest run` and `npm run build`. Expected: everything passes.
- [ ] **Step 5: Commit** "Follow every seat, including tournament moves and new devices".

### Task 5: Wallet and table UI fixes (audit findings 6, 8, 9 and 10, plus the gate 3 banner)

**Files:**
- Modify:
  - `src/poker_frontend/src/components/Table.tsx`: the `act` and `showCards` busy flags in `finally`, and the ICP tournament prize format.
  - `components/Wallet.tsx`: busy flags in `finally`; post-commit refresh failures reported separately; deposit cost totals; external-deposit net and minimum; withdrawal review; unknown-operation state.
  - `App.tsx`: the pause banner wording.
- Create: `src/poker_frontend/src/walletMath.ts` and `walletMath.test.ts`.

**Interfaces:**
- **`walletMath.ts`:**
  - `depositCost(amount, fee) -> { approveFee, pullFee, total }`. ICRC-2 needs an approve fee plus the transfer-from fee; the total the wallet must hold is `amount + 2 × fee`.
  - `sweepNet(amount, fee) -> bigint`.
  - `parseDestination(text) -> { owner: Principal; subaccount: Uint8Array | null } | { error: string }`. It accepts a principal, a principal plus `.`/space plus 64 hex characters, or the ICRC-1 textual account format. It rejects 64-hex legacy account IDs with "Legacy ICP account IDs aren't supported — use a principal".
  - `unknownOpState(op, nowMs) -> 'confirming' | 'needs_support'`. It returns `needs_support` once the op is older than the automatic retry window (23 h, from `wallet.rs`).
- **Withdrawals:**
  - A review step shows the full destination, the network (ICP ledger), the amount, the fee and the total debit, with Confirm and Back.
  - It validates the amount (non-zero, within the balance) before review.
- **Unknown operations:** each row shows the operation ID with a copy button, its age and the last attempt.
  - While confirming, it says "Confirming with the ledger — usually within minutes".
  - Once it needs support, it says "Needs support review — contact support with operation #N" and shows the support line from the existing footer or contact link. If none exists, it uses a `SUPPORT_CONTACT` constant in one place.
- **ICP tournament prize:** use the tournament's currency.
- **Pause banner:** replace "Your balance is safe." with accurate wording for each pause reason, e.g. "Withdrawals are paused while we check balances. Nothing is lost; withdrawals resume after the check."

- [ ] **Step 1: Write the failing tests** in `walletMath.test.ts` for all four helpers, plus a test that the prize formatting helper uses the event currency.
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `npx vitest run && npm run build`. Expected: everything passes.
- [ ] **Step 5: Commit** "Fix stuck controls and make wallet costs, withdrawals and pending transfers clear".

### Task 6: Limit failed deposit records (audit finding 7)

**Files:**
- Modify: `src/poker_backend/src/wallet.rs` and `ratelimit.rs`, with their tests.

**Interfaces:**
- **Per user:** after 5 failed deposits or sweeps in an hour, `begin_deposit` and `begin_sweep` return `Err("Too many failed deposits — try again in N minutes")`. Successful deposits reset the count.
- **Everyone together:** at most 500 failed deposit records per hour across all users. Past that, new deposit attempts are refused with a "busy, try again shortly" message, and successful ones continue.
- **Retention:** `prune_failed_deposits(now)` runs from the existing 60 s timer. It removes Deposit and Sweep records that are `Failed`, have no money effect and are older than 30 days, in batches of at most 500 per run. It never touches Pending, Unknown, Completed or Withdraw records, and it drops pruned IDs from `User.ops`.

- [ ] **Step 1: Write the failing tests:**
  - `repeated_failed_deposits_back_off`
  - `aggregate_failure_budget`
  - `prune_removes_only_old_failed_deposits`
  - `prune_keeps_unresolved_and_completed`
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test`. Expected: everything passes.
- [ ] **Step 5: Commit** "Back off repeated failed deposits and prune old failures".

### Task 7: Startup that can recover (audit finding 12)

**Files:**
- Modify: `src/poker_frontend/src/context.tsx` and `App.tsx`.
- Create: `src/poker_frontend/src/startup.ts` and `startup.test.ts`.

**Interfaces:**
- **`startup.ts`:**
  - `type Startup = 'connecting' | 'failed' | 'ready'`
  - `type ProfileLoad = 'unknown' | 'loading' | 'none' | 'found' | 'failed'`
  - `nextRetryMs(attempt) -> number`: exponential backoff, 1 s, 2 s, 4 s, …, capped at 30 s.
- **Behaviour:**
  - Connection failure shows a screen with the error and a Retry button. It also retries automatically on the backoff.
  - The toast container renders in every state.
  - Profile lookup failure shows "Couldn't load your profile" with Retry.
  - The registration prompt appears only when `ProfileLoad === 'none'`.

- [ ] **Step 1: Write the failing tests:** `nextRetryMs`, and a pure `shouldOfferRegistration(load)` that is true only for `'none'`.
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `npx vitest run && npm run build`. Expected: everything passes.
- [ ] **Step 5: Commit** "Show a retry screen when the app can't connect or load your profile".

### Task 8: Accessibility pass (audit finding 11)

**Files:**
- Modify:
  - `components/JoinGame.tsx` (the buy-in dialog), `components/CreateRoom.tsx` and any other `modal-bg` dialogs.
  - `components/MultiTable.tsx` (the table selectors).
  - `components/Wallet.tsx` and `components/Table.tsx` (labels and sliders).
  - The toast container in `context.tsx` or `App.tsx`.
- Create: `src/poker_frontend/src/components/Dialog.tsx`, a shared accessible dialog.

**Interfaces:**
- **`Dialog`:** props `{ title: string; onClose: () => void; children }`.
  - It renders `role="dialog"`, `aria-modal="true"` and `aria-labelledby`.
  - Focus moves into the dialog on open and is contained while open; Escape closes it; focus returns to the opener on close.
  - Every existing modal uses it.
- **Other changes:**
  - Clickable `div`s become `button`s.
  - Every money input and slider gets a visible associated `<label>` (or `aria-label` on sliders) and `aria-valuetext` with the amount.
  - Error toasts use `role="alert"` and info toasts `role="status"`, inside an `aria-live` region.

- [ ] **Step 1: Write the failing tests** with vitest and the existing render setup in `art.test.tsx`. Render `Dialog` and check the role and attributes, that Escape calls `onClose`, and that focus moves into it.
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `npx vitest run && npm run build`. Expected: everything passes.
- [ ] **Step 5: Commit** "Make dialogs, table tabs, money fields and alerts accessible".

### Task 9: Dependency advisory scanning (audit gate 4)

**Files:**
- Modify: `.github/workflows/ci.yml`.

**Interfaces:**
- **A new `audit` job**, using the same pinned-SHA action style and read-only permissions as the existing jobs:
  - `cargo audit` on `Cargo.lock`, through `rustsec/audit-check` pinned by SHA or `cargo install cargo-audit --locked`.
  - `npm audit --audit-level=high` in `src/poker_frontend` against `package-lock.json`.
- **Reporting:** findings fail the job. Record any current advisories in the report; don't silence them.

- [ ] **Step 1: Run** both scans locally in WSL and record the results.
- [ ] **Step 2: Add the job.** Fix or document any advisories found: upgrade where a patch exists, otherwise use a dated, explained ignore.
- [ ] **Step 3: Commit** "Scan Rust and npm dependencies for advisories in CI".

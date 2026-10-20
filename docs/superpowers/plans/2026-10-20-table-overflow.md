# Table Overflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A full cash game opens copies automatically. The lobby shows one row per game with its copies folded inside, and a watcher can join an auto-sit queue for a table.

**Architecture:**
- **Copies:** each copy is an ordinary `Table` with the original's `TableConfig`, plus `game` and `copy_number` fields. Grouping, caps and cleanup live in `State`.
- **Joining:** `join_game` picks or opens a table, then reuses `join_table`'s checks and seating.
- **Auto-sit:** the queue is per-table data on `Table`. `tick` seats queued players and closes empty copies.
- **Frontend:** groups `list_tables` rows by `game` with a pure, tested function.

**Tech Stack:** Rust IC canister (candid, ic-cdk), React 19 + TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-10-20-table-overflow-design.md`. Read it first.

## Global Constraints

- **Upgrade safety:** every new field on a persisted struct (`Table`, or anything in `heap_state!`) is an `Option<…>`. The snapshot is decoded with candid, where a missing `opt` field reads as `None` and any other missing field fails the upgrade. Candid changes to the API are additive.
- **Copy caps:**
  - `MAX_COPIES_PER_GAME = 50`
  - `MAX_CHIP_COPIES = 200`, counting copies of `Currency::Chips` games
  - `MAX_ICP_COPIES = 100`, counting copies of `Currency::Icp` games
- **Copy cleanup:** `COPY_IDLE_CLOSE_NS = 60 s`. Originals never close.
- **Auto-sit timing:**
  - `AUTO_SIT_PING_SECS = 20` on the frontend.
  - Drop an entry not seen for `AUTO_SIT_EXPIRE_NS = 60 s`.
  - Seat only if seen within `AUTO_SIT_LIVE_NS = 25 s`.
  - `AUTO_SIT_MAX_QUEUE = 100`. One queue per player.
- **Same game, several tables:** allowed, up to `MAX_CASH_SEATS` (4). `join_game` never picks a table the caller already sits at.
- **First click wins:** `join_table` (a manual seat click) ignores queues. `join_game` skips tables whose queue is non-empty.
- **Buy-back rule:** keyed per game, so `cashouts` uses key `(game_of(table), player)`.
- **Out of scope:**
  - Rooms (`rooms` map): never copied, never grouped, and they keep their own flow.
  - Tournament tables (`table_tournament`): untouched.
- **Commands:** cargo and npm run inside WSL Ubuntu-24.04, from `/mnt/c/BulletsPoker`:
  - `cargo test`
  - `UPDATE_CANDID=1 cargo test` regenerates `.did` and the bindings.
  - From `src/poker_frontend`: `npm test` and `npm run build`.

## Review Focus

1. A copy closes (60 s empty) while someone is watching it, or has the buy-in dialog open. `join_table`/`auto_sit` on it should return "No such table", and the page should show "This table closed" instead of loading forever. Tested in Tasks 1 and 5.
2. A caller already sits at every table of a game that has a free seat. `join_game` should open a new copy, not error and not double-seat. Tested in Task 2.
3. Two queued players and one seat opens. Only the first live one is seated, and the second stays queued at place 1. Tested in Task 3.
4. A room id or a tournament table id is passed to `join_game`, or a room is full. There should be no copy, and the error should be clear. Tested in Task 2.
5. Upgrading from a snapshot written before this change, where `Table` has no `game`/`copy_number`/`empty_since`/`auto_sit` and the heap has no `auto_sit_outcomes`. Decoding should succeed with `None`. Tested in Task 1 (candid opt decode) and Task 3 (heap field).

---

### Task 1: Backend — game identity, copies, caps, cleanup, game-wide buy-back

**Files:**
- Modify: `src/poker_backend/src/table.rs`, adding `Table` fields and `first_free_seat`.
- Modify: `src/poker_backend/src/state.rs`:
  - the constants near line 35,
  - `TableSummary` (~512) and `TableView` (~473),
  - `list_tables` (~1956) and `full_table_view`,
  - `leave_table` (~1590) and `min_return_buy_in` (~1627),
  - `tick` (~1900),
  - new methods next to `create_table` (~1434).
- Regenerate: `.did` and bindings.

**Interfaces:**
- **Produces in `table.rs`:**
  - `Table` fields:
    - `pub game: Option<u64>`, the original's id for a copy and `None` for originals and rooms,
    - `pub copy_number: Option<u8>`, which is `None` for Table 1,
    - `pub empty_since: Option<u64>`.

    `Table::new` sets all three to `None`.
  - `pub fn first_free_seat(&self) -> Option<u8>`, the lowest index whose seat is `None`.
- **Produces in `state.rs`:**
  - `pub fn game_of(&self, table: u64) -> u64`, which is `game.unwrap_or(table)`.
  - `pub fn game_tables(&self, game: u64) -> Vec<u64>`, the tables whose `game_of == game`, excluding tournament tables, sorted by copy number.
  - `fn is_copyable_game(&self, game: u64) -> bool`. True when the table exists, is its own game, is not a room and is not a tournament table.
  - `fn can_open_copy(&self, game: u64) -> bool`. True when the game is copyable and within all three caps.
  - `pub fn open_copy(&mut self, game: u64, now: u64) -> R<u64>`. It creates the copy through `create_table(config.clone())`, sets `game = Some(game)`, sets `copy_number` to the lowest unused number ≥ 2, and sets `empty_since = Some(now)`.
    - At a cap it returns `"<name> is full right now. Watch a table and turn on auto-sit to get the next seat."`
  - `fn close_empty_copies(&mut self, now: u64)`, called from `tick`. For a copy:
    - with `player_count() == 0`, it sets `empty_since` if unset, and removes the copy through `remove_table` once `now - since >= COPY_IDLE_CLOSE_NS`;
    - otherwise it clears `empty_since`.
  - `TableSummary` and `TableView` gain:
    - `game: u64`,
    - `number: u8`, where 1 means the original,
    - `game_at_cap: bool`, which is `!can_open_copy(game)`. On rooms it's `true`.
  - `cashouts` keys become `(game_of(id), player)` in `leave_table`, and `min_return_buy_in` looks them up the same way. For originals the key is unchanged, since the original's game is its own id.

- [ ] **Step 1: Write the failing tests** in `state.rs` `mod tests`, using `setup()` and `p(n)`. Heads Up Hangout is table 3.
  - `copies_share_config_and_number_up`:
    - `open_copy(3, 0)` twice gives copies numbered 2 and 3.
    - Each copy has the same `TableConfig` as table 3 and `game_of` equal to 3.
    - `game_tables(3)` returns `[3, c2, c3]`.
  - `copy_numbers_reuse_the_lowest_free`: remove copy 2 with `close_empty_copies`; the next `open_copy` is numbered 2.
  - `copy_caps`:
    - The 51st `open_copy(3)` fails.
    - With `MAX_CHIP_COPIES` reached across chip games, `open_copy` on a chip game fails, while `open_copy` on an ICP game (table 5) still succeeds.
    - Build the state by calling `open_copy` in loops.
  - `rooms_and_tournament_tables_are_not_copyable`: `open_copy` on a `create_room` id fails, and so does `open_copy` on a copy's own id.
  - `empty_copy_closes_after_a_minute`:
    - A copy created at 0 is still there after `tick(59 * SEC)` and gone after `tick(60 * SEC)`.
    - The original table 3, empty, is never removed.
  - `occupied_copy_never_closes`: `p(1)` joins the copy. After `tick(10 * 60 * SEC)` the copy still exists.
    - Use a time below `IDLE_STAND_NS`, or have `p(1)` act, so the idle stand-up doesn't empty the table first.
  - `buy_back_minimum_covers_the_whole_game`: `p(1)` joins table 3 with 1,000 and leaves. A `join_table` on a copy of 3 with less than 1,000 within 30 min fails with the existing buy-back message.
  - `joining_a_closed_copy_fails_cleanly` (Review Focus 1): after the copy is removed, `join_table(copy, …)` returns `Err("No such table")`, and the caller's balance is unchanged.
  - `summaries_carry_game_and_number`: `list_tables()` shows the copy with `game == 3`, `number == 2`, and `game_at_cap == false`.
  - `old_snapshots_decode_new_options_as_none` (Review Focus 5):
    - Encode `candid::encode_one(Old { a: 1u64 })`, where `#[derive(CandidType)] struct Old { a: u64 }`.
    - Decode it as `#[derive(Deserialize)] struct New { a: u64, b: Option<u64> }`.
    - Assert `b == None`. This pins the upgrade assumption the Global Constraints rely on.
- [ ] **Step 2:** Run `cargo test`. Expected: compile errors for the missing methods and fields.
- [ ] **Step 3:** Implement the interfaces above. Add the constants `COPY_IDLE_CLOSE_NS`, `MAX_COPIES_PER_GAME`, `MAX_CHIP_COPIES` and `MAX_ICP_COPIES` next to `ROOM_IDLE_CLOSE_NS`. Call `close_empty_copies(now)` in `tick` after `close_idle_rooms`.
- [ ] **Step 4:** Run `UPDATE_CANDID=1 cargo test`, then `cargo test`. Expected: all pass, and the bindings show the new `TableSummary`/`TableView` fields.
- [ ] **Step 5:** Run `npm test` and `npm run build` in `src/poker_frontend`. Expected: pass, since the new fields are additive.
- [ ] **Step 6:** Commit with message `Add table copies with caps, cleanup and a game-wide buy-back rule`.

---

### Task 2: Backend — `join_game`

**Files:**
- Modify: `src/poker_backend/src/state.rs` (new method plus tests) and `src/poker_backend/src/lib.rs` (endpoint after `join_table`, ~line 547).
- Regenerate: `.did` and bindings.

**Interfaces:**
- **Consumes:** `game_tables`, `is_copyable_game`, `open_copy` and `first_free_seat` (Task 1), and the existing `join_table`.
- **Produces:**
  - `pub fn join_game(&mut self, caller: Principal, game: u64, buy_in: u64, now: u64) -> R<u64>` in `state.rs`.
  - `#[update] fn join_game(game: u64, buy_in: u64) -> R<u64>` in `lib.rs`. It calls `caller()?` for the rate limit.
  - `pub(crate) fn join_candidates(&self, caller: &Principal, game: u64) -> Vec<u64>`. It lists the game's tables with a free seat where the caller isn't seated, ordered by players descending and then copy number ascending. Task 3 adds a queue filter here.
- **How `join_game` works:**
  1. If `!is_copyable_game(game)`, return `Err("Join this table directly")`. This covers rooms and tournament tables.
  2. Before opening anything, run the cheap checks so a failing caller can't create copies:
     - the pause flags,
     - `check_not_banned`,
     - `MAX_CASH_SEATS`,
     - `buy_in` within `[config.min_buy_in, config.max_buy_in]`,
     - `buy_in >= min_return_buy_in(game, …)`,
     - the wallet or chip balance covering `buy_in`.
  3. Take the first of `join_candidates`, or else `open_copy(game, now)?`.
  4. Call `self.join_table(caller, id, first_free_seat, buy_in, None, now)?`, then return `id`.

- [ ] **Step 1: Write the failing tests** in `state.rs`:
  - `join_game_fills_the_fullest_table_first`: on table 1 (a 9-max chip game), open a copy. `p(1)` and `p(2)` sit at the copy. `join_game(p(3), 1, …)` returns the copy's id.
  - `join_game_opens_a_copy_when_full` (heads-up table 3): `p(1)` and `p(2)` fill it. `join_game(p(3), 3, …)` returns a new copy id with `game_of == 3`. `join_game(p(4), 3, …)` then returns that same copy, so tables fill before a new one opens.
  - `join_game_skips_tables_you_already_sit_at` (Review Focus 2): `p(1)` sits at table 3, which has a free seat. `join_game(p(1), 3, …)` returns a new copy, and `p(1)` now sits at 2 tables of the game.
  - `join_game_rejects_rooms_and_tournament_tables` (Review Focus 4):
    - On a `create_room` id it returns `Err` and the table count is unchanged.
    - On a tournament table id it does the same. Find one by starting a sit-and-go, or by picking any key of `table_tournament` after `register_tournament` fills one.
  - `join_game_checks_money_before_opening_a_copy`: with table 3 full, a caller with too few chips gets `Err`, and no copy was created (`game_tables(3).len() == 1`).
  - `join_game_at_cap_says_full`: with `MAX_COPIES_PER_GAME` copies of 3 and all of them full, the error contains `"is full right now"`.
- [ ] **Step 2:** Run `cargo test`. Expected: FAIL to compile.
- [ ] **Step 3:** Implement the method and the endpoint.
- [ ] **Step 4:** Run `UPDATE_CANDID=1 cargo test`, then `cargo test`. Expected: PASS.
- [ ] **Step 5:** Commit with message `Add join_game: seat at the fullest open table or a new copy`.

---

### Task 3: Backend — auto-sit queue

**Files:**
- Modify: `src/poker_backend/src/table.rs` (`Table` field plus the `AutoSitEntry` type).
- Modify: `src/poker_backend/src/state.rs`:
  - the heap field in `heap_state!`,
  - the types and methods,
  - `tick`, `join_table`, `join_candidates` and `table_view`.
- Modify: `src/poker_backend/src/lib.rs` (2 endpoints).
- Regenerate: `.did` and bindings.

**Interfaces:**
- **Produces in `table.rs`:**
  - `#[derive(CandidType, Deserialize, Clone, Debug)] pub struct AutoSitEntry { pub player: Principal, pub buy_in: u64, pub last_seen: u64 }`
  - The `Table` field `pub auto_sit: Option<VecDeque<AutoSitEntry>>`, where `None` means empty.
- **Produces in `state.rs`:**
  - `#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)] pub enum AutoSitStatus { NotQueued, Queued { place: u32, waiting: u32, buy_in: u64 }, Seated { table: u64 }, Dropped { reason: String } }`
  - `#[derive(CandidType, Deserialize, Clone, Debug)] pub struct AutoSitView { pub place: u32, pub buy_in: u64 }`
  - The heap field `auto_sit_outcomes: Option<BTreeMap<Principal, AutoSitStatus>>`, holding one-shot `Seated`/`Dropped` results.
  - `pub fn auto_sit(&mut self, caller: Principal, table: u64, buy_in: Option<u64>, now: u64) -> R<AutoSitStatus>`.
    - `None` removes the caller from every queue and returns `NotQueued`.
    - `Some(b)`:
      - Validate that the table is a cash table, that the caller isn't seated there, that `b` is within the config limits and at least the game's buy-back minimum, and that the balance is at least `b` (a check only, no debit).
      - Remove the caller from every other queue. If they're already queued here, update `buy_in` and `last_seen` and keep their place. Otherwise push them, failing with `"The line for this table is full"` at 100 entries.
      - Return `Queued`.
  - `pub fn auto_sit_ping(&mut self, caller: Principal, table: u64, now: u64) -> AutoSitStatus`.
    - If `auto_sit_outcomes` holds an entry for the caller, remove and return it.
    - Otherwise, if the caller is queued at `table`, set `last_seen = now` and return `Queued`.
    - Otherwise return `NotQueued`.
  - `fn seat_from_queues(&mut self, now: u64)`, called in `tick` before the per-table loop. For each table with a queue:
    1. Drop entries with `now - last_seen > AUTO_SIT_EXPIRE_NS`.
    2. While `first_free_seat()` is `Some`, take the first entry with `now - last_seen <= AUTO_SIT_LIVE_NS`, stopping if there is none.
    3. Remove it and call `join_table(player, id, seat, buy_in, None, now)`.
    4. Record `Seated { table: id }` on success, or `Dropped { reason: e }` on `Err(e)`.
  - `join_table`: after a successful seat, remove the caller from that table's queue (first click wins).
  - `join_candidates`: exclude tables whose `auto_sit` is non-empty.
  - `TableView` gains `my_auto_sit: Option<AutoSitView>` (the viewer's place and buy-in) and `auto_sit_waiting: u32`.
- **Produces in `lib.rs`:**
  - `#[update] fn auto_sit(table: u64, buy_in: Option<u64>) -> R<AutoSitStatus>`
  - `#[update] fn auto_sit_ping(table: u64) -> R<AutoSitStatus>`. It calls `caller()?` and wraps the result in `Ok`.

- [ ] **Step 1: Write the failing tests** in `state.rs`:
  - `auto_sit_seats_the_first_live_player_when_a_seat_opens` (Review Focus 3):
    - Table 3 is full with `p(1)` and `p(2)`.
    - `p(3)`, then `p(4)`, call `auto_sit(…, Some(1000))`. Their places are 1 and 2.
    - `p(2)` leaves, then `tick(now)` runs. `p(3)` is seated, and `auto_sit_ping(p(3))` returns `Seated { table: 3 }`.
    - `p(4)`'s ping returns `Queued { place: 1, .. }`.
  - `auto_sit_skips_players_not_seen_recently`:
    - `p(3)` queued at t0 and never pinged; `p(4)` queued at t0 and pinged at t0 + 30 s.
    - At t0 + 31 s a seat opens and `tick` runs. `p(4)` is seated, and `p(3)` is skipped but still queued.
    - At t0 + 61 s, `p(3)` has been dropped and its ping returns `NotQueued`.
  - `auto_sit_failure_is_reported_once`: `p(3)` is queued, then spends their chips so the buy-in no longer fits. At the next seat opening, the first ping returns `Dropped { reason }` with a non-empty reason, and the second ping returns `NotQueued`.
  - `one_queue_per_player`: `auto_sit` at table 3 and then at a copy of 3 leaves `p(3)` only in the copy's queue.
  - `manual_seat_click_beats_the_queue_and_leaves_it`:
    - With a free seat and `p(3)` queued, `p(4)` calls `join_table` and gets the seat.
    - With `p(3)` queued at table X, `p(3)` calls `join_table` on X and is removed from X's queue.
  - `join_game_skips_tables_with_a_queue`: a copy has a free seat and a queue. `join_game` returns a different table, or a new copy.
  - `auto_sit_validates_buy_in`: below the minimum, above the maximum, more than the balance, and already seated at this table each return `Err`.
  - `table_view_shows_queue_state`: `table_view(3, Some(p(4)))` has `my_auto_sit == Some(place 2)`, `auto_sit_waiting == 2`.
  - `upgrade_round_trip_keeps_queues` (Review Focus 5): follow the existing upgrade round-trip test (search `save()`/`restore` in the tests). With a queue entry and an outcome present, `save` then `restore` keeps both.
- [ ] **Step 2:** Run `cargo test`. Expected: FAIL to compile.
- [ ] **Step 3:** Implement. Add `AUTO_SIT_EXPIRE_NS`, `AUTO_SIT_LIVE_NS` and `AUTO_SIT_MAX_QUEUE` next to the other constants.
- [ ] **Step 4:** Run `UPDATE_CANDID=1 cargo test`, then `cargo test`, then `npm run build`. Expected: PASS.
- [ ] **Step 5:** Commit with message `Add the auto-sit queue`.

---

### Task 4: Frontend — grouped lobby (layout B) and joining a game

**Files:**
- Create: `src/poker_frontend/src/lobby.ts` and `src/poker_frontend/src/lobby.test.ts`
- Create: `src/poker_frontend/src/components/JoinGame.tsx`
- Modify: `src/poker_frontend/src/components/Lobby.tsx` (table rows ~line 70-98) and `styles.css`

**Interfaces:**
- **Consumes:** `TableSummary.game`, `number` and `game_at_cap` (Task 1), and `backend.join_game` (Task 2).
- **Produces in `lobby.ts`:**
  - `export interface GameRow { game: bigint; first: TableSummary; tables: TableSummary[]; players: number; open: number; room: boolean; full: boolean }`.
    - `first` is the original, or the lowest-numbered table present.
    - `tables` is sorted by `number`.
    - `open` is the sum of `max_seats - players`.
    - `room` is `room_owner.length > 0`.
    - `full` is `open === 0 && first.game_at_cap`.
  - `export function groupTables(rows: TableSummary[]): GameRow[]`. It groups by `game`, while rooms always stay as their own row. Rows keep the order in which their game first appears in the input.
- **Produces `JoinGame`:** `export function JoinGame({ row, onClose }: { row: GameRow; onClose: () => void })`. It's a buy-in modal modelled on `BuyIn` in `Table.tsx` (slider from `min_buy_in` to `min(max_buy_in, balance)`, with the existing balance logic).
  - On submit it runs `run(() => unwrap(backend.join_game(row.game, value)), 'Good luck!')`, then calls the tables context's open/focus for the returned id the same way a lobby row click does today (`go({ name: 'table', id })`), then `refreshProfile()`.

- [ ] **Step 1: Write the `lobby.test.ts` tests:**
  - Copies with `game = 3` and numbers 2 and 3 fold into one row with `tables.length === 3`, sorted 1, 2, 3.
  - `players` and `open` add up across the tables.
  - Two rooms never merge, even with equal names.
  - Row order follows first appearance.
  - `full` is true only when no seat is open and `game_at_cap` is set.
  - A copy with no original in the list (a defensive case) still forms a row.
- [ ] **Step 2:** Run `npm test`. Expected: FAIL, because the module is missing.
- [ ] **Step 3:** Implement `lobby.ts`. Then render layout B in `Lobby.tsx` from `groupTables(shown)`:
  - **Game row:**
    - the name and HU/🔒 tags,
    - an "N tables" tag (`.tag.copies`) when there are 2 or more,
    - the stakes and buy-in range,
    - "P playing · S seats open",
    - **Join** (opens `JoinGame`), or **Full** (disabled) when `row.full`.
  - **Caret and sub-rows:** a caret toggles the sub-rows. The fold state is in localStorage key `bp.lobby.open` as a game-id list, wrapped in try/catch, and defaults to open when `row.full`. Each sub-row shows "Table n", the seat bar with n/max, and a **Watch** button that opens that table id.
  - **Single-table games:** one line with **Join** and **Watch**.
  - **Rooms:** unchanged. The row click and Join behave as today.
  - **Styles:** copy the mockup's look for `.tag.copies` and the sub-rows (`.superpowers/brainstorm/1572-1792440247/content/lobby-copies.html`).
- [ ] **Step 4:** Run `npm test` and `npm run build`. Expected: PASS.
- [ ] **Step 5:** Commit with message `Group table copies in the lobby and join a game from its row`.

---

### Task 5: Frontend — watch bar, auto-sit toggle and closed tables

**Files:**
- Create: `src/poker_frontend/src/components/WatchBar.tsx`
- Modify: `src/poker_frontend/src/components/Table.tsx`:
  - render `WatchBar` in the action-bar area when not seated (~line 560),
  - the `!view` branch (~line 254),
  - the table header title (show "Table n" when `number > 1`).
- Modify: `styles.css`

**Interfaces:**
- **Consumes:** `TableView.my_auto_sit`, `auto_sit_waiting` and `number`, and `backend.auto_sit` and `backend.auto_sit_ping` (Task 3).
- **Produces:** `export function WatchBar({ view }: { view: TableView })`. Render it only when the viewer is signed in, `view.my_seat` is empty, `!view.restricted` and `!view.tournament.length`.
  - **Text:** "Watching <name> (Table n)", then either "S seats open" or "all seats taken".
  - **Toggle:**
    - Turning it on opens a small buy-in picker with the same slider rules as `JoinGame`, defaulting to the last buy-in stored in localStorage `bp.autosit.buyin`. It then calls `auto_sit(id, [value])`.
    - Turning it off calls `auto_sit(id, [])`.
  - **While queued** (`view.my_auto_sit` present): it shows "You're Nth in line · W waiting" and pings `auto_sit_ping(id)` every `AUTO_SIT_PING_SECS = 20` s through `setInterval`, cleared on unmount or when the queue entry disappears.
    - A `Seated` ping shows `notify("You're in! Seated at Table n")`.
    - A `Dropped` ping shows `notify(reason, 'err')`.
- **Closed tables:** in `Table.tsx`, a `hadView` ref records that a view loaded once. When `view` is null afterwards, show "This table closed." with a **Back to lobby** button that calls `closeTable(id)` and `go({ name: 'lobby' })`, instead of "Loading table…" (Review Focus 1).

- [ ] **Step 1:** Implement `WatchBar`, the closed-table state and the header number. These are UI-only, so the build checks them here and Task 6 verifies them live.
- [ ] **Step 2:** Run `npm test` and `npm run build`. Expected: PASS.
- [ ] **Step 3:** Commit with message `Add the watch bar with auto-sit, and show when a table closes`.

---

### Task 6: Local end-to-end check

**Files:**
- Create (scratchpad, not committed): `overflow-probe.mjs`, modelled on the scratchpad's `visuals-probe.mjs` (bot accounts, a Playwright watcher with `channel: 'chrome'`).

- [ ] **Step 1:** In WSL, run `icp deploy`, an upgrade that keeps state. Expected: success. That success also exercises the upgrade path with existing tables.
- [ ] **Step 2:** Run the probe:
  1. Bots A and B sit at Heads Up Hangout, which is now full.
  2. Bot C calls `join_game(3, …)`. Expected: a copy with `game == 3`.
  3. The watcher's lobby screenshot shows one Heads Up Hangout row with a "2 tables" tag, folding open to Table 1 and Table 2.
  4. Bot D `auto_sit`s on Table 1 and keeps pinging. Bot B leaves. Within 2 s, D is seated and D's ping returns `Seated`.
  5. Bot C leaves the copy. About 61 s later the copy is gone from `list_tables`, and a watcher that had it open shows "This table closed".
  6. Screenshot every step and look at each screenshot.
- [ ] **Step 3:** Fix anything found, then commit with message `Polish table overflow after the local check`.

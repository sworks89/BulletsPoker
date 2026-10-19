# Table overflow: automatic copies, grouped lobby and auto-sit

Date: 2026-10-20
Status: draft, awaiting approval

## Why

Every cash table is a single fixed table. Once its seats are taken, everyone
else can only watch, pick another stake, or host a room. Heads-up tables fill
with just two players. With many players online, most of them would be turned
away from the game they want. The owner wants a full game to open a new table
automatically, while still letting people watch and take a seat when one opens.

## Goals

1. **Join a game, not a table.** Joining a game seats you at an open seat
   anywhere in that game. If every table of the game is full, a new copy opens
   and you're seated there.
2. **Fill before opening.** Join picks the copy with the most players that still
   has a free seat, so a second player joins the first player's half-empty copy
   instead of opening another empty one.
3. **Watch any table and sit two ways.**
   - Manual: click an empty seat when one opens, as today.
   - Auto: an "auto-sit when a seat opens" toggle puts you in a queue, and the
     first player in line is seated automatically.
4. **Grouped lobby (layout B, the owner's pick).** One row per game shows the
   combined player count and open seats, plus one Join button. A game with
   2 or more tables gets a "N tables" tag and folds open to list each copy
   ("Table 1", "Table 2"…), each with its own Watch button.
5. **Copies clean themselves up.** An empty copy closes after about a minute.
   The original table never closes.

## Non-goals

- **Tournaments.** The owner deferred these. Sit-and-gos already reopen a fresh
  instance when one fills, and scheduled events keep their caps.
- **Player-hosted rooms.** They are never copied, and they keep the existing
  30-minute idle close (`ROOM_IDLE_CLOSE_NS`). A room's Join button stays as
  it is today.
- **Game rules and money.** No changes to rake, the jackpot, buy-in limits,
  stakes, the clock or the time bank.

## Anti-goals (failure even if it "works")

- **Charging someone who isn't there.** A player seated from the queue must
  still be watching. A queued player who closed the tab must never be seated
  and charged a buy-in.
- **Losing or leaking chips:**
  - when a copy closes,
  - when a join fails partway (debit without a seat),
  - when auto-sit fails because the player can't afford the buy-in.
- **Dodging the buy-back rule by switching copies.** "Left less than 30 minutes
  ago, buy back in for at least what you left with" must apply across the
  whole game, not per table id.
- **Unbounded growth.** A bot wave must not be able to create tables without
  limit.
- **Breaking existing behavior:**
  - existing direct `join_table(id, seat, …)` calls,
  - players already seated at a table,
  - deep links to a table id.

## Constraints

- **Code:** backend in `src/poker_backend` (Rust canister; state in
  `state.rs`/`table.rs`), frontend in `src/poker_frontend` (React,
  `Lobby.tsx`, `Table.tsx`). Candid changes must be additive.
- **Upgrades:** stable-memory upgrades must stay compatible. New persisted
  fields are optional or defaulted.
- **Caps (my call):** at most **50 copies per game** and **300 copies in
  total**, so the once-a-second `tick` over all tables stays cheap. When a game
  is at its cap and full, Join fails with a clear message suggesting watching
  with auto-sit.
- **Rate limits:** existing limits and `inspect_message` rules apply to the new
  endpoints too. The new updates count against the per-player budget.
- **Seats per player:** `MAX_CASH_SEATS` (4) still caps how many cash tables a
  player sits at.

## Behaviour

### Games and copies

- **Which tables form a game.** Every non-room, non-tournament cash table is
  its own game: the 10 default tables, plus any tables an admin creates.
  - The table a game started from is its **original**.
  - A **copy** is a new table with an identical `TableConfig` (name, stakes,
    seats, currency, rake, jackpot settings, clock).
  - Each copy records the original's id as its `game` id. The original's
    `game` id is its own id.
- **Numbering.** Copies are numbered for display. The original is Table 1, and
  each copy takes the lowest number not currently in use. A copy is never
  renamed.
- **Closing.** A copy that has had 0 players for **60 seconds** is removed with
  the existing `remove_table` cleanup. A copy is never removed while anyone is
  seated, which includes players sitting out. The original is never removed.
  - Track `empty_since` per copy: set it when the copy becomes empty, clear it
    when someone sits.
  - The check runs in `tick`.
- **Players watching a closed copy.** In the frontend, the table view stops
  resolving. The player sees "This table closed" and goes back to the lobby or
  closes the tab.

### Joining a game

New update `join_game(game: u64, buy_in: u64) -> R<u64>`. It returns the table
id where the player was seated.

1. Validate as `join_table` does: pauses, bans, `MAX_CASH_SEATS`, funds,
   buy-in limits and the buy-back minimum.
   - The buy-back minimum now looks at cashouts from **any table in the game**
     within the last 30 minutes.
   - A player already seated at any table of this game gets "You're already
     playing this game".
2. Pick a table:
   - **Preference:** the table in the game with the most players that still has
     a free seat.
   - **Tie-break:** the lowest table number.
   - **Queue rule:** a table whose auto-sit queue isn't empty counts as having
     no free seat. Its seats belong to the queue.
3. If none qualifies and the game is under both caps, create a copy and use it.
4. Seat the player in the first free seat (lowest index), debit as
   `join_table` does, and post "X sat down".
5. Return the table id. The frontend opens that table.

`join_table(id, seat, …)` keeps working for a specific seat, as used by the
in-table seat click. It also applies the game-wide buy-back minimum.

### Auto-sit queue

- **Per table.** Each table, original or copy, has a FIFO queue of
  `{player, buy_in, last_seen}`.
- **Joining the queue.**
  - New update `auto_sit(table: u64, buy_in: Option<u64>) -> R<u32>` joins with
    that buy-in, or leaves the queue when given `None`.
  - It returns the player's 1-based place, or 0 after leaving.
  - It validates the buy-in against the table's limits and the buy-back minimum
    at join time.
  - A player can be in at most **one** queue at a time; joining a second queue
    moves them.
  - Each queue holds at most 100 players.
- **Staying in line.** While the table is open, the frontend calls
  `auto_sit_ping(table)` every **20 s**. It updates `last_seen` and returns the
  current place. An entry not seen for **60 s** is dropped. Closing the tab,
  leaving the table view, or the copy closing therefore drops the player.
- **Seating.** In `tick`, while a table has a free seat and a non-empty queue,
  take the first live entry and try to seat them as `join_table` would.
  - **Success:** remove the entry, post "X sat down (auto-sit)", and add a
    one-shot notice to the player's table view so the UI can tell them.
  - **Failure** (insufficient funds, now at `MAX_CASH_SEATS`, banned, buy-back
    minimum not met): drop the entry and record the reason so the player's next
    ping returns it. The frontend shows it and turns the toggle off.
  - **Seat choice:** auto-sit takes the lowest free seat.
- **Viewer state.** `TableView` gains the viewer's queue state, so the toggle
  survives a reload: `my_auto_sit: Option<{place: u32, buy_in: u64}>`, plus
  `auto_sit_waiting: u32`, the queue length shown to everyone.

### Lobby (layout B)

`list_tables` stays as it is. Each `TableSummary` gains `game: u64` (its own id
for originals and rooms) and `number: u8` (1 for originals and rooms). The
frontend groups rows by `game`:

- **The row:**
  - the game's name, stakes and buy-in range,
  - "**P** playing · **S** seats open", totalled across its tables,
  - the "N tables" tag when N ≥ 2,
  - a caret to fold the copies open or closed.
- **Join** calls `join_game` through the existing buy-in dialog, then opens the
  returned table.
  - When every table is full and the game is at its cap, the button reads
    "Full", and the row's sub-list is open so the player can watch.
- **Folded open,** each table has a line: "Table n", the seat bar and n/max,
  and a Watch button that opens that table.
- **Single-table games** behave as Watch plus Join on one line. A watcher can
  still click an empty seat in the table view.
- **Hosted rooms** show exactly as today, with their own Join and password
  flow.
- The fold state is per viewer in localStorage. The default is folded,
  except when the game is full.

### Table view

When the viewer isn't seated:

- **Watch bar:**
  - "Watching **name** (Table n)",
  - the open seats, or "all seats taken",
  - the **Auto-sit when a seat opens** toggle with its buy-in.
- **The toggle.** Turning it on asks for the buy-in in the existing buy-in
  dialog. The default is the last-used buy-in.
- **While queued,** the bar shows "You're Nth in line" and the queue length.
- **Clicking an empty seat** still works. If the player is queued here, sitting
  manually removes them from the queue.
- **When auto-sit seats the player,** a toast says "You're in! Seated at Table
  n", and the view switches to seated.
- **When auto-sit fails,** a toast gives the reason and the toggle turns off.

## Left to the builder

- Data layout:
  - where queues and `empty_since` live (a per-table field or `State` maps),
  - how `game` is stored, for example a field in `Table`, or a `copies` map
    from game to set.
- Exact copy-name strings beyond "Table n".
- Lobby styling, within the mockup (`.superpowers/brainstorm/*/content/lobby-copies.html`).
- Whether `tick` runs the queue seating and copy cleanup together or separately.
- Tests:
  - Backend unit tests cover:
    - the join choice (fullest with space, then a new copy, caps),
    - the game-wide buy-back,
    - queue order, expiry, failure reasons and moving between queues,
    - copy closing after 60 s and never with players seated,
    - originals never closing,
    - queued tables not being taken by `join_game`.
  - Frontend unit tests cover the lobby grouping function.
  - A local end-to-end run with bots checks: a third player on Heads Up
    Hangout gets a copy, auto-sit takes a freed seat, and an empty copy
    disappears.

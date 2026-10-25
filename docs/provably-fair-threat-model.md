# Fair-shuffle threat model

This document covers the BP-FAIR-v1 verifiable shuffle and hand history. It
sets out what they prevent, what they only make detectable, and what remains a
trust assumption. The protocol itself is specified in
[`provably-fair.md`](provably-fair.md). Every mitigation below names the code
that implements it. Where the code does not do something, this document says
so.

Audience: developers and auditors. Paths are relative to the repository root.
Backend files are in `src/poker_backend/src/`, frontend files in
`src/poker_frontend/src/`.

## Summary

After a hand, a player who was dealt into it can check four things:

- the revealed seed opens the commitment the canister recorded for that hand;
- every card they can see is the card BP-FAIR-v1 derives from that seed: their
  own hole cards, shown hands and the board;
- the hand log is internally consistent (see threat 5 for exactly which rules
  are checked);
- the pot, rake, jackpot drop and payouts follow from the log's commitments and
  the rebuilt cards. The verifier recomputes these independently.

This is not mental poker, and players must trust the canister. The canister
generates the seed and stores both the seed and the shuffled deck in plaintext
in its own state. It deals from them. A verifier must still trust four things:

- **The canister's code.** The canister picks the seed. A controller who
  installs different code can choose, read or replace seeds before the
  commitment is recorded, and can rewrite stored records afterwards.
- **The subnet.** Every replica of the canister's subnet holds the seed, the
  deck and every hole card in canister memory while the hand is played.
- **`raw_rand`.** The seed is the 32-byte output of the management canister's
  `raw_rand`. **That output carries no public proof of its origin.** ICP's
  threshold randomness is a property of the network, not something a player
  can check. The verifier can only check the seed against the commitment. It
  cannot check where the seed came from.
- **The replica that answers `get_hand`.** `get_hand` is a query (`lib.rs`,
  `get_hand`), so a single replica answers it and the reply is not certified.
- **The verifier code and the environment running it.** The in-app verifier
  (`fair/verify.ts` and the Verify panel) is served by the same operator's
  frontend canister. A controller, a compromised frontend or a compromised
  browser could show VERIFIED whatever the data says.
  - Players who want high assurance should export the evidence file ("Export
    evidence") and check it with a verifier they obtained independently.
  - They can build the open-source verifier (`src/poker_frontend/src/fair/`)
    themselves from a reviewed commit, or write their own from
    [`provably-fair.md`](provably-fair.md) and its test vectors.

Verification happens after the hand. It cannot show that nobody saw the cards
during play. The in-app verifier states both limits in every report: the
`trusted` list in `fair/verify.ts` (`verifyHand`), and `DISCLAIMER` in
`replay.ts`, shown on the Verify panel.

## How a hand is committed and revealed

1. **Ready tables get a seed request.** Every second, `tick` (`lib.rs`) asks
   `State::tick` (`state.rs`) which tables are ready for a new hand. For each
   one, `claim_seed_requests` takes a per-table slot in `PENDING_SEEDS` and
   spawns `seed_and_deal` as a detached task.
2. **One `raw_rand` per request.** `seed_and_deal` awaits one `raw_rand`
   (`ledger::raw_rand`, a bounded-wait call). An error, or a reply that is not
   exactly 32 bytes, ends the task without a hand.
3. **The table is re-checked after the await.** `State::deal_with_seed`
   confirms three things:
   - the table still exists;
   - its `hand_no` has not moved since the request;
   - it still wants a hand (`wants_new_hand`).

   Only then does it call `Table::start_hand` (`table.rs`).
4. **The commitment is logged before any card is dealt.** `start_hand` refuses:
   - a commitment for the wrong table or hand;
   - a commitment that the seed does not open (`fair::verify_commitment`);
   - a second commitment for the same hand;
   - the seed the table used for its previous hand (`last_seed_hash`).

   It then computes the deck and writes the `ShuffleCommitted` event as log
   entry 0. After that it copies the table's rake and jackpot rates into
   `FairHand.rates`, and deals. The seat order of the hole cards is recorded in
   `FairHand.dealt_order`.
5. **The hand is recorded with its evidence.** When the hand ends,
   `record_hand` (`state.rs`) stores a `HandRecord` in stable memory
   (`stores.hands`). The record carries `FairEvidence`, built by
   `FairHand::evidence`, which includes the seed. `publish_hand` appends the
   `ShuffleRevealed` and `HandVerificationAvailable` log entries.
6. **The seed is withheld for 24 hours.** `State::hand` (behind `get_hand`)
   returns the record only to players dealt into the hand. It blanks
   `revealed_seed` until `now >= reveal_at`, where
   `reveal_at = settled_at + 24 h` (`REVEAL_DELAY_NS` in `table.rs`).

**Players see the commitment during the hand.** While a hand is being played
(PreFlop to River), `TableView.commitment` carries that hand's commitment, for
every viewer except outsiders of a private room; it is `None` between hands.
The app stores what it saw while the board was still empty (pre-flop), keyed
by canister, table and hand number, in the browser's localStorage
(`fair/seen.ts`, last 500 hands, first value kept). A commitment first seen
after the flop is not stored, since it says nothing about when the deck was
fixed. The
verifier compares the published commitment with that copy
(`verifyHand`, `observedCommitment`): a different value fails the commitment
check, and the panel shows "Commitment seen at the table: yes/no".

- With a copy, the commitment is shown to have been fixed before the flop was
  dealt. It is not shown to precede the blinds or pre-flop actions that happened
  before the first poll landed. The hole cards are dealt in the same step that
  records the commitment, so the first view a player can get already has them
  out; a copy does not prove timing relative to the hole-card deal beyond the
  canister's log.
- Without a copy (another device, cleared storage, a hand the player never
  watched), "commitment before the deal" means the canister's own log order,
  which the verifier checks (`ShuffleCommitted` must be entry 0). It is not an
  externally timestamped event. The report lists it as taken on trust
  ("commitment timing") and warns.

**Players can keep their own copy of the commitment.** Besides the automatic
copy, the Verify panel shows the commitment with a Copy button before the
reveal (`components/HandVerification.tsx`), and "Export evidence" works before
the reveal too: the file then holds the record, the player's own cards and the
commitment without the seed, and the verifier reports it as pending
(INCOMPLETE).

## Threats

Each threat gets one of three classes, given the trust assumptions above:

- **Mitigated:** the code prevents it.
- **Detectable:** a player running the verifier on the record finds it.
- **Residual:** neither. It is a trust assumption.

### 1. An operator or privileged method replacing a committed shuffle

**Detectable**, with a residual part.

- **No endpoint touches the shuffle.** No admin, moderator or player endpoint
  sets or changes a seed, a deck or a commitment.
  - The admin and moderator endpoints in `lib.rs` (`admin_*`, `mod_*`) affect
    tables in four ways: creating them (`admin_create_table`), creating
    tournaments (`admin_add_tournament`), pausing dealing (`admin_set_pause`),
    and moving a banned player out (`mod_ban`, or `mod_resolve_report` with a
    ban, see threat 7).
  - Pausing dealing stops new hands but lets hands in progress finish
    (`State::tick`, `wants_new_hand`).
- **One stored-record edit exists, and it is narrow.** `show_cards`
  (`State::show_cards`) lets a player who won without a showdown show their
  cards before the next hand. It makes two changes to their latest record:
  - sets `shown` on that player;
  - inserts their "Shows" log line before the reveal notices.

  It does not touch the evidence, the board, the commitments or the payouts.
  The verifier's log reader ignores "Shows" lines (`readLog`, `fair/settle.ts`).
- **A commitment cannot be replaced.** `Table::start_hand` refuses a second
  commitment for a hand that already has one, and the deck is derived from the
  committed seed alone (`fair::shuffle_deck`). Tests:
  `cannot_overwrite_a_commitment` and `commitment_is_logged_before_any_card`
  (`table.rs`).
- **A swapped deck is caught.** If the dealt cards differ from the committed
  seed's deck, the verifier's dealing check fails (`verifyHand`, "dealing").
- **Residual:** a controller can upgrade the canister to code that picks the
  seed before recording the commitment, or that rewrites records afterwards.
  See threats 6 and 8.

### 2. Seeing the deck or seed early

**Residual.**

- **The secrets are plaintext state.** While a hand is played, the seed
  (`FairHand.seed`), the shuffled deck (`Table.deck`) and all hole cards are
  plaintext canister state. Every replica in the subnet can see them, and so
  can anyone who can read the canister's memory.
- **Hole cards travel in query replies.** They are returned to their owner by
  query calls (`get_table`, `get_tables`), which a single replica answers.
- **The canister's own interface does not expose them.** This part the code
  does prevent:
  - **A candid guard keeps the seed types out of the interface.**
    `candid_tests` in `lib.rs` is a `cargo test`, run in CI. It fails if the
    generated `.did` contains `FairHand`, `VoidedHand`, `last_seed_hash`,
    `fair_voided` or the word "verified". It also fails if there is any field
    whose name ends in `seed`, other than one `FairEvidence.revealed_seed`.
    This is a deny-list of names: a secret field whose name does not contain
    one of them (for example a deck field) would not be caught.
  - **A test checks the table views.** `seed_never_appears_in_table_view`
    (`state.rs`) plays a hand through. At each step it encodes `TableView` and
    `list_tables` for several viewers and checks that the seed bytes never
    appear. While a hand is being bet, it also checks that the undealt deck and
    the next cards never appear.
  - **The seed is withheld until the reveal.** `State::hand` blanks
    `revealed_seed` until `reveal_at` (test
    `seed_is_hidden_until_24h_after_settlement`).

### 3. A malicious participant manipulating events

**Mitigated**, except that exported evidence files are unsigned (below).

- **Players can only act through validated update methods.** These are `act`,
  `sit_out`, `leave_table`, `show_cards` and so on, and the engine checks each
  call. Players cannot write protocol log entries, evidence or records
  directly.
- **Players cannot influence the seed.** It is requested after the table is
  ready, and nobody outside the canister knows it until the reveal.
  - Joining, leaving or sitting out while the request is in flight can change
    who is dealt in, or cause the seed to be dropped (step 3 above).
  - It cannot change the deck for a given seed and hand.
  - A dropped seed was never committed or shown, so dropping it gives nobody a
    choice of decks.
- **Evidence cannot be moved to another hand.** The commitment binds the
  canister, table and hand number (`fair::commitment`), and the evidence names
  all three. The verifier checks each against the record and against the
  app's canister id (`verifyHand`, "commitment").
- **Not covered: exported evidence files are unsigned.** Offline verification
  (`components/VerifyEvidence.tsx`) takes the canister id from the file itself.
  `fromEvidenceJson` (`fair/evidence.ts`) only checks it against the file's own
  `evidence.canister` field.
  - So anyone can fabricate a file that verifies offline, or one that fails.
  - The offline screen shows the canister the file names, warns when it is not
    this app's game canister, and says: "This file is not signed: a passing
    check shows the file is consistent, not that the game produced it."
  - An evidence file proves only that it is internally consistent. It does not
    prove that this canister produced it.

### 4. Exploiting async `raw_rand` calls

**Mitigated.**

- **One request per table at a time.** `SeedRequest::claim` takes the table's
  slot in `PENDING_SEEDS` and releases it on drop. That includes the cleanup
  of a trapped call. Code: `lib.rs`; test:
  `a_table_has_one_seed_request_in_flight`.
- **A late seed is re-checked and dropped.** The request records the table's
  `hand_no`. After the await, `deal_with_seed` refuses in three cases: the
  table is gone, its `hand_no` has moved, or it is no longer ready. A refused
  seed was never committed and is dropped
  (`late_rand_for_a_changed_table_is_dropped`, `state.rs`).
- **There is no fallback.** A failed or malformed `raw_rand` reply means no
  hand (`seed_and_deal`). The seed never comes from any other source.
- **Duplicates and reuse are refused.** `start_hand` refuses a duplicate
  commitment for the same hand, and a reused seed
  (`same_seed_twice_is_refused`). The reuse check compares only against the
  table's previous seed (`last_seed_hash`). With 32-byte random seeds this
  guards against bugs; it is not a cryptographic need.
- **A slow reply does not block the clock.** `seed_and_deal` runs as a detached
  task (`spawn_migratory`), so a slow `raw_rand` does not hold the one-second
  tick open.

**`refresh_entropy` is separate.** It (`lib.rs`) also calls `raw_rand`, every
5 minutes, but its output only feeds `State.entropy`, never a shuffle seed.
`State.entropy` is used for tournament seating (`tournament.rs`) and the
rewards wheel (`rewards.rs`).

**A revealed seed does not expose that entropy.** Each hand's seed is mixed
into it through a domain-separated hash (`"BulletsPoker/entropy/v1"` in
`seed_and_deal`). `mix_entropy` hashes the previous secret state together with
the new input. A revealed seed therefore does not reveal the entropy.

### 5. Replay tampering or selective omission of events

**Detectable** for the cards and for a record that contradicts itself.
**Residual** for a consistent rewrite of betting actions, and for whole hands
that are never recorded.

The verifier (`fair/verify.ts`, `verifyHand`) reports **FAILED** for:

- **A misplaced or wrong commitment event.** This covers a duplicated
  `ShuffleCommitted` event, one that is not log entry 0, and one whose
  commitment differs from the evidence.
- **A seed that does not open the commitment.** It also fails a revealed seed
  that is not 32 bytes long.
- **Mismatched cards.** That is, any visible card (the board, the viewer's own
  hole cards, shown hands) that differs from the rebuilt deck. It also fails a
  `dealt_order` that is not the dealt seats twice, starting left of the button,
  and a wrong burn count.
- **A log that breaks the checked rules.** This covers a log that goes back a
  street; a seat that acts after folding or going all-in; a line by a seat that
  was not dealt in; and streets that don't match the board.
- **Totals that disagree with the log.** This means per-seat commitments, the
  pot or payouts that disagree with the log.
- **A wrong settlement.** The verifier recomputes it independently
  (`fair/settle.ts`, `fair/evaluate.ts`): side pots, odd chips, and the rake
  and jackpot drop from the rates recorded at the deal.
- **An unreturned uncalled bet** (`unreturnedBet`, `fair/settle.ts`).

**Missing data gives INCOMPLETE, not FAILED.** The cases are:

- no `ShuffleCommitted` entry;
- an empty `dealt_order`;
- a seed that is still absent after `reveal_at`.

Missing data cannot hide a proven failure: any failed check makes the overall
result FAILED. The one exception is a hand whose seed is not yet due. It is
reported INCOMPLETE ("pending reveal") until `reveal_at`, with any failures
still listed.

The verifier cannot detect:

- **Rule-breaking betting that the record agrees with.** The replay check does
  not verify turn order, bet and raise sizes, blind amounts or the
  minimum-raise rule. The canister writes both the log and the record. Only
  the cards are bound to the commitment, so a canister that rewrites the
  actions, the commitments and the payouts consistently would pass.
- **Missing hands.** It cannot see a hand the canister never recorded, or a
  record removed before the player looked at it. A player's history does not
  list every hand number at a table, so a gap is not evidence by itself.
- **Wrong fees on older evidence.** A wrong rake or jackpot drop on evidence
  that predates recorded fee rates goes undetected (see Known limitations).
- **Other players' unshown hole cards.** The record blanks them for the viewer
  (`State::hand`), so they are not compared, even though the seed rebuilds
  them.

### 6. A dealer generating many outcomes and choosing one

**Residual.**

The canister obtains the seed itself, and nothing outside it can tell how many
seeds it drew before committing. The commitment stops the seed from changing
after it is recorded. It does not stop a malicious build from calling
`raw_rand` repeatedly and committing the seed it prefers.

The current code takes the first `raw_rand` reply for each request.
`deal_with_seed` drops a seed only because of the table's state (gone, moved
on, not ready), never because of the deck the seed gives.

**Still residual, but code changes are now detectable.** The game canister is
built reproducibly and its module hash is published with each release
([`reproducible-builds.md`](reproducible-builds.md)). Anyone can rebuild a
release and compare it with the module hash the IC reports for the canister,
so a build that draws many seeds and keeps the best one cannot run unnoticed
unless it is a published release. A malicious release would itself be public
and open to review. Players still rely on the code being reviewed and honest:
a matching hash shows which code runs, not that the code is fair, and
a controller can upgrade to different code (threat 8).

### 7. Aborting after unfavourable cards

**Mitigated** for players. **Residual** for a malicious build.

**Sitting out or leaving never voids a hand.**

- **Leaving.** `leave_table` calls `Table::stand`. A player who leaves while
  still live in a hand stays seated and is marked `leaving`. Their remaining
  turns are checked or folded for them, and they are paid out when the hand
  ends (`take_leavers`). A player who has already folded leaves at once. They
  get back their remaining stack; the chips they already committed stay in
  the pot.
- **Sitting out.** On a sitting-out player's turn, `set_sitting_out` and
  `set_deadline` set their clock to now. The next tick's `handle_timeout` then
  checks or folds for them. `handle_timeout` does the same for any player
  whose clock and time bank run out.
- Tests: `leaver_whose_turn_it_is_is_checked_or_folded` and
  `sitting_out_player_is_checked_or_folded_at_once_for_the_rest_of_the_hand`
  (`table.rs`).
- **Bans.** A moderator ban follows the same paths (`State::ban`,
  `moderation.rs`): `leave_table` at cash tables, `set_sitting_out` at
  tournament tables.

**A committed hand is voided only when its table is removed**
(`void_unsettled_hand`). None of the code paths that remove a table is
expected to do so during a live hand. For tournament close this rests on an
invariant the code does not enforce (see the tournament item below):

- `close_room` refuses while anyone is seated (`player_count() > 0`). A
  leaving player still counts.
- `close_idle_rooms` requires `player_count() == 0`.
- `close_empty_copies` requires every seat to be empty and the table not to be
  betting.
- Tournament tables are broken up only when `between_hands()` holds
  (`tournament.rs`, table balancing). They are closed only after the
  tournament has finished.
  - This relies on a finished tournament having no live hand. Close does not
    check `between_hands()`.
  - If a hand were still live at close, `void_unsettled_hand` (tournament
    close in `tournament_tick`) would publish it as voided rather than lose
    it.
- No admin endpoint removes a table.

**A voided hand is still published.** Its players get it with its evidence and
the same 24-hour reveal delay, counted from publication
(`publish_voided_hands`). The Verify panel labels it voided.

**Residual:** a controller could install code that aborts or rewrites hands.
Stopping the canister also stops every hand in progress.

### 8. Upgrades or admin controls modifying history

**Residual.**

- **The running code edits records in only two ways.**
  - `show_cards` adds a voluntary show (threat 1).
  - Pruning removes the oldest records (`prune_hands`).

  No endpoint can rewrite a record's evidence, cards or payouts.
- **A controller can change anything.** A controller can install any code, and
  that code can read or rewrite stable memory, including stored records, seeds
  and evidence.
- **Upgrades are now visible, and mainnet upgrades go through releases.**
  The owner upgrades mainnet only with `scripts/deploy-release.sh`, which
  installs a published release after checking its hash. Anyone can compare the
  live module hash with the releases (`icp canister status <id> -n ic
  --public`, or the in-app badge, which also lists the controllers). An
  upgrade to code that is not a published release shows as "does not match any
  published release". This is a procedure and a check, not a technical block:
  a controller can still install anything, and nothing makes the site, the
  badge or the owner's process binding. The badge is shown by the operator's
  website, so rebuild yourself to check. The hash shows which code runs; it
  doesn't show that the code is honest, and it can't stop a later upgrade.
- **The only copy outside the canister is what a player kept.** That is the
  commitment the app stored when it was shown at the table, a commitment copied
  by hand, or an evidence file exported before or after the reveal
  (`toEvidenceJson`, `fair/evidence.ts`).
  - The export is the player's own copy. With it they can check that a later
    record still matches.
  - The export is not signed, so it proves nothing to a third party (see
    threat 3).

### 9. Seed leakage through logs, queries, backups, stable state or diagnostics

**Residual** for stable state, snapshots and backups. **Mitigated** for the
canister's own outputs.

**Residual: seeds are stored in plaintext.**

- The table's `FairHand` is part of the heap snapshot written to stable memory
  (`State::save`).
- Every stored `HandRecord` keeps its seed in `FairEvidence.revealed_seed`
  (`record_hand`), including before `reveal_at`.
- Anyone who can read the canister's memory, or a snapshot of it, can read
  seeds early. That includes the subnet's replicas, and a controller acting
  through an upgrade or a snapshot download.

**Mitigated in the interface.** See threat 2, and also:

- **Debug output is redacted.** The custom `Debug` implementations for
  `FairHand`, `FairEvidence`, `Table` and `Seat` (`table.rs`) never print the
  seed, the deck or the hole cards. The seed is printed as `<secret>`, the
  undealt deck as a card count, and `last_seed_hash` as `<hash>`.
- **Nothing is printed.** The canister code makes no `println!` or debug-print
  calls.
- **Errors do not carry the seed.** The rule that the seed never appears in
  logs or error messages is stated in `fair.rs`'s module documentation. It is a
  convention, not something a test enforces. The error strings of
  `deal_with_seed` and `start_hand` are fixed text.
- **Metrics are counts only.** `/metrics` serves counts and balances
  (`Metrics::from_state`, `metrics.rs`), and no game state.

### 10. Collusion

**Out of scope.** Collusion between players, such as sharing hole cards
outside the game, is not a property of the shuffle, and a verifier cannot see
it. It is handled by reporting and moderation (`report_player`,
`mod_resolve_report` and `mod_ban` in `lib.rs`; `moderation.rs`). There is no
automated collusion detection.

## Owner decisions

### D1: reveal to the hand's players after 24 hours

- **When the seed is returned.** `get_hand` returns the seed only from
  `reveal_at = settled_at + 24 h` (`REVEAL_DELAY_NS`, `State::hand`). Before
  then it returns the commitment, the parameters and `reveal_at`.
- **What the seed reveals.** It reveals every card dealt in the hand,
  including folded and mucked hole cards, burn cards and undealt cards.
  - The delay stops players from studying an opponent's folded cards during
    the same session.
  - The auto-muck setting's hint and the Verify panel both say this
    (`D1_LINE`, `replay.ts`; `components/Profile.tsx`).
- **Retention.** A record with evidence is kept until at least
  `reveal_at + 7 days` (`FAIR_RETENTION_NS`), even past the normal limit of
  20,000 records (`HISTORY_MAX` in `state.rs`, applied by `prune_hands`). The
  replay shows the window (`windowText`, `replay.ts`).
- **Hard cap.** At most 200,000 records are stored (`FAIR_HISTORY_HARD_MAX`).
  - Above the cap, the oldest records are removed even inside their window.
  - Removal runs as each new record is published, at most 4 per record
    (`PRUNE_PER_CALL`).
  - Once a record is gone, a player's exported evidence is the only copy.

### D2: one `raw_rand` per hand

- Each ready table gets its own request (`tick`, `seed_and_deal`), and
  requests for different tables run concurrently.
- The scheme is `icp-raw_rand/v1` (`fair::RANDOMNESS_SCHEME`).
- The cycle cost per hand has not been measured yet.

### D3: participants only

- `get_hand` returns a hand only to players dealt into it (`State::hand`).
  Spectators and other players get nothing.
- A player can share their exported evidence themselves.

## Known limitations

- **Older evidence has no fee rates.** Evidence for hands dealt before the
  rates were recorded has empty `rake_bps`, `rake_cap`, `jackpot_bps` and
  `jackpot_cap`.
  - For these hands the verifier takes the rake from the record, and infers
    the jackpot drop as pot − rake − payouts.
  - It checks only the no-flop rule and the global limits: rake ≤ 10%,
    drop ≤ 3%.
  - The report lists the rake and drop under "trusted".
- **Rates are recorded at the deal, but settlement reads the live config.**
  `Table::take_rake` uses `self.config`, not `fair.rates`.
  - Today nothing changes a table's rates during a hand.
  - A future mid-hand config change would make an honest hand fail
    verification.
- **Hands dealt by an older build during an upgrade.** A hand committed by a
  build from before `dealt_order` existed has an empty `dealt_order`.
  - The verifier checks the board but cannot place hole cards, so the result
    is INCOMPLETE.
  - Voided hands queued by such a build are published without their real
    seats, cards, board or log (`publish_voided_hands`). They also come out
    INCOMPLETE.
- **The voided-hand queue is capped.** At most 64 voided hands wait to be
  published (`FAIR_VOIDED_MAX`). Every tick drains the queue.
  - If more than 64 hands are voided between two ticks, the oldest are dropped
    unpublished.
  - Nothing counts or logs the drop.
- **Retention.** Records are removed after `reveal_at + 7 days`, or earlier
  past the 200,000-record hard cap.
  - The storage used at the cap has not been measured. A rough estimate is
    0.6–1 GB of stable memory.
- **History list length.** The recent-hands list keeps only the last 50 hand
  ids (`HISTORY_PER_USER`). Hands with evidence are also listed separately
  (`get_verifiable_hands`, History > "Ready to verify"): a per-player list in
  stable memory (`stores.fair_hands`) kept until `reveal_at + 7 days`, pruned
  on write and filtered on read.
  - That list holds at most 5,000 hands per player (`FAIR_HANDS_PER_USER`);
    past it the oldest go even inside their window. That is about one hand
    every 2.3 minutes for 8 days.
  - A record removed early by the 200,000-record hard cap drops out of the
    list.
- **`dealt_order` is checked against the record.** The verifier checks that
  `dealt_order` is the dealt seats twice, starting left of the button. It uses
  the record's own `button` and player list, which the canister supplies.
  - `dealt_order_skips_sitting_out_seats_and_follows_a_vacated_button`
    (`table.rs`) covers a sitting-out seat and a button seat vacated between
    hands. The engine moves the button to the next seat in the hand, so there
    is no dead button as such.
- **`reveal_at` is taken from the evidence.** The verifier warns when
  `reveal_at != settled_at + 24 h` but does not fail the hand: the reveal time
  does not change which cards were dealt. Evidence with a far-future
  `reveal_at` stays "pending reveal" (INCOMPLETE) until then. Pending is judged by the
  local clock, in the browser as well as offline.
- **Hidden hole cards are not compared.** The record blanks other players'
  unshown cards for each viewer. Only visible cards are checked against the
  rebuilt deck.
- **Exported evidence is unsigned**, and `get_hand` replies are not certified
  (threats 3 and 8).
- **The candid guard is a deny-list of names** (`candid_tests`, `lib.rs`;
  threat 2).
- **Note for verifier authors:** `LogEntry.amount` is chips only on game
  entries. On the protocol entry `ShuffleRevealed` it carries `reveal_at`.

## Future work

### Done: reproducible builds with published module hashes

The game canister has a deterministic build, a GitHub Release with its SHA-256
for each `v*` tag, a release-only deploy script for mainnet and an in-app
badge ([`reproducible-builds.md`](reproducible-builds.md)). For threats 1, 6,
7 and 8, this lets anyone check which published code the canister runs.

- **What it proves:** the module the IC runs is, byte for byte, the build of a
  published commit, which anyone can read and rebuild.
- **What it doesn't prove:** that the code is honest or fair (that takes
  review), that the frontend that shows the badge is honest, or that the code
  won't change. The canister is still upgradeable, and the controllers can
  install any code. The seed is still visible to the subnet's node operators
  (threat 2), `raw_rand` still has no proof of origin and the verifier still
  runs on the operator's frontend.

### Not built

These are separate projects. None of them is built.

1. **Decentralised control.** Hand control of the canister to an SNS, or
   remove its controllers (blackhole it), so that a single operator cannot
   install new code.
2. **Player-contributed entropy.** Players commit to their own random values
   before the deal and reveal them after. These values are mixed into the
   seed, so the canister alone cannot choose it (threat 6).
3. **vetKeys encrypted dealing.** Encrypt each hole card to its player with
   vetKeys, so that query replies no longer carry plaintext cards. The
   canister still holds the deck.
4. **Mental poker or a multi-party shuffle.** Players jointly shuffle and
   encrypt the deck, so that no single party, the canister included, knows the
   cards (threats 2 and 9).

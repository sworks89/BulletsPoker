# Provably Fair Shuffle and Hand Verification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every hand dealt after this ships has a committed, later-revealed seed. A versioned, deterministic shuffle lets anyone rebuild the deck. A TypeScript verifier, running independently of the canister, checks the commitment, the shuffle, the dealt cards, the replay and the settlement.

**Architecture:**
- The existing engine stays. Dealing still happens in `Table::start_hand` and `deal_board`, and settlement, side pots and hand evaluation are unchanged.
- **Backend:**
  - `fair.rs` holds the protocol: canonical encoding, commitment, HKDF/HMAC stream, Fisher–Yates v1 and test vectors.
  - `Table` holds one `FairHand` per hand.
  - `lib.rs tick` gets a fresh `raw_rand` per hand.
  - `record_hand` publishes the reveal into the `HandRecord`.
- **Frontend:**
  - `fair/`: the TypeScript port of the protocol, the verifier, and an independent hand evaluator and settlement check.
  - A Verify panel inside the existing `History.tsx` replay.
  - An offline "verify evidence JSON" view.
- **Docs:** the protocol specification and the threat model.

**Tech Stack:** Rust (ic-cdk, candid, sha2, plus `hmac` and `hkdf` from RustCrypto), React + TypeScript (WebCrypto `crypto.subtle`, no new npm dependency), vitest.

**Spec:** `docs/superpowers/specs/2026-10-23-provably-fair-requirements.md` (the feature brief as written). The binding deviations are in "Analysis of the brief" below.

## Analysis of the brief

### Where the brief doesn't match this codebase

1. **Rust backend.** The backend is a Rust canister (`src/poker_backend`), and the requirements are written for it. The cross-language test vectors are shared between Rust and TypeScript from one JSON file.
2. **The randomness already exists.** `lib.rs tick()` calls `raw_rand` once per tick for all tables that are ready.
   - Each table's seed is `SHA256(rand ‖ table_id ‖ hand_no)`.
   - `cards::shuffled_deck` already runs Fisher–Yates with rejection sampling over a SHA-256 counter stream.
   - So the brief's phases 2–3 are an upgrade to a versioned, documented, per-hand construction, not a first implementation.
3. **There's no hand ID when a hand starts.** `HandRecord.id` (`next_hand_id`) is only assigned in `record_hand`, at the end. The immutable ID that exists from the start is (canister principal, `table_id`, `hand_no`). That triple becomes the canonical protocol hand ID. `HandRecord.id` stays as the storage key.
4. **The replay "events" are `Table.log: Vec<LogEntry>`** (phase, seat, text, amount), copied into `HandRecord.log`.
   - `ShuffleCommitted`, `ShuffleRevealed` and `HandVerificationAvailable` become typed `LogEntry` items, using a new `kind: Option<…>` field so old records still decode.
   - The structured evidence goes in a new `HandRecord.fair: Option<FairEvidence>`.
5. **Commit before deal is atomic in a canister.**
   - Committing and dealing happen in the same update message, after `raw_rand` returns, so no observer can see a state between them.
   - The commitment is written to the hand's log as event 0, before any card is dealt. That ordering is what the verifier checks.
   - Splitting commit and deal across two messages would add a tick of delay with no security gain. The canister itself picks the seed, so the commitment can't stop a dishonest canister from choosing among seeds. See the threat model.
6. **Who can see hands.** `get_hand` today only returns a hand to its participants, and blanks out unshown hole cards. A revealed seed lets anyone who holds it rebuild every player's hole cards, including folded and mucked ones. **This is a privacy change the owner must accept** (see "Owner decisions").
7. **Hand terminal states that already exist:**
   - normal settlement (`HandOver`, then `record_hand`);
   - tables deleted while idle (`state.rs:1745`; `tournament.rs:893, 1070` merges tables between hands).
   - Timeouts auto-act, so hands don't hang today. The brief's "abandoned" and "cancelled" states map to a new `FairOutcome::Voided` for a committed hand whose deal or table never completed. The seed is revealed only after the hand can no longer be acted on.
8. **Settlement verification needs an independent implementation.** The verifier must not reuse the canister's result. It reimplements in TypeScript, from the existing rules in `table.rs`:
   - hand ranking, mirroring `cards.rs` evaluation;
   - main and side pots by commitment level;
   - rake (2.5% when a flop is dealt, with a cap);
   - the jackpot drop;
   - odd-chip handling.

   The tests are cross-checked against the Rust engine through shared JSON vectors.

### Owner decisions (all decided 2026-10-23)

- **D1 (privacy) — DECIDED: reveal to the hand's players after a 24-hour delay.**
  - The seed (and so every card dealt in the hand, including folded and mucked ones) is returned through `get_hand` to that hand's players only, and only once `REVEAL_DELAY_NS = 24 h` has passed since settlement. Before that, `get_hand` returns the commitment, the parameters and `reveal_at`, but no seed.
  - Why: players can't study opponents' folded cards during the same session. Later review stays possible, which is normal.
  - The rules page and the auto-muck setting get one line: "24 hours after a hand, the players in it can verify the shuffle, which reveals every card dealt in that hand."
  - **Retention:** hands with fair evidence are kept until at least `reveal_at + 7 days`, even if that exceeds `HISTORY_MAX`. Pruning skips them until then, and there's a separate hard cap so storage stays bounded. Once pruned, the exported evidence JSON is the permanent copy. The replay states the verification window.
- **D2 (randomness source) — DECIDED: (a) one `raw_rand` call per hand.** Calls for tables that are ready in the same tick run concurrently. Task 9 measures the cycle cost. If it turns out to matter, the fallback is (b): one call per tick with per-hand seeds derived by HKDF using the hand ID as context.
- **D3 (who can verify) — DECIDED by the owner (2026-10-23):** participants only. Evidence is served through the existing replay (`get_hand`, which already returns a hand only to its players). Spectators and other players get nothing. A participant can still export and share the evidence JSON themselves.

## Global Constraints

### Protocol `BP-FAIR-v1`

The exact bytes are specified in `docs/provably-fair.md`. Rust and TypeScript must agree on the shared vectors.

- `lp(x)` is the length prefix: `u32` big-endian `len(x)`, followed by `x`.
- **Hand ID:** `hand_id_bytes = lp(canister_principal_bytes) ‖ u64be(table_id) ‖ u64be(hand_no)`.
- **Parameters:** `params_bytes = u16be(1 /*shuffle_version*/) ‖ u8(52) ‖ lp("card=rank*4+suit;rank0=2..rank12=A;suit0=c,1=d,2=h,3=s") ‖ lp("deal=pop-from-end;burn-before-flop-turn-river")`.
- **Commitment:** `commitment = SHA256( lp("BulletsPoker/commit/v1") ‖ lp(seed[32]) ‖ lp(hand_id_bytes) ‖ lp(params_bytes) )`.
- **Shuffle key:** `key = HKDF-SHA256(ikm = seed, salt = "BulletsPoker/shuffle/v1", info = lp(hand_id_bytes) ‖ lp(params_bytes), L = 32)`.
- **Random stream:** block `i` is `HMAC-SHA256(key, u64be(i))` for `i = 0, 1, …`, consumed as successive 4-byte big-endian `u32` words.
- **Uniform draw:** `uniform(n)` for `n` in 2..=52 sets `limit = 2^32 − (2^32 mod n)` and draws until `v < limit`, then returns `v mod n`.
- **Shuffle:** start from `deck = [0, 1, …, 51]`. For `i` from 51 down to 1, take `j = uniform(i+1)` and swap `deck[i]` with `deck[j]`.
- **Dealing:** the existing engine's order. Cards are popped from the end, hole cards go one at a time starting left of the button for two rounds, and one card is burned before the flop, the turn and the river.
- **Legacy compatibility:**
  - Hands before this ship have `fair: None` and show as **Legacy / Not verifiable**.
  - The legacy `shuffled_deck` stays only as `shuffle_legacy` for tests. New hands always use v1.

### Rules for every task

- **Seed secrecy:**
  - The seed for the hand being played is stored only in `Table.fair` (canister state).
  - No query and no `TableView` field may return it before the reveal.
  - It must never be logged (no `ic_cdk::println!`) and never included in errors.
- **Persistence:** new persisted fields are `Option<T>`. Regenerate candid with `UPDATE_CANDID=1 cargo test`.
- **The engine is untouched:** no changes to hand rankings, betting, side pots, rake or jackpot rules. Existing tests must pass unchanged, except where a test hard-codes the legacy shuffle.
- **Commands:** run them inside WSL, as `MSYS_NO_PATHCONV=1 wsl -d Ubuntu-24.04 -- bash -lc 'cd /mnt/c/BulletsPoker && …'`.
  - Backend: `cargo test`, unfiltered at least once.
  - Frontend: `cd src/poker_frontend && npx vitest run && npm run build`. No new npm dependencies; use WebCrypto.
- **Honest claims:** no copy, doc or UI may say the hand was "trustless", "tamper-proof" or "provably random". Use the wording from the threat model.

## Review Focus

1. **No early leak:** no code path (query, `TableView`, `get_hand` before `reveal_at`, log, error, admin endpoint, a `Debug` print reaching a query) exposes the seed or the undealt deck before the reveal, which is 24 h after settlement.
2. **Exactly one seed per hand:** two ticks racing, a `raw_rand` reply arriving after the table changed, a table removed during the await, or a deal that fails after committing must never give a hand two seeds, reuse a seed, or deal without a commitment.
3. **Byte compatibility:** the encoding (lengths, endianness, field order) is identical in Rust and TypeScript and in the docs. Any mismatch makes every hand fail verification.
4. **Independent settlement:** the verifier's pot, side-pot, rake and jackpot arithmetic must match the engine for every shared vector: a split pot, three-way all-in side pots, everyone folding, no flop dealt (no rake), and the rake cap.
5. **Legacy behaviour:** old hands show "Legacy / Not verifiable", never "FAILED". No authoritative "verified" flag is stored on the backend.

---

### Task 1: Protocol module and shared test vectors (backend)

**Files:**
- Create:
  - `src/poker_backend/src/fair.rs`.
  - `docs/provably-fair.md`: the byte-level spec with worked examples.
  - `src/poker_frontend/src/fair/vectors.json`: shared vectors. They are generated by a Rust test and checked by both sides.
- Modify:
  - `src/poker_backend/Cargo.toml`: add `hmac = "0.12"` and `hkdf = "0.12"`, the RustCrypto crates that go with `sha2 0.10`.
  - `lib.rs`: add `mod fair;`.
  - `cards.rs`: rename `shuffled_deck` to `shuffle_legacy`.

**Interfaces:**
- **Produces (`fair.rs`):**
  - `pub const SHUFFLE_VERSION: u16 = 1;` and `pub const RANDOMNESS_SCHEME: &str = "icp-raw_rand/v1";`
  - `pub struct HandId { pub canister: Principal, pub table_id: u64, pub hand_no: u64 }`, with `fn bytes(&self) -> Vec<u8>`.
  - `pub fn params_bytes(version: u16) -> Vec<u8>`.
  - `pub fn commitment(seed: &[u8; 32], hand: &HandId, version: u16) -> [u8; 32]`.
  - `pub fn shuffle_deck(seed: &[u8; 32], hand: &HandId, version: u16) -> Result<Vec<Card>, String>`. It returns `Err` for an unknown version.
  - `pub fn verify_commitment(seed, hand, version, commitment) -> bool`.
- **Vectors:** `vectors.json` lists `{ seed_hex, canister_text, table_id, hand_no, version, commitment_hex, deck: [u8; 52] }` for at least 6 cases. A Rust test regenerates the file when `UPDATE_FAIR_VECTORS=1` is set, and otherwise asserts that the file matches.

- [ ] **Step 1: Write the failing tests** in `fair.rs`:
  - `deck_has_52_unique_cards` (for 1,000 seeds)
  - `same_inputs_same_deck`
  - `different_seed_or_hand_changes_deck`
  - `uniform_rejects_the_biased_zone`: stub the stream to return `u32::MAX` first, and assert it is rejected.
  - `commitment_changes_with_any_field`
  - `encoding_is_length_prefixed`: two different field splits must not produce the same bytes.
  - `unknown_version_is_an_error`
  - `vectors_file_matches`
- [ ] **Step 2: Run** `cargo test fair`. Expected: FAIL (compile).
- [ ] **Step 3: Implement.** Write `docs/provably-fair.md` from the code and the vectors.
- [ ] **Step 4: Run** `cargo test`. Expected: everything passes.
- [ ] **Step 5: Commit** "Add the BP-FAIR-v1 shuffle and commitment protocol".

### Task 2: TypeScript protocol port (frontend)

**Files:**
- Create: `src/poker_frontend/src/fair/protocol.ts` and `protocol.test.ts`.

**Interfaces:**
- **Produces:**
  - `handIdBytes(canister: string, tableId: bigint, handNo: bigint): Uint8Array`
  - `paramsBytes(version: number): Uint8Array`
  - `async commitment(seed: Uint8Array, hand…, version): Promise<Uint8Array>`
  - `async shuffleDeck(seed, hand…, version): Promise<number[]>`. It throws `UnsupportedVersion` for an unknown version.
  - All of it uses `crypto.subtle`: `importKey`, `sign` (HMAC), `digest`, and `deriveBits` (HKDF).

- [ ] **Step 1: Write the failing tests:** every entry in `vectors.json` reproduces the same commitment and deck; an unknown version throws.
- [ ] **Step 2: Run** `npx vitest run src/fair`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `npx vitest run && npm run build`. Expected: everything passes.
- [ ] **Step 5: Commit** "Port the fair shuffle protocol to TypeScript".

### Task 3: Hand lifecycle (backend)

**Files:**
- Modify:
  - `lib.rs` (`tick`, the `DEALING` guard).
  - `ledger.rs` (`raw_rand`).
  - `table.rs` (`start_hand`, the `Table` struct, `LogEntry`).
  - `state.rs` (`start_hand`, `tick`, table removal paths).
  - `tournament.rs` (table merges and removals).

**Interfaces:**
- **Produces (`table.rs`):**
  - `pub struct FairHand { hand: HandId, version: u16, seed: [u8; 32] /*secret until reveal*/, commitment: [u8; 32], committed_at: u64, outcome: FairOutcome }`
  - `pub enum FairOutcome { Committed, Dealt, Settled, Voided(String) }`
  - New `Table` fields:
    - `fair: Option<FairHand>`: the hand being played.
    - `last_seed_hash: Option<[u8; 32]>`: used to refuse reuse.
    - `fair_voided: Option<Vec<FairHand>>`: bounded, at most 4; voided hands waiting to be published.
  - `LogEntry.kind: Option<LogKind>`, where `LogKind` is `ShuffleCommitted { commitment } | ShuffleRevealed | HandVerificationAvailable`.
  - `Table::start_hand(&mut self, fair: FairHand, now) -> Result<(), String>`:
    1. Refuse if a `fair` is already committed for this `hand_no + 1`.
    2. Refuse if the seed's hash equals `last_seed_hash`.
    3. Write the `ShuffleCommitted` log entry first, then shuffle with `fair::shuffle_deck`, then deal as today.
    4. On any refusal after the commitment is built, nothing is stored.
  - Test-only `start_hand_with_seed(seed, now)` wraps this for the existing tests that use a fixed seed.
- **Async flow in `lib.rs tick`:**
  - For each table that is ready: `raw_rand()`, one call per hand (D2a). Calls can run concurrently, guarded by a per-table `pending` set rather than one global `DEALING` flag.
  - After the await, inside `with_mut`:
    - If the table still exists, is still ready, and `hand_no` hasn't moved, build the `FairHand` and call `start_hand`.
    - Otherwise drop the seed, which was never committed or seen.
  - A `raw_rand` error means no hand this tick. Never fall back.
- **Terminal states:**
  - A normal hand reaching `HandOver` is `Settled`, then `record_hand` publishes it (Task 4).
  - A committed hand on a table that is removed, or where `start_hand` failed after committing, is `Voided(reason)`. It is published to that hand's participants only once the table can no longer act on it.

- [ ] **Step 1: Write the failing tests:**
  - `commitment_is_logged_before_any_card`
  - `cannot_deal_without_a_commitment`
  - `cannot_overwrite_a_commitment`
  - `same_seed_twice_is_refused`
  - `late_rand_for_a_changed_table_is_dropped`
  - `seed_never_appears_in_table_view`: serialise a `TableView` to candid bytes and assert the seed bytes don't occur, across every phase.
  - `voided_hand_on_table_removal`
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.** Update the fixed-seed tests to use `start_hand_with_seed`.
- [ ] **Step 4: Run** `cargo test`. Expected: everything passes.
- [ ] **Step 5: Commit** "Commit to every hand's seed before dealing".

### Task 4: Publishing the evidence (backend)

**Files:**
- Modify:
  - `state.rs` (`HandRecord`, `record_hand`, `hand()`).
  - `lib.rs` (`get_hand`).
  - The `.did` and the bindings, regenerated.

**Interfaces:**
- **Produces:**
  - `HandRecord.fair: Option<FairEvidence>`, where `FairEvidence` is:
    - `version: u16`, `scheme: String`;
    - `canister: Principal`, `table_id: u64`, `hand_no: u64`;
    - `commitment: Vec<u8>`, `seed: Option<Vec<u8>>` (filled in only in the reply, once revealed), `params: Vec<u8>`;
    - `outcome: String` (`"settled"` or `"voided: …"`);
    - `dealt_order: Vec<u8>`: seat indices in the order hole cards were dealt;
    - `burns: u8`;
    - `settled_at: u64`, `reveal_at: u64` (`settled_at + REVEAL_DELAY_NS`, where `REVEAL_DELAY_NS = 24 * 3600 * 10^9`).
  - `record_hand` stores the seed in the stored record (it's private canister state, as the table's seed already is). It appends `ShuffleRevealed`, with `amount = reveal_at` to say when, and `HandVerificationAvailable` log entries.
  - `hand(caller, id, now)`:
    - Participants only, as today (D3).
    - The reply's `fair.seed` is `Some` only when `now >= reveal_at`. Before that it's `None`, with the commitment, parameters and `reveal_at` still included.
    - Hole-card blanking stays as it is. After the reveal, a participant can rebuild every card (D1).
  - Voided hands get a `HandRecord` with `fair` and an empty settlement, so they show in history as "Voided". The same 24-hour delay applies.
  - No backend field claims a hand is verified.
- **Retention (D1):**
  - `HISTORY_MAX` pruning must not delete a record with `fair` until `now >= reveal_at + 7 days`.
  - A separate hard cap, `FAIR_HISTORY_HARD_MAX`, with a stated value of, e.g., 200,000 records, keeps storage bounded. If even that is reached, the oldest records go, and the docs say so.
  - Pruning stays bounded per call.
  - The docs and the replay state the verification window. Evidence export (Task 6) is the durable copy.

- [ ] **Step 1: Write the failing tests:**
  - `seed_is_hidden_until_24h_after_settlement`: the seed is `None` at `reveal_at - 1` and `Some` at `reveal_at`.
  - `seed_is_published_only_after_settlement`
  - `fair_hands_survive_history_pruning_until_the_window_ends`
  - `published_evidence_rebuilds_the_dealt_cards`: in Rust, shuffle from the evidence and compare against the record's hole cards and board.
  - `legacy_records_decode_without_fair`
  - `voided_hand_evidence`
  - `non_participant_gets_no_evidence`
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.** Then run `UPDATE_CANDID=1 cargo test`.
- [ ] **Step 4: Run** `cargo test`. Expected: everything passes.
- [ ] **Step 5: Commit** "Publish shuffle evidence with each settled hand".

### Task 5: Independent verifier (frontend)

**Files:**
- Create:
  - `src/poker_frontend/src/fair/evaluate.ts`: an independent 7-card evaluator.
  - `fair/settle.ts`: pots, side pots, rake and jackpot drop.
  - `fair/verify.ts`, plus tests.
  - `src/poker_frontend/src/fair/settlement-vectors.json`, generated by a Rust test from real engine hands, like the Task 1 vectors.

**Interfaces:**
- **Produces:** `async verifyHand(record: HandRecordLike, evidence: FairEvidenceLike | null, canisterId: string): Promise<VerificationReport>`.
- **`VerificationReport`:**
  - `overall`: `'VERIFIED' | 'FAILED' | 'INCOMPLETE' | 'LEGACY' | 'UNSUPPORTED'`
  - `version`
  - `checks: { commitment, shuffle, dealing, replay, settlement }`, each `{ status: 'pass' | 'fail' | 'skipped', basis: 'cryptographic' | 'consistency' | 'trusts-canister', detail }`
  - `errors: string[]`, `warnings: string[]`
- **Checks:**
  - **Commitment:** recompute it from the seed, the hand ID and the parameters. Also confirm the log has `ShuffleCommitted` before the first dealt card.
  - **Shuffle:** rebuild the deck.
  - **Dealing:** simulate the engine's dealing from the rebuilt deck using `dealt_order` and the burns. Every known hole card and every board card must match.
  - **Replay:** check that the log's sequence of streets and actions is consistent with the board length and the folded and all-in players.
  - **Settlement:** independently compute the pots and the winners from the rebuilt cards and the log's commitments. Compare against `players[].won`, the rake and the jackpot drop.
- **Overall result:**
  - `fair == null` gives `LEGACY`.
  - An unknown version gives `UNSUPPORTED`.
  - Missing log events or fields give `INCOMPLETE`.
  - A seed not yet revealed (`now < reveal_at`) gives `INCOMPLETE` with the reason "pending reveal" and `reveal_at`, never `FAILED`.
  - Any `fail` gives `FAILED`.
  - Otherwise `VERIFIED`.
- **Report wording:** commitment, shuffle and dealing are `cryptographic`. Replay and settlement are `consistency`. The seed's origin and the confidentiality of cards during play are listed as `trusts-canister`, and are never claimed as passed.

- [ ] **Step 1: Write the failing tests.**
  - Settlement vectors: a split pot, a three-way all-in with side pots, everyone folding, no flop, and the rake cap. Each must match the engine.
  - Valid hand → `VERIFIED`.
  - Changed seed → commitment fails.
  - Changed commitment → fails.
  - A tampered hole card or board card → dealing fails, naming the card and its position.
  - A missing `ShuffleCommitted` event → `INCOMPLETE`.
  - A changed payout → settlement fails, naming the player and the expected and actual amounts.
  - A legacy record → `LEGACY`.
  - An unknown version → `UNSUPPORTED`.
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `npx vitest run && npm run build`. Expected: everything passes.
- [ ] **Step 5: Commit** "Verify hands independently in the browser".

### Task 6: Evidence export and offline verification (frontend)

**Files:**
- Create: `src/poker_frontend/src/fair/evidence.ts` and its tests, plus `components/VerifyEvidence.tsx`.

**Interfaces:**
- **Produces:**
  - `toEvidenceJson(record, evidence): string`, in canonical JSON:
    - `format: "bulletspoker-hand-evidence/v1"`;
    - sorted keys;
    - bytes as lowercase hex;
    - integers as decimal strings.
  - `fromEvidenceJson(text) -> { record, evidence } | { error }`.
  - A route or view where a pasted or uploaded evidence file is verified with no canister call.

- [ ] **Step 1: Write the failing tests:**
  - A JSON round trip gives the same report.
  - The output is the same canonical text for objects with different key order.
  - Malformed JSON gives a clear error.
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `npx vitest run && npm run build`. Expected: everything passes.
- [ ] **Step 5: Commit** "Export and verify hand evidence offline".

### Task 7: Verify Hand in the replay (frontend)

**Files:**
- Modify: `src/poker_frontend/src/components/History.tsx`, `styles.css`.
- Create: `components/HandVerification.tsx`.

**Interfaces:**
- **Behaviour:**
  - A "Verify hand" button on completed hands that have `fair` and a revealed seed.
  - Before `reveal_at`, the replay shows "Verification available in 23 h 41 m" with the commitment already visible. After the retention window, it shows "Verification window ended — use exported evidence".
  - Legacy hands show a "Legacy / Not verifiable" badge, never a failure.
  - The rules page and the auto-muck setting carry the D1 line: "24 hours after a hand, the players in it can verify the shuffle, which reveals every card dealt in that hand."
  - Progress, then one row per check with its basis: Seed commitment, Deterministic shuffle, Dealt cards, Replay consistency, Pot settlement, and Overall.
  - The commitment, the seed (after the reveal), the shuffle version and the scheme are shown with copy buttons, along with "Export evidence".
  - Use the existing `Dialog` and panel styles.
- **Disclaimer, always visible:** "This check proves the cards dealt match a seed committed before the hand and that the pot was paid by the rules. It does not prove the seed came from ICP randomness, and it does not prove nobody could see the cards during play — the game canister holds them."

- [ ] **Step 1: Write the failing tests** for a pure `verificationRows(report)` helper: labels, status text, and legacy and unsupported wording.
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `npx vitest run && npm run build`. Expected: everything passes.
- [ ] **Step 5: Commit** "Add Verify Hand to the replay".

### Task 8: Threat model and documentation

**Files:**
- Create: `docs/provably-fair-threat-model.md`.
- Modify:
  - `docs/provably-fair.md`.
  - `README.md`: the "Card secrecy" section links to both documents.

**Content (binding):**
- Assess each of the brief's 10 threats as **Mitigated**, **Detectable** or **Residual**, with the reason. In particular:
  - **Dishonest dealer choosing among seeds** (threat 6): residual. The canister picks the seed; a commitment doesn't stop a malicious code upgrade.
  - **Seed or deck visible to the subnet, replicas, controllers and snapshots** (threats 2 and 9): residual. These are plaintext canister state.
  - **Aborting after unfavourable cards** (threat 7): sitting out and leaving don't void a hand; the engine auto-acts.
  - **Async `raw_rand`** (threat 4): mitigated by the Task 3 checks.
  - **Upgrades rewriting history** (threat 8): residual. Exported evidence is the player's own copy.
  - **Collusion** (threat 10): out of scope; refer to moderation.
- State plainly that `raw_rand` output carries no public proof of its origin. ICP's threshold randomness is a network-level property, not something a player can check.
- Propose the future enhancements as separate projects, not built here:
  - vetKeys-based encrypted dealing;
  - mental poker and multi-party shuffles;
  - verifiable builds with published module hashes;
  - commit-reveal with player-contributed entropy.

- [ ] **Step 1: Write the two documents.** Check every claim against the code from Tasks 1–7.
- [ ] **Step 2: Commit** "Document the fair-shuffle protocol and its threat model".

### Task 9: End-to-end verification and cycle check

**Files:**
- Modify: `src/poker_backend/tests/integration.rs`, the PocketIC suite that runs in CI.

- [ ] **Step 1: Add an integration test.** Play a full ICP hand on PocketIC, fetch it with `get_hand`, rebuild the deck in Rust from the published evidence, and match the dealt cards. Also assert that during the hand neither `get_table` nor `get_tables` contains the seed bytes.
- [ ] **Step 2: Measure the cycles.** Locally, compare cycles per hand before and after (one `raw_rand` per hand, D2a). Record the result in `docs/provably-fair.md`.
- [ ] **Step 3: Run** `cargo test`, `npx vitest run` and `npm run build`. Expected: everything passes. Then commit "Test fair dealing end to end and record its cost".

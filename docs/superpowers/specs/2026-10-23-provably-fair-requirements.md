# Provably fair poker: requirements

The feature brief as given by the owner on 2026-10-23. The implementation plan
(`docs/superpowers/plans/2026-10-23-provably-fair.md`) maps it onto this
codebase. That codebase uses a Rust canister, not Motoko, so read "Motoko" below
as the Rust backend.

## Task

Extend the existing ICP Texas Hold'em app. Do not rebuild or replace the poker
engine or the replay system; integrate with them and stay compatible.

The extension adds:
- cryptographically verifiable randomness;
- deterministic card shuffling;
- seed commitments;
- an independently verifiable hand-auditing system.

## Phase 1: Analyze the existing codebase

Inspect:
- the engine, deck generation and randomness;
- where shuffling, dealing, player actions, settlement and replay events live;
- the canisters, Candid interfaces and frontend;
- hand identifiers, event schemas, persistence and lifecycle hooks;
- how replay reconstructs hands.

Then produce a concise integration plan of the files and interfaces to change,
reusing what exists.

## Phase 2: ICP native randomness

Use management-canister `raw_rand()`.

- Request 32 bytes per new hand.
- Make the async calls correctly, and handle failures safely.
- Never fall back to predictable randomness, timestamps or `Math.random()`.
- One hand must never have more than one active seed. Handle async execution,
  duplicate requests and concurrent state changes.
- Bind the randomness to a unique hand ID and the immutable game parameters.
- Use a dedicated randomness module, or extend the existing one.
- Don't claim that `raw_rand()` output carries a publicly consumable proof.
  Document the difference between ICP's network-level threshold randomness and
  application-level hand verification.

## Phase 3: Deterministic, unbiased shuffle

- Fisher–Yates over the 52-card deck.
- The random stream comes from a documented construction: HKDF-SHA256 with a
  suitable CSPRNG, or a specified HMAC-SHA256 counter stream.
- Domain separation, with the hand ID included in seed derivation.
- Rejection sampling, so there's no modulo bias.
- Identical seed and parameters give an identical deck.
- A versioned canonical deck representation, card ordering, byte encoding and
  shuffle specification.
- An internal API like `shuffleDeck(seed, handId, shuffleVersion) -> Deck`, using
  the existing card types.
- Deterministic test vectors shared between the backend and TypeScript.

## Phase 4: Seed commitment

For each hand:
1. Generate a secret seed.
2. Compute `commitment = SHA256(domain || seed || handId || canonicalParameters)`
   with an unambiguous, length-delimited encoding.
3. Persist it in the authoritative hand state.
4. Make it available before relevant cards are dealt or any betting happens.
5. Lock the commitment and the parameters.
6. Reject any overwrite of a finalized commitment.

Also:
- Record a `ShuffleCommitted` event in the hand history and replay.
- A hand can't become dealable without a persisted commitment.
- **Limitation:** a commitment proves consistency with what was recorded. It
  doesn't prove the seed came from `raw_rand()`, that it was unbiased, or that
  nobody could see hidden cards.

## Phase 5: Engine integration

The hand lifecycle runs through these states:
1. Initialization.
2. Randomness pending.
3. Randomness generated and commitment recorded.
4. Deck shuffled.
5. Cards dealt (by the existing engine).
6. Betting.
7. Settlement final.
8. Seed revealed and verification data published.
9. Hand available for audit.

Adapt the existing lifecycle rather than adding a parallel engine.

Requirements:
- Don't change the rules, hand rankings, betting, side pots, all-ins, folds or
  settlement unless integration requires it.
- Commit and reveal are idempotent.
- The seed can't be revealed through public APIs before the authorized reveal
  point, and is exposed only after the hand is irrevocably settled.
- A new hand can't reuse the previous hand's seed.
- Cancellation, abandonment, disconnects and interruptions have explicit
  terminal states.
- Don't reveal seed material for an abandoned hand while it could still expose
  actionable private information.
- Stay backward compatible with historical hands.

## Phase 6: Replay extension

Add the minimum metadata. Suggested fields:
- `shuffleVersion`
- `randomnessScheme`
- `handId`
- `seedCommitment`
- `seedReveal`
- `shuffleParameters`
- optional `deckCommitment`
- `verificationStatus`

Suggested events: `ShuffleCommitted`, `ShuffleRevealed` and
`HandVerificationAvailable`.

Also:
- Existing replay keeps working.
- Legacy hands show "Legacy / Not Verifiable", not a failure.
- Don't store unverifiable success flags as authoritative. Status is computed
  from the cryptographic inputs and the events.

## Phase 7: Independent TypeScript verifier

The verifier must not trust a backend `verifyHand()`. It must:
1. Retrieve the finalized history and the metadata.
2. Confirm the commitment came before dealing.
3. Validate the commitment against the revealed seed.
4. Rebuild the deck.
5. Reproduce the dealing from the recorded progression.
6. Check the recorded hole and community cards.
7. Validate the final state against the replay.
8. Verify the winners, the pot distribution and side pots independently.
9. Report mismatches precisely.

API: `verifyHand(handHistory, verificationData) -> VerificationReport`.

The report includes:
- Overall: `VERIFIED`, `FAILED`, `INCOMPLETE`, `LEGACY` or `UNSUPPORTED`.
- Separate results for the commitment, the shuffle, dealing, replay integrity
  and settlement.
- Errors and warnings.
- The protocol version.

Distinguish checks that are cryptographically proven from those that trust the
canister.

## Phase 8: Frontend

- Reuse the replay components.
- A "Verify Hand" button for eligible completed hands, with progress and
  per-check results.
- Show the commitment, the revealed seed and the shuffle version.
- Explain what was and wasn't verified.
- Copy or export the evidence as canonical, versioned JSON that can be verified
  without the live canister.
- Handle the legacy, incomplete, failed and unsupported states.
- A visible disclaimer: post-hand verification doesn't guarantee card
  confidentiality, and doesn't prove where the ICP randomness came from.
- Use the existing styling and patterns.

## Phase 9: Threat model

Assess each of these:
1. An operator or privileged method replacing a committed shuffle.
2. Seeing the deck or seed early.
3. A malicious participant manipulating events.
4. Exploiting async `raw_rand()` calls.
5. Replay tampering or selective omission of events.
6. A dealer generating many outcomes and choosing one.
7. Aborting after unfavourable cards.
8. Upgrades or admin controls modifying history.
9. Seed leakage through logs, queries, backups, stable state or diagnostics.
10. Collusion.

A canister that stores the seed and deck in plaintext is not trustless mental
poker. Don't claim protection against subnet participants, privileged actors or
a compromised execution environment. Document the residual trust assumptions.
Don't build mental poker or a zero-knowledge proof system here; propose them as
future work.

## Phase 10: Tests

- **Randomness and commitments:**
  - seed size;
  - no duplicate initialization;
  - immutability;
  - valid and invalid seeds;
  - canonical serialization;
  - no seed reuse.
- **Shuffling:**
  - 52 unique cards, each exactly once;
  - determinism;
  - different inputs give different decks;
  - rejection sampling;
  - matching cross-language vectors.
- **Lifecycle:**
  - commitment before dealing;
  - no early reveal;
  - settlement before reveal;
  - interrupted and cancelled hands;
  - async safety.
- **Replay and verification:**
  - a valid hand verifies;
  - a changed seed, a changed commitment, a tampered card or a modified payout
    fails;
  - a missing event is incomplete or fails;
  - side pots are reconstructed;
  - legacy replays still work.

## Phase 11: Deliverables

1. Randomness and shuffle integration.
2. The commit and reveal lifecycle.
3. Extended evidence.
4. The TypeScript verifier.
5. The replay UI.
6. Tests.
7. Updated Candid.
8. Developer documentation.

Afterwards:
- Run builds and tests.
- Report the files changed and the decisions made.
- State the unresolved limitations.
- Separate tested work from recommendations.
- Never claim tests passed unless they actually ran.

**Primary objective:** every eligible completed hand has a reproducible,
independently auditable shuffle and a verifiable replay, without disrupting the
game.

**Non-objective:** don't claim that replay and commitment verification alone
give trustless or confidential poker.

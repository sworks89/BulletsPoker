# BP-FAIR-v1: the BulletsPoker shuffle and commitment protocol

This document specifies, byte for byte, how BulletsPoker commits to a hand's
shuffle before dealing and how anyone can recompute that shuffle after the seed
is revealed. It is enough to write an independent verifier.

- Reference implementation (Rust, canister): `src/poker_backend/src/fair.rs`
- Shared test vectors: `src/poker_frontend/src/fair/vectors.json`. Every
  implementation must reproduce every field of every case exactly.

## What this does and does not show

When a hand's seed is revealed, a verifier can check two things: the
seed matches the commitment the canister logged as entry 0 of the hand's log
before the first card was dealt, and the deck that was dealt is exactly the one
this protocol derives from that seed. This means the shuffle cannot have been
swapped after the commitment without detection. Players do not see the
commitment during the hand; it is published with the hand record afterwards.

It is **not** mental poker, and it keeps no card secret from the canister. The canister holds the
seed and the deck in its state while the hand is played. A verifier must still
trust the canister's code, the Internet Computer subnet that runs it, and the
management canister's `raw_rand`, which supplies the seed. `raw_rand` output
carries no public proof of its origin. The in-app verifier is served by the
same operator, so for high assurance, export a hand's evidence and check it
with a verifier you build or write yourself from this document. The
[threat model](provably-fair-threat-model.md) lists what this verifiable
shuffle and hand history do and do not protect against, and the remaining
trust assumptions.

## Constants

| Name | Value |
| --- | --- |
| `SHUFFLE_VERSION` | `1` |
| `RANDOMNESS_SCHEME` | `"icp-raw_rand/v1"` (one 32-byte `raw_rand` result per hand) |
| commit domain | `"BulletsPoker/commit/v1"` (ASCII) |
| HKDF salt | `"BulletsPoker/shuffle/v1"` (ASCII) |
| card encoding | `"card=rank*4+suit;rank0=2..rank12=A;suit0=c,1=d,2=h,3=s"` (ASCII) |
| deal order | `"deal=pop-from-end;burn-before-flop-turn-river"` (ASCII) |

## Notation

- `‖` is byte concatenation.
- `u8(x)`, `u16be(x)`, `u32be(x)` and `u64be(x)` are unsigned integers of 1, 2, 4
  and 8 bytes, **big-endian**.
- `lp(x) = u32be(len(x)) ‖ x` is a length prefix. A field that is already
  length-prefixed is prefixed again when it is nested, as shown below.
- Strings are their ASCII bytes, without a terminator.

## Cards

A card is an integer `0..=51`. `rank = card / 4` (0 = 2, 1 = 3, …, 8 = T,
9 = J, 10 = Q, 11 = K, 12 = A) and `suit = card % 4` (0 = clubs, 1 = diamonds,
2 = hearts, 3 = spades). For example, card 12 is the 5 of clubs and card 51 is
the ace of spades.

## Inputs

- `seed`: 32 bytes from the management canister's `raw_rand`.
- `canister`: the principal of the canister that dealt the hand, as its raw
  bytes (0 to 29 bytes). These are the bytes inside the textual form
  `xxxxx-…-cai`: decode the base32 text and drop the first 4 bytes, which are a
  CRC-32 checksum. The management canister has 0 bytes; the anonymous principal
  is the single byte `04`.
- `table_id`: u64.
- `hand_no`: u64.
- `version`: u16, currently 1.

## Encoding

```
hand_id_bytes = lp(canister_bytes) ‖ u64be(table_id) ‖ u64be(hand_no)

params_bytes  = u16be(version) ‖ u8(52)
              ‖ lp("card=rank*4+suit;rank0=2..rank12=A;suit0=c,1=d,2=h,3=s")
              ‖ lp("deal=pop-from-end;burn-before-flop-turn-river")
```

For version 1, `params_bytes` is always the same 110 bytes.

## Commitment

```
commitment = SHA-256( lp("BulletsPoker/commit/v1")
                    ‖ lp(seed)
                    ‖ lp(hand_id_bytes)
                    ‖ lp(params_bytes) )
```

The canister records the commitment as entry 0 of the hand's log, in the same
step that deals the hole cards, so no card leaves the deck before it. While the
hand is played, the table view (`TableView.commitment`) shows the commitment to
everyone watching, and the app keeps a copy on the player's device
(`fair/seen.ts`). A copy seen at the table shows the deck was fixed before the
flop and before any betting. Without one, the order rests on the canister's own
log, which is not externally timestamped. The seed is revealed after the hand. A verifier recomputes the commitment from the revealed
seed and the hand id and compares all 32 bytes. A verifier must reject any
version it does not implement.

## Shuffle key

HKDF-SHA256 (RFC 5869):

```
key = HKDF-SHA256( IKM  = seed,
                   salt = "BulletsPoker/shuffle/v1",        (raw bytes, no lp)
                   info = lp(hand_id_bytes) ‖ lp(params_bytes),
                   L    = 32 )
```

Equivalently, `PRK = HMAC-SHA256(salt, seed)` and
`key = HMAC-SHA256(PRK, info ‖ 0x01)`.

## Random stream

```
block(i) = HMAC-SHA256(key, u64be(i))     for i = 0, 1, 2, …
```

The stream is block 0, then block 1, and so on, read as successive 4-byte
**big-endian** `u32` words (8 words per block). Words left over from one draw
are used by the next draw. A new block is computed only when the current one is
used up.

## Uniform draw

`uniform(n)` for `n` in `2..=52`:

```
limit = 2^32 − (2^32 mod n)          (compute in 64-bit arithmetic)
repeat: v = next word
until v < limit
return v mod n
```

Words at or above `limit` are discarded, which removes the modulo bias. When `n`
is a power of two, `limit = 2^32` and no word is rejected. For `n = 52`,
`limit = 4294967248`, so the 48 largest words are rejected. Rejections are rare:
none of the published vectors contains one. The unit test
`uniform_rejects_the_biased_zone` covers this path.

## Shuffle

```
deck = [0, 1, 2, …, 51]
for i = 51 down to 1:
    j = uniform(i + 1)
    swap deck[i], deck[j]
```

`n` therefore runs from 52 down to 2: 51 draws, which usually consume 51 words
(7 blocks; the last block is only partly used).

## Dealing

Cards are dealt by popping from the **end** of `deck`, so `deck[51]` is the first
card dealt. Hole cards go one at a time, starting with the seat left of the
button, for two rounds. One card is burned (popped and not shown) before the
flop, before the turn and before the river.

## Published evidence

Each recorded hand carries a `fair` record (`FairEvidence` in the candid
interface), returned by `get_hand` to the players dealt into that hand and to
nobody else:

| Field | Meaning |
|---|---|
| `version`, `scheme` | shuffle version and randomness source (`icp-raw_rand/v1`) |
| `canister`, `table_id`, `hand_no` | the hand id used in the encodings above |
| `commitment`, `params` | the commitment and `params_bytes(version)` |
| `revealed_seed` | the 32-byte seed; empty until `reveal_at` |
| `outcome` | `settled`, or `voided: <reason>` for a hand that never finished |
| `dealt_order` | the seat of each hole card in the order it was popped (both rounds, `2 × players` entries) |
| `burns` | burn cards used: one before each street that was dealt |
| `settled_at`, `reveal_at` | when the hand ended; `reveal_at = settled_at + 24 h` |
| `rake_bps`, `rake_cap`, `jackpot_bps`, `jackpot_cap` | the table's rake and jackpot-drop rates when the hand was dealt; empty for hands dealt before the rates were recorded |

The seed is withheld for 24 hours after the hand ends, because it reveals every
card dealt, including folded and mucked ones. Hands that were dealt but never
finished (their table closed) are published the same way, with an empty
settlement.

The hand record, and so the evidence, is kept for at least 7 days after
`reveal_at`, beyond the usual history limit. Storage has a hard limit of
200,000 hand records; if even that is reached, the oldest records are removed
early. Once a record is gone, the evidence a player exported is the only copy.

## Worked example

This is case 2 of `vectors.json`.

### Inputs

| Field | Value |
| --- | --- |
| seed | `000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f` |
| canister | `ryjl3-tyaaa-aaaaa-aaaba-cai`, bytes `00000000000000020101` (10 bytes) |
| table_id | `1` |
| hand_no | `1` |
| version | `1` |

### hand_id_bytes (30 bytes)

```
0000000a                    lp length = 10
00000000000000020101        canister bytes
0000000000000001            u64be(table_id)
0000000000000001            u64be(hand_no)
```

That is `0000000a0000000000000002010100000000000000010000000000000001`.

### params_bytes (110 bytes)

```
0001                        u16be(1)
34                          u8(52)
00000036                    lp length = 54
636172643d72616e6b2a342b737569743b72616e6b303d322e2e72616e6b31323d413b73756974303d632c313d642c323d682c333d73
                            "card=rank*4+suit;rank0=2..rank12=A;suit0=c,1=d,2=h,3=s"
0000002d                    lp length = 45
6465616c3d706f702d66726f6d2d656e643b6275726e2d6265666f72652d666c6f702d7475726e2d7269766572
                            "deal=pop-from-end;burn-before-flop-turn-river"
```

### Commitment

```
39551fd29065699e1e6d9528bfdba8deac9d362604f20d85e66f16f7acaa30e0
```

### Shuffle key

```
PRK = dfba0f715adc96de45c8839d4b733b624db20511c022fbd85d6e0533f526b9c4
key = 5785712322da31092734e7bfeff10111dbbe63bf34de3ecff32bc7a285bbe1f5
```

### First stream block

```
block(0) = 47390d386183d1725fcb616e17dbb94c9c916ef107539546be1b38504c9b0c27
```

Words: `47390d38`, `6183d172`, `5fcb616e`, `17dbb94c`, `9c916ef1`, `07539546`,
`be1b3850`, `4c9b0c27`.

### First draws

None of these words is rejected.

| i | n | word v | limit | j = v mod n | swap |
| --- | --- | --- | --- | --- | --- |
| 51 | 52 | `0x47390d38` = 1194921272 | 4294967248 | 12 | deck[51]=51 ↔ deck[12]=12 |
| 50 | 51 | `0x6183d172` = 1636028786 | 4294967295 | 41 | deck[50]=50 ↔ deck[41]=41 |
| 49 | 50 | `0x5fcb616e` = 1607164270 | 4294967250 | 20 | deck[49]=49 ↔ deck[20]=20 |
| 48 | 49 | `0x17dbb94c` = 400275788 | 4294967257 | 31 | deck[48]=48 ↔ deck[31]=31 |
| 47 | 48 | `0x9c916ef1` = 2626776817 | 4294967280 | 1 | deck[47]=47 ↔ deck[1]=1 |
| 46 | 47 | `0x07539546` = 122918214 | 4294967254 | 7 | deck[46]=46 ↔ deck[7]=7 |

The shuffle continues through block 6.

### Final deck

```
[5, 22, 30, 49, 9, 23, 24, 10, 2, 18, 37, 13, 6, 35, 38, 45, 29, 50, 46, 44,
 39, 3, 15, 48, 51, 0, 16, 14, 17, 40, 25, 28, 8, 19, 4, 36, 21, 27, 26, 47,
 11, 34, 33, 43, 42, 32, 7, 1, 31, 20, 41, 12]
```

The first four cards dealt (popped from the end) are 12 (5c), 41 (Qd), 20 (7c)
and 31 (9s).

## Test vectors

`src/poker_frontend/src/fair/vectors.json` has the form
`{ "protocol": "BP-FAIR-v1", "cases": [ … ] }`. Each case has these fields:

| Field | Meaning |
| --- | --- |
| `seed_hex` | 32-byte seed, lowercase hex |
| `canister_text` | principal in textual form |
| `canister_hex` | principal bytes, lowercase hex (may be empty) |
| `table_id`, `hand_no` | u64 as **decimal strings**, because JSON numbers cannot hold every u64 exactly |
| `version` | shuffle version (number) |
| `hand_id_hex`, `params_hex` | the encodings above |
| `commitment_hex` | the commitment |
| `key_hex` | the HKDF shuffle key |
| `deck` | the 52 card integers after the shuffle |

The cases cover an all-zero seed with the management canister (empty principal)
and zero ids, an all-`0xFF` seed with the anonymous principal and `u64::MAX`
ids, a 29-byte principal, real 10-byte canister ids, and two hands that differ
only in `hand_no`.

The file is generated by the Rust test `fair::tests::vectors_file_matches`:

```
UPDATE_FAIR_VECTORS=1 cargo test -p poker_backend vectors_file_matches
```

Without the variable, the test fails if the file differs from what the code
produces. A change to the vectors is a change to the protocol and needs a new
version number.

## Measured cost

Measured on 2026-10-24 by `cycle_cost_per_hand` in
`src/poker_backend/tests/integration.rs` (run through
`scripts/integration-test.sh`), against the backend canister on PocketIC 16.1.0
with an application subnet.

- Each hand draws one `raw_rand` (D2a), hashes one commitment and runs one
  shuffle.
- N = 25 heads-up hands on a 0.01/0.02 ICP table, each ended by a fold, in 146
  simulated seconds.
- The backend burned 3,993,153,042 cycles while playing them, and
  2,626,251,826 cycles over the same 146 one-second steps with nobody seated.
- The difference is about 1.37 billion cycles over 25 hands: roughly 54.7
  million cycles per hand (gross, without subtracting idle: 159.7 million).

Caveats:

- The per-hand figure includes the `act` ingress message that ends each hand,
  all of the table's other timer work and the hand's storage. The `raw_rand`
  fee is not separated out, and the dealing path with and without it was not
  compared.
- PocketIC's application subnet charges its own fee schedule and subnet size.
  Mainnet figures will differ; treat these as an order of magnitude, not a
  quote.
- One run, no repetitions. The test prints the numbers and asserts nothing
  about them.

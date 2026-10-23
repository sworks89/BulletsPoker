//! `BP-FAIR-v1`: the commit-reveal shuffle protocol.
//!
//! Before a hand is dealt the canister publishes `commitment(seed, hand)`.
//! After the hand the seed is revealed, and anyone holding the hand id can
//! recompute the commitment and the exact deck with [`shuffle_deck`].
//! The byte-level specification is `docs/provably-fair.md`; the shared test
//! vectors live in `src/poker_frontend/src/fair/vectors.json`.
//!
//! Never log the seed and never put it in an error message.

use crate::cards::Card;
use candid::Principal;
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

/// The only shuffle version this build can deal and verify.
pub const SHUFFLE_VERSION: u16 = 1;
/// Where the 32-byte seed comes from: one management-canister `raw_rand` per hand.
pub const RANDOMNESS_SCHEME: &str = "icp-raw_rand/v1";

const DECK_SIZE: u8 = 52;
const CARD_ENCODING: &str = "card=rank*4+suit;rank0=2..rank12=A;suit0=c,1=d,2=h,3=s";
const DEAL_ORDER: &str = "deal=pop-from-end;burn-before-flop-turn-river";
const COMMIT_DOMAIN: &str = "BulletsPoker/commit/v1";
const SHUFFLE_SALT: &str = "BulletsPoker/shuffle/v1";

/// Identifies one hand: the canister that dealt it, the table and the hand number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandId {
    pub canister: Principal,
    pub table_id: u64,
    pub hand_no: u64,
}

impl HandId {
    /// `lp(canister_principal_bytes) ‖ u64be(table_id) ‖ u64be(hand_no)`.
    pub fn bytes(&self) -> Vec<u8> {
        let mut out = lp(self.canister.as_slice());
        out.extend_from_slice(&self.table_id.to_be_bytes());
        out.extend_from_slice(&self.hand_no.to_be_bytes());
        out
    }
}

/// Length prefix: `u32be(len(x)) ‖ x`.
pub fn lp(x: &[u8]) -> Vec<u8> {
    let len = u32::try_from(x.len()).expect("field longer than u32::MAX");
    let mut out = Vec::with_capacity(4 + x.len());
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(x);
    out
}

/// `u16be(version) ‖ u8(52) ‖ lp(card encoding) ‖ lp(deal order)`.
pub fn params_bytes(version: u16) -> Vec<u8> {
    let mut out = version.to_be_bytes().to_vec();
    out.push(DECK_SIZE);
    out.extend(lp(CARD_ENCODING.as_bytes()));
    out.extend(lp(DEAL_ORDER.as_bytes()));
    out
}

/// `SHA256(lp("BulletsPoker/commit/v1") ‖ lp(seed) ‖ lp(hand_id_bytes) ‖ lp(params_bytes))`.
pub fn commitment(seed: &[u8; 32], hand: &HandId, version: u16) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(lp(COMMIT_DOMAIN.as_bytes()));
    h.update(lp(seed));
    h.update(lp(&hand.bytes()));
    h.update(lp(&params_bytes(version)));
    h.finalize().into()
}

/// True when `commitment` was made from exactly these inputs under a known version.
pub fn verify_commitment(seed: &[u8; 32], hand: &HandId, version: u16, commitment_: &[u8; 32]) -> bool {
    version == SHUFFLE_VERSION && commitment(seed, hand, version) == *commitment_
}

/// `HKDF-SHA256(ikm = seed, salt = "BulletsPoker/shuffle/v1",
/// info = lp(hand_id_bytes) ‖ lp(params_bytes), L = 32)`.
pub fn shuffle_key(seed: &[u8; 32], hand: &HandId, version: u16) -> [u8; 32] {
    let mut info = lp(&hand.bytes());
    info.extend(lp(&params_bytes(version)));
    let mut key = [0u8; 32];
    Hkdf::<Sha256>::new(Some(SHUFFLE_SALT.as_bytes()), seed)
        .expand(&info, &mut key)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    key
}

/// Block `i` is `HMAC-SHA256(key, u64be(i))`, read as successive big-endian
/// `u32` words. Unused words carry over from one draw to the next.
pub struct Stream {
    key: [u8; 32],
    block: u64,
    buf: [u8; 32],
    pos: usize,
}

impl Stream {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key, block: 0, buf: [0; 32], pos: 32 }
    }

    pub fn next_u32(&mut self) -> u32 {
        if self.pos == 32 {
            let mut m = Hmac::<Sha256>::new_from_slice(&self.key).expect("HMAC accepts any key length");
            m.update(&self.block.to_be_bytes());
            self.buf = m.finalize().into_bytes().into();
            self.block += 1;
            self.pos = 0;
        }
        let w = u32::from_be_bytes(self.buf[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        w
    }
}

/// Uniform value in `0..n`: with `limit = 2^32 − (2^32 mod n)`, draw words
/// until one is below `limit`, then return it `mod n`.
pub fn uniform<F: FnMut() -> u32>(n: u32, next: &mut F) -> u32 {
    assert!(n >= 1);
    let limit = (1u64 << 32) - ((1u64 << 32) % n as u64);
    loop {
        let v = next();
        if (v as u64) < limit {
            return v % n;
        }
    }
}

/// The deck for one hand. Starts from `[0, 1, …, 51]`; for `i` from 51 down
/// to 1, swaps `deck[i]` with `deck[uniform(i + 1)]`. Cards are dealt by
/// popping from the end.
pub fn shuffle_deck(seed: &[u8; 32], hand: &HandId, version: u16) -> Result<Vec<Card>, String> {
    if version != SHUFFLE_VERSION {
        return Err(format!("unknown shuffle version {version}"));
    }
    let mut stream = Stream::new(shuffle_key(seed, hand, version));
    let mut next = || stream.next_u32();
    let mut deck: Vec<Card> = (0..DECK_SIZE).collect();
    for i in (1..deck.len()).rev() {
        let j = uniform(i as u32 + 1, &mut next) as usize;
        deck.swap(i, j);
    }
    Ok(deck)
}

#[cfg(test)]
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Inputs for the shared test vectors.
#[cfg(test)]
fn vector_cases() -> Vec<([u8; 32], HandId)> {
    let mut mixed = [0u8; 32];
    for (i, b) in mixed.iter_mut().enumerate() {
        *b = i as u8;
    }
    let mut mixed2 = [0u8; 32];
    for (i, b) in mixed2.iter_mut().enumerate() {
        *b = (i as u8).wrapping_mul(37).wrapping_add(11);
    }
    let long: Vec<u8> = (0..29u8).map(|i| 0xF0 ^ i.wrapping_mul(7)).collect();
    let canister = |t: &str| Principal::from_text(t).unwrap();
    vec![
        ([0x00; 32], HandId { canister: Principal::management_canister(), table_id: 0, hand_no: 0 }),
        ([0xFF; 32], HandId { canister: Principal::anonymous(), table_id: u64::MAX, hand_no: u64::MAX }),
        (mixed, HandId { canister: canister("ryjl3-tyaaa-aaaaa-aaaba-cai"), table_id: 1, hand_no: 1 }),
        (mixed2, HandId { canister: Principal::from_slice(&long), table_id: 0, hand_no: u64::MAX }),
        ([0x5A; 32], HandId { canister: canister("rrkah-fqaaa-aaaaa-aaaaq-cai"), table_id: u64::MAX, hand_no: 0 }),
        ([0x01; 32], HandId { canister: canister("bkyz2-fmaaa-aaaaa-qaaaq-cai"), table_id: 3, hand_no: 1_234_567 }),
        (mixed, HandId { canister: canister("ryjl3-tyaaa-aaaaa-aaaba-cai"), table_id: 1, hand_no: 2 }),
    ]
}

/// Cases for the `uniform` section: a fixed word list drives `uniform`, so the
/// rejection path is cross-checked in both languages.
#[cfg(test)]
fn uniform_cases() -> Vec<(u32, Vec<u32>)> {
    let limit52 = ((1u64 << 32) - ((1u64 << 32) % 52)) as u32;
    vec![
        (52, vec![5]),
        (52, vec![u32::MAX, 5]),
        (52, vec![limit52, limit52 - 1]),
        (52, vec![limit52 - 1]),
        (52, vec![u32::MAX, limit52 + 7, 1_000_003]),
        (3, vec![u32::MAX, u32::MAX, 7]),
        (3, vec![u32::MAX - 1]),
        (4, vec![u32::MAX]),
        (1, vec![123]),
        (2, vec![u32::MAX]),
        (51, vec![0, 1]),
    ]
}

/// The shared test vectors as stable, pretty-printed JSON (fields in a fixed
/// order, u64 values as decimal strings, bytes as lowercase hex).
#[cfg(test)]
fn vectors_json() -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"protocol\": \"BP-FAIR-v1\",\n");
    out.push_str("  \"cases\": [\n");
    let cases = vector_cases();
    for (n, (seed, hand)) in cases.iter().enumerate() {
        let v = SHUFFLE_VERSION;
        let deck = shuffle_deck(seed, hand, v).unwrap();
        let deck_txt: Vec<String> = deck.iter().map(|c| c.to_string()).collect();
        out.push_str("    {\n");
        out.push_str(&format!("      \"seed_hex\": \"{}\",\n", hex(seed)));
        out.push_str(&format!("      \"canister_text\": \"{}\",\n", hand.canister.to_text()));
        out.push_str(&format!("      \"canister_hex\": \"{}\",\n", hex(hand.canister.as_slice())));
        out.push_str(&format!("      \"table_id\": \"{}\",\n", hand.table_id));
        out.push_str(&format!("      \"hand_no\": \"{}\",\n", hand.hand_no));
        out.push_str(&format!("      \"version\": {v},\n"));
        out.push_str(&format!("      \"hand_id_hex\": \"{}\",\n", hex(&hand.bytes())));
        out.push_str(&format!("      \"params_hex\": \"{}\",\n", hex(&params_bytes(v))));
        out.push_str(&format!("      \"commitment_hex\": \"{}\",\n", hex(&commitment(seed, hand, v))));
        out.push_str(&format!("      \"key_hex\": \"{}\",\n", hex(&shuffle_key(seed, hand, v))));
        out.push_str(&format!("      \"deck\": [{}]\n", deck_txt.join(", ")));
        out.push_str(if n + 1 == cases.len() { "    }\n" } else { "    },\n" });
    }
    out.push_str("  ],\n");
    out.push_str("  \"uniform\": [\n");
    let ucases = uniform_cases();
    for (k, (n, words)) in ucases.iter().enumerate() {
        let mut used = 0usize;
        let mut next = || {
            let w = words[used];
            used += 1;
            w
        };
        let result = uniform(*n, &mut next);
        let ws: Vec<String> = words.iter().map(|w| format!("\"{w}\"")).collect();
        out.push_str(&format!(
            "    {{ \"n\": {n}, \"words\": [{}], \"result\": {result}, \"consumed\": {used} }}{}\n",
            ws.join(", "),
            if k + 1 == ucases.len() { "" } else { "," }
        ));
    }
    out.push_str("  ]\n}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use candid::Principal;

    fn hand(canister: Principal, table_id: u64, hand_no: u64) -> HandId {
        HandId { canister, table_id, hand_no }
    }

    fn some_hand() -> HandId {
        hand(Principal::from_text("ryjl3-tyaaa-aaaaa-aaaba-cai").unwrap(), 7, 42)
    }

    fn seed_from(n: u32) -> [u8; 32] {
        let mut s = [0u8; 32];
        s[..4].copy_from_slice(&n.to_be_bytes());
        s[31] = 0xA5;
        s
    }

    #[test]
    fn deck_has_52_unique_cards() {
        let h = some_hand();
        for n in 0..1000u32 {
            let mut d = shuffle_deck(&seed_from(n), &h, SHUFFLE_VERSION).unwrap();
            assert_eq!(d.len(), 52);
            d.sort();
            assert_eq!(d, (0..52).collect::<Vec<Card>>());
        }
    }

    #[test]
    fn same_inputs_same_deck() {
        let h = some_hand();
        let a = shuffle_deck(&[9; 32], &h, 1).unwrap();
        let b = shuffle_deck(&[9; 32], &h.clone(), 1).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, (0..52).collect::<Vec<Card>>());
    }

    #[test]
    fn different_seed_or_hand_changes_deck() {
        let h = some_hand();
        let base = shuffle_deck(&[9; 32], &h, 1).unwrap();
        assert_ne!(base, shuffle_deck(&[8; 32], &h, 1).unwrap());
        let mut h2 = h.clone();
        h2.table_id += 1;
        assert_ne!(base, shuffle_deck(&[9; 32], &h2, 1).unwrap());
        let mut h3 = h.clone();
        h3.hand_no += 1;
        assert_ne!(base, shuffle_deck(&[9; 32], &h3, 1).unwrap());
        let mut h4 = h.clone();
        h4.canister = Principal::anonymous();
        assert_ne!(base, shuffle_deck(&[9; 32], &h4, 1).unwrap());
    }

    #[test]
    fn uniform_rejects_the_biased_zone() {
        // 2^32 mod 52 = 48, so limit = 2^32 - 48 and u32::MAX must be rejected.
        let words = [u32::MAX, 5u32];
        let mut used = 0;
        let mut next = || {
            let w = words[used];
            used += 1;
            w
        };
        assert_eq!(uniform(52, &mut next), 5);
        assert_eq!(used, 2);

        // The last accepted value for n = 52 is limit - 1; limit itself is rejected.
        let limit = (1u64 << 32) - ((1u64 << 32) % 52);
        let words = [limit as u32, (limit - 1) as u32];
        let mut used = 0;
        let mut next = || {
            let w = words[used];
            used += 1;
            w
        };
        assert_eq!(uniform(52, &mut next), ((limit - 1) % 52) as u32);
        assert_eq!(used, 2);

        // For a power of two nothing is rejected.
        let mut next = || u32::MAX;
        assert_eq!(uniform(4, &mut next), 3);
    }

    #[test]
    fn stream_words_are_big_endian_hmac_blocks() {
        use hmac::{Hmac, Mac};
        let key = [3u8; 32];
        let mut s = Stream::new(key);
        let mut expect = Vec::new();
        for i in 0u64..2 {
            let mut m = Hmac::<sha2::Sha256>::new_from_slice(&key).unwrap();
            m.update(&i.to_be_bytes());
            expect.extend_from_slice(&m.finalize().into_bytes());
        }
        for k in 0..16 {
            let w = u32::from_be_bytes(expect[k * 4..k * 4 + 4].try_into().unwrap());
            assert_eq!(s.next_u32(), w, "word {k}");
        }
    }

    #[test]
    fn commitment_changes_with_any_field() {
        let h = some_hand();
        let seed = [1u8; 32];
        let c = commitment(&seed, &h, 1);
        assert!(verify_commitment(&seed, &h, 1, &c));
        let mut s2 = seed;
        s2[31] ^= 1;
        assert_ne!(c, commitment(&s2, &h, 1));
        assert!(!verify_commitment(&s2, &h, 1, &c));
        let mut h2 = h.clone();
        h2.table_id ^= 1;
        assert_ne!(c, commitment(&seed, &h2, 1));
        let mut h3 = h.clone();
        h3.hand_no ^= 1;
        assert_ne!(c, commitment(&seed, &h3, 1));
        let mut h4 = h.clone();
        h4.canister = Principal::management_canister();
        assert_ne!(c, commitment(&seed, &h4, 1));
        assert_ne!(c, commitment(&seed, &h, 2));
        assert!(!verify_commitment(&seed, &h, 2, &commitment(&seed, &h, 2)));
        let mut bad = c;
        bad[0] ^= 0x80;
        assert!(!verify_commitment(&seed, &h, 1, &bad));
    }

    #[test]
    fn encoding_is_length_prefixed() {
        assert_eq!(lp(b"abc"), vec![0, 0, 0, 3, b'a', b'b', b'c']);
        assert_eq!(lp(b""), vec![0, 0, 0, 0]);
        // Moving a byte between adjacent fields must change the encoding.
        let a = [lp(b"ab"), lp(b"c")].concat();
        let b = [lp(b"a"), lp(b"bc")].concat();
        assert_ne!(a, b);

        let h = hand(Principal::from_slice(&[1, 2, 3]), 0x0102030405060708, u64::MAX);
        assert_eq!(
            h.bytes(),
            [
                &[0, 0, 0, 3, 1, 2, 3][..],
                &[1, 2, 3, 4, 5, 6, 7, 8],
                &[0xFF; 8],
            ]
            .concat()
        );
        // Principals whose bytes run into the table id must not collide.
        let x = hand(Principal::from_slice(&[1, 0]), 0, 0).bytes();
        let y = hand(Principal::from_slice(&[1]), 0, 0).bytes();
        assert_ne!(x, y);
        assert_eq!(hand(Principal::management_canister(), 0, 0).bytes(), vec![0u8; 20]);

        let p = params_bytes(1);
        let mut want = vec![0, 1, 52];
        want.extend(lp(b"card=rank*4+suit;rank0=2..rank12=A;suit0=c,1=d,2=h,3=s"));
        want.extend(lp(b"deal=pop-from-end;burn-before-flop-turn-river"));
        assert_eq!(p, want);
    }

    #[test]
    fn unknown_version_is_an_error() {
        let h = some_hand();
        assert!(shuffle_deck(&[0; 32], &h, 0).is_err());
        assert!(shuffle_deck(&[0; 32], &h, 2).is_err());
        let e = shuffle_deck(&[0xAB; 32], &h, 2).unwrap_err();
        assert!(!e.to_lowercase().contains("abab"), "error must not leak the seed");
    }

    #[test]
    fn vectors_file_matches() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../poker_frontend/src/fair/vectors.json");
        let want = vectors_json();
        if std::env::var("UPDATE_FAIR_VECTORS").as_deref() == Ok("1") {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &want).unwrap();
        }
        let got = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!(
                "{} is missing; run `UPDATE_FAIR_VECTORS=1 cargo test vectors_file_matches`",
                path.display()
            )
        });
        assert!(
            got.replace("\r\n", "\n") == want,
            "vectors.json is stale; run `UPDATE_FAIR_VECTORS=1 cargo test vectors_file_matches`"
        );
    }
}

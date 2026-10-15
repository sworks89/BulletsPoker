//! Cards, deck shuffling and 7-card hand evaluation.
//!
//! A card is a `u8` in `0..52`: `rank = card / 4` (0 = deuce .. 12 = ace) and
//! `suit = card % 4` (0 = clubs, 1 = diamonds, 2 = hearts, 3 = spades).

use sha2::{Digest, Sha256};

pub type Card = u8;

pub fn rank(c: Card) -> u8 {
    c / 4
}

pub fn suit(c: Card) -> u8 {
    c % 4
}

pub fn card_name(c: Card) -> String {
    const RANKS: &[u8] = b"23456789TJQKA";
    const SUITS: &[u8] = b"cdhs";
    format!(
        "{}{}",
        RANKS[rank(c) as usize] as char,
        SUITS[suit(c) as usize] as char
    )
}

/// Deterministic stream of random numbers derived from a 32-byte seed
/// (the seed itself comes from the management canister's `raw_rand`).
pub struct SeededRng {
    seed: [u8; 32],
    counter: u64,
    buf: Vec<u8>,
}

impl SeededRng {
    pub fn new(seed: [u8; 32]) -> Self {
        Self {
            seed,
            counter: 0,
            buf: Vec::new(),
        }
    }

    fn next_u64(&mut self) -> u64 {
        if self.buf.len() < 8 {
            let mut h = Sha256::new();
            h.update(self.seed);
            h.update(self.counter.to_le_bytes());
            self.counter += 1;
            self.buf.extend_from_slice(&h.finalize());
        }
        let bytes: Vec<u8> = self.buf.drain(..8).collect();
        u64::from_le_bytes(bytes.try_into().unwrap())
    }

    /// Uniform integer in `0..n` using rejection sampling (no modulo bias).
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0);
        let zone = u64::MAX - (u64::MAX % n);
        loop {
            let v = self.next_u64();
            if v < zone {
                return v % n;
            }
        }
    }
}

/// Returns a freshly shuffled 52-card deck (Fisher-Yates).
pub fn shuffled_deck(seed: [u8; 32]) -> Vec<Card> {
    let mut rng = SeededRng::new(seed);
    let mut deck: Vec<Card> = (0..52).collect();
    for i in (1..deck.len()).rev() {
        let j = rng.below(i as u64 + 1) as usize;
        deck.swap(i, j);
    }
    deck
}

pub const HAND_CATEGORIES: [&str; 9] = [
    "High Card",
    "Pair",
    "Two Pair",
    "Three of a Kind",
    "Straight",
    "Flush",
    "Full House",
    "Four of a Kind",
    "Straight Flush",
];

/// Score of a 5-card hand. Higher is better; equal scores split the pot.
/// Layout: `category << 20 | r0 << 16 | r1 << 12 | r2 << 8 | r3 << 4 | r4`.
pub type HandScore = u32;

pub fn category(score: HandScore) -> &'static str {
    HAND_CATEGORIES[(score >> 20) as usize]
}

fn pack(cat: u32, ranks: &[u8]) -> HandScore {
    let mut s = cat << 20;
    for (i, r) in ranks.iter().take(5).enumerate() {
        s |= (*r as u32) << (16 - 4 * i);
    }
    s
}

pub fn eval5(cards: &[Card; 5]) -> HandScore {
    let mut counts = [0u8; 13];
    for &c in cards {
        counts[rank(c) as usize] += 1;
    }
    let flush = cards.iter().all(|&c| suit(c) == suit(cards[0]));

    // Distinct ranks, descending.
    let distinct: Vec<u8> = (0..13u8).rev().filter(|&r| counts[r as usize] > 0).collect();
    let straight_high = if distinct.len() == 5 {
        if distinct[0] - distinct[4] == 4 {
            Some(distinct[0])
        } else if distinct == [12, 3, 2, 1, 0] {
            Some(3) // wheel: A-2-3-4-5, five-high
        } else {
            None
        }
    } else {
        None
    };

    if let Some(high) = straight_high {
        return pack(if flush { 8 } else { 4 }, &[high]);
    }
    if flush {
        return pack(5, &distinct);
    }

    // Order ranks by (count desc, rank desc).
    let mut groups: Vec<(u8, u8)> = distinct.iter().map(|&r| (counts[r as usize], r)).collect();
    groups.sort_by(|a, b| b.cmp(a));
    let ordered: Vec<u8> = groups.iter().map(|g| g.1).collect();
    let cat = match (groups[0].0, groups.get(1).map(|g| g.0)) {
        (4, _) => 7,
        (3, Some(2)) => 6,
        (3, _) => 3,
        (2, Some(2)) => 2,
        (2, _) => 1,
        _ => 0,
    };
    pack(cat, &ordered)
}

/// Best 5-card score out of 5 to 7 cards, plus the 5 cards that make it.
pub fn best_hand(cards: &[Card]) -> (HandScore, [Card; 5]) {
    assert!(cards.len() >= 5 && cards.len() <= 7);
    let n = cards.len();
    let mut best: Option<(HandScore, [Card; 5])> = None;
    for a in 0..n {
        for b in a + 1..n {
            for c in b + 1..n {
                for d in c + 1..n {
                    for e in d + 1..n {
                        let hand = [cards[a], cards[b], cards[c], cards[d], cards[e]];
                        let s = eval5(&hand);
                        if best.is_none_or(|(bs, _)| s > bs) {
                            best = Some((s, hand));
                        }
                    }
                }
            }
        }
    }
    best.unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Vec<Card> {
        s.split_whitespace()
            .map(|t| {
                let b = t.as_bytes();
                let r = b"23456789TJQKA".iter().position(|&x| x == b[0]).unwrap() as u8;
                let su = b"cdhs".iter().position(|&x| x == b[1]).unwrap() as u8;
                r * 4 + su
            })
            .collect()
    }

    fn score(s: &str) -> HandScore {
        best_hand(&parse(s)).0
    }

    #[test]
    fn deck_is_a_permutation() {
        let mut d = shuffled_deck([7; 32]);
        assert_ne!(d, (0..52).collect::<Vec<_>>());
        d.sort();
        assert_eq!(d, (0..52).collect::<Vec<_>>());
        assert_ne!(shuffled_deck([1; 32]), shuffled_deck([2; 32]));
    }

    #[test]
    fn categories() {
        assert_eq!(category(score("As Ks Qs Js Ts 2c 3d")), "Straight Flush");
        assert_eq!(category(score("As Ad Ac Ah 2s 3c 4d")), "Four of a Kind");
        assert_eq!(category(score("As Ad Ac Kh Ks 3c 4d")), "Full House");
        assert_eq!(category(score("2s 7s 9s Js Ks 3c 4d")), "Flush");
        assert_eq!(category(score("As 2d 3c 4h 5s Kc Kd")), "Straight");
        assert_eq!(category(score("7s 7d 7c Kh 2s 3c 9d")), "Three of a Kind");
        assert_eq!(category(score("7s 7d Kc Kh 2s 3c 9d")), "Two Pair");
        assert_eq!(category(score("7s 7d Ac Jh 2s 3c 9d")), "Pair");
        assert_eq!(category(score("7s 8d Ac Jh 2s 3c 9d")), "High Card");
    }

    #[test]
    fn ordering_and_kickers() {
        // Wheel loses to six-high straight.
        assert!(score("As 2d 3c 4h 5s") < score("2d 3c 4h 5s 6d"));
        // Kicker decides.
        assert!(score("Ks Kd Ac 7h 2s") > score("Ks Kd Qc 7h 2s"));
        // Two pair: higher second pair wins.
        assert!(score("Ks Kd 9c 9h 2s") > score("Ks Kd 8c 8h As"));
        // Full house compares trips first.
        assert!(score("3s 3d 3c 2h 2s") > score("2s 2d 2c Ah As"));
        // Board plays: split.
        assert_eq!(score("2c 3d As Ks Qs Js Ts"), score("4c 5d As Ks Qs Js Ts"));
        // Flush beats straight.
        assert!(score("2s 7s 9s Js Ks") > score("Ts Jd Qc Kh As"));
    }
}

//! Sign-up proof-of-work. Principals cost nothing to create, so each new
//! account must find a nonce whose hash has enough leading zero bits. The work
//! is a fraction of a second normally and grows as sign-ups exceed the hourly
//! target, so a bot wave makes itself expensive instead of locking real
//! players out.

use crate::state::{State, R};
use candid::{CandidType, Deserialize, Principal};
use sha2::{Digest, Sha256};

pub const DEFAULT_POW_BITS: u8 = 12;
/// Hardest it gets: a few seconds in a browser.
pub const MAX_POW_BITS: u8 = 22;
const DOMAIN: &[u8] = b"bp-signup";
const HOUR_NS: u64 = 60 * 60 * 1_000_000_000;

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub struct SignupChallenge {
    /// Required leading zero bits of sha256("bp-signup" ‖ principal ‖ nonce as u64 LE).
    pub difficulty: u8,
}

/// Leading zero bits of sha256(DOMAIN ‖ principal ‖ nonce).
pub fn pow_bits(p: &Principal, nonce: u64) -> u32 {
    let mut h = Sha256::new();
    h.update(DOMAIN);
    h.update(p.as_slice());
    h.update(nonce.to_le_bytes());
    let d = h.finalize();
    let mut bits = 0;
    for b in d.iter() {
        if *b == 0 {
            bits += 8;
        } else {
            bits += b.leading_zeros();
            break;
        }
    }
    bits
}

/// Finds a nonce for `difficulty` (tests and tooling; browsers do the same in JS).
pub fn solve(p: &Principal, difficulty: u8) -> u64 {
    (0..).find(|n| pow_bits(p, *n) >= difficulty as u32).unwrap()
}

impl State {
    /// Work required for the next sign-up: the base, plus 2 bits once the
    /// hourly target is reached and 2 more for every further quarter of it.
    pub fn signup_difficulty(&self, now: u64) -> u8 {
        let count = if self.signup_hour == now / HOUR_NS { self.signup_count } else { 0 };
        let target = self.signups_per_hour.max(1);
        let extra = if count < target { 0 } else { 2 + 2 * ((count - target) / (target / 4).max(1)) };
        (self.signup_pow_bits as u32 + extra).min(MAX_POW_BITS as u32) as u8
    }

    pub fn check_signup_pow(&self, p: &Principal, nonce: u64, now: u64) -> R<()> {
        if pow_bits(p, nonce) < self.signup_difficulty(now) as u32 {
            return Err("Sign-up check failed (lots of people are signing up). Please try again.".into());
        }
        Ok(())
    }

    pub fn set_signup_pow_bits(&mut self, bits: u8) -> R<()> {
        if bits > MAX_POW_BITS {
            return Err(format!("At most {MAX_POW_BITS} bits"));
        }
        self.signup_pow_bits = bits;
        Ok(())
    }

    pub fn signup_pow_bits(&self) -> u8 {
        self.signup_pow_bits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_of_work_round_trip() {
        let s = State::new(Principal::anonymous(), 10_000);
        let p = Principal::from_slice(&[7; 29]);
        let d = s.signup_difficulty(0);
        assert_eq!(d, DEFAULT_POW_BITS);
        let nonce = solve(&p, d);
        s.check_signup_pow(&p, nonce, 0).unwrap();
        // Nonces that don't meet the difficulty are refused.
        let bad = (0..).find(|n| pow_bits(&p, *n) < d as u32).unwrap();
        assert!(s.check_signup_pow(&p, bad, 0).is_err());
    }
}

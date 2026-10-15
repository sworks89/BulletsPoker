//! Per-caller token buckets for update calls.

use candid::Principal;
use std::collections::BTreeMap;

/// Burst size: calls a player can make back to back.
pub const CAPACITY: f64 = 20.0;
/// Sustained calls per second.
pub const REFILL_PER_SEC: f64 = 5.0;
/// Buckets untouched for this long are full again and can be dropped.
const IDLE_NS: u64 = 10 * 60 * 1_000_000_000;

#[derive(Default)]
pub struct Limiter {
    buckets: BTreeMap<Principal, Bucket>,
    /// Calls refused since the last upgrade.
    pub rejected: u64,
}

struct Bucket {
    tokens: f64,
    last: u64,
}

impl Bucket {
    fn level(&self, now: u64) -> f64 {
        let secs = now.saturating_sub(self.last) as f64 / 1e9;
        (self.tokens + secs * REFILL_PER_SEC).min(CAPACITY)
    }
}

impl Limiter {
    /// Spends one token; false (and counted) when the caller is out of budget.
    pub fn take(&mut self, p: Principal, now: u64) -> bool {
        let b = self.buckets.entry(p).or_insert(Bucket { tokens: CAPACITY, last: now });
        let level = b.level(now);
        b.last = now;
        if level < 1.0 {
            b.tokens = level;
            self.rejected += 1;
            return false;
        }
        b.tokens = level - 1.0;
        true
    }

    /// Would `take` succeed? Doesn't spend anything (for `inspect_message`).
    pub fn has_budget(&self, p: &Principal, now: u64) -> bool {
        self.buckets.get(p).is_none_or(|b| b.level(now) >= 1.0)
    }

    pub fn prune(&mut self, now: u64) {
        self.buckets.retain(|_, b| now.saturating_sub(b.last) < IDLE_NS);
    }

    /// Callers who made an update call since `since`.
    pub fn active_since(&self, since: u64) -> usize {
        self.buckets.values().filter(|b| b.last >= since).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bursts_then_refills() {
        let p = Principal::from_slice(&[1]);
        let mut l = Limiter::default();
        for _ in 0..20 {
            assert!(l.take(p, 0));
        }
        assert!(!l.take(p, 0));
        assert!(!l.has_budget(&p, 0));
        assert_eq!(l.rejected, 1);
        // 5 tokens a second.
        assert!(l.has_budget(&p, 200_000_000));
        assert!(l.take(p, 200_000_000));
        assert!(!l.take(p, 200_000_000));
        assert!(l.has_budget(&Principal::from_slice(&[2]), 0), "other callers unaffected");
        assert_eq!(l.active_since(0), 1);
        l.prune(IDLE_NS + 200_000_000);
        assert_eq!(l.active_since(0), 0);
    }
}

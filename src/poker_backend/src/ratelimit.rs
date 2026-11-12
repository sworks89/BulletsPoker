//! Per-caller token buckets for update calls.

use candid::Principal;
use std::collections::BTreeMap;

/// Burst size: calls a player can make back to back.
pub const CAPACITY: f64 = 20.0;
/// Sustained calls per second.
pub const REFILL_PER_SEC: f64 = 5.0;
/// Buckets untouched for this long are full again and can be dropped.
const IDLE_NS: u64 = 10 * 60 * 1_000_000_000;

/// The one bucket shared by callers who aren't players: Internet Identity's
/// alert receipts, whose principals are new each time, so no per-caller
/// budget could hold them. Sized well above a full minute of daily reminders
/// (500) being received and opened at once.
pub const SHARED_CAPACITY: f64 = 2_000.0;
pub const SHARED_REFILL_PER_SEC: f64 = 50.0;

pub struct Limiter {
    buckets: BTreeMap<Principal, Bucket>,
    shared: Bucket,
    /// Calls refused since the last upgrade.
    pub rejected: u64,
}

impl Default for Limiter {
    fn default() -> Self {
        Limiter { buckets: BTreeMap::new(), shared: Bucket { tokens: SHARED_CAPACITY, last: 0 }, rejected: 0 }
    }
}

struct Bucket {
    tokens: f64,
    last: u64,
}

impl Bucket {
    fn level_with(&self, now: u64, capacity: f64, refill: f64) -> f64 {
        let secs = now.saturating_sub(self.last) as f64 / 1e9;
        (self.tokens + secs * refill).min(capacity)
    }

    fn level(&self, now: u64) -> f64 {
        self.level_with(now, CAPACITY, REFILL_PER_SEC)
    }

    /// Spends one token if there is one.
    fn spend(&mut self, now: u64, capacity: f64, refill: f64) -> bool {
        let level = self.level_with(now, capacity, refill);
        self.last = now;
        if level < 1.0 {
            self.tokens = level;
            return false;
        }
        self.tokens = level - 1.0;
        true
    }
}

impl Limiter {
    /// Spends one token; false (and counted) when the caller is out of budget.
    pub fn take(&mut self, p: Principal, now: u64) -> bool {
        let b = self.buckets.entry(p).or_insert(Bucket { tokens: CAPACITY, last: now });
        let ok = b.spend(now, CAPACITY, REFILL_PER_SEC);
        if !ok {
            self.rejected += 1;
        }
        ok
    }

    /// Would `take` succeed? Doesn't spend anything (for `inspect_message`).
    pub fn has_budget(&self, p: &Principal, now: u64) -> bool {
        self.buckets.get(p).is_none_or(|b| b.level(now) >= 1.0)
    }

    /// Spends from the shared bucket; false (and counted) when it is empty.
    pub fn take_shared(&mut self, now: u64) -> bool {
        let ok = self.shared.spend(now, SHARED_CAPACITY, SHARED_REFILL_PER_SEC);
        if !ok {
            self.rejected += 1;
        }
        ok
    }

    /// Would `take_shared` succeed? Doesn't spend anything (for `inspect_message`).
    pub fn has_shared_budget(&self, now: u64) -> bool {
        self.shared.level_with(now, SHARED_CAPACITY, SHARED_REFILL_PER_SEC) >= 1.0
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

    /// Calls from principals nobody registered share one budget, so a flood
    /// of them can cost at most so much.
    #[test]
    fn strangers_share_one_bucket() {
        let mut l = Limiter::default();
        for _ in 0..SHARED_CAPACITY as usize {
            assert!(l.take_shared(0));
        }
        assert!(!l.take_shared(0));
        assert!(!l.has_shared_budget(0));
        assert_eq!(l.rejected, 1);
        assert!(l.take_shared(1_000_000_000 / SHARED_REFILL_PER_SEC as u64), "refills");
        assert!(l.take(Principal::from_slice(&[1]), 0), "players' own buckets are untouched");
    }
}

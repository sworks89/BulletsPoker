//! Rewards: the house's rake is split between the Fish Buffet pool, the four
//! leaderboard prize pools and the treasury.
//!
//! Board index order everywhere: 0 weekly points, 1 weekly winnings,
//! 2 monthly points, 3 monthly winnings.

use crate::state::State;
use candid::{CandidType, Deserialize};

/// Share of the house's rake that funds the Fish Buffet cashback pool (30%).
pub const FISH_POOL_BPS: u64 = 3_000;
/// Share of the house's rake that funds the four leaderboard pools together (10%).
pub const BOARD_POOL_BPS: u64 = 1_000;
/// Number of leaderboards.
pub const BOARDS: usize = 4;

#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct RewardsState {
    /// ICP (e8s) backing the cashback wheel.
    pub fish_pool: u64,
    /// ICP (e8s) prize pool per leaderboard.
    pub board_pools: [u64; BOARDS],
}

impl RewardsState {
    pub fn pools_total(&self) -> u64 {
        self.fish_pool + self.board_pools.iter().sum::<u64>()
    }
}

impl State {
    pub(crate) fn rewards(&self) -> RewardsState {
        self.rewards.clone().unwrap_or_default()
    }

    pub(crate) fn rewards_mut(&mut self) -> &mut RewardsState {
        self.rewards.get_or_insert_with(RewardsState::default)
    }

    /// Splits the house's share of the rake: 30% to the Fish pool, 2.5% to
    /// each leaderboard pool, the rest (including rounding) to the treasury.
    pub(crate) fn fund_rewards(&mut self, house_rake: u64) {
        let fish = house_rake * FISH_POOL_BPS / 10_000;
        let board = house_rake * (BOARD_POOL_BPS / BOARDS as u64) / 10_000;
        let r = self.rewards_mut();
        r.fish_pool += fish;
        for p in r.board_pools.iter_mut() {
            *p += board;
        }
        self.treasury += house_rake - fish - board * BOARDS as u64;
    }
}

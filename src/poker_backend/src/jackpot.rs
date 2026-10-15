//! Bad-beat jackpot for cash tables.
//!
//! Every cash pot that sees a flop drops a small amount (see
//! `TableConfig::jackpot_bps`) into a jackpot for the table's currency. When
//! four of a kind or better loses at showdown, with both of the loser's hole
//! cards playing and at least `MIN_PLAYERS` dealt in, most of the jackpot is
//! paid out to the players in that hand. Tournament tables never take part.

use crate::cards::{best_hand, category, eval5, Card, HandScore};
use crate::state::{fmt_amount, Channel, MessageKind, State, R};
use crate::table::{Currency, HandResult};
use candid::{CandidType, Deserialize, Principal};

/// Losing hand category that qualifies (7 = four of a kind).
pub const QUALIFYING_CATEGORY: u32 = 7;
pub const MIN_PLAYERS: usize = 4;
/// Share of the jackpot paid out on a hit; the rest seeds the next one.
pub const PAYOUT_BPS: u64 = 9_000;
/// Split of the payout: loser, winner, everyone else dealt in.
pub const LOSER_BPS: u64 = 5_000;
pub const WINNER_BPS: u64 = 2_500;
/// Play-chip jackpot starts with house-minted chips so it's worth chasing from day one.
pub const CHIP_JACKPOT_SEED: u64 = 100_000;
const RECENT_HITS: usize = 20;

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct JackpotPayout {
    pub player: Principal,
    pub username: String,
    pub amount: u64,
    /// "loser", "winner" or "table".
    pub share: String,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct JackpotHit {
    pub time: u64,
    pub table_id: u64,
    pub table_name: String,
    pub hand_no: u64,
    pub currency: Currency,
    pub total: u64,
    pub board: Vec<Card>,
    pub loser: Principal,
    pub loser_name: String,
    pub loser_hand: String,
    pub loser_cards: Vec<Card>,
    pub winner: Principal,
    pub winner_name: String,
    pub winner_hand: String,
    pub winner_cards: Vec<Card>,
    pub payouts: Vec<JackpotPayout>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct JackpotInfo {
    pub icp: u64,
    pub chips: u64,
    pub drop_bps: u16,
    pub min_players: u8,
    pub qualifying_hand: String,
    pub payout_bps: u64,
    pub loser_bps: u64,
    pub winner_bps: u64,
    pub recent: Vec<JackpotHit>,
}

/// Best score of a hand that uses both hole cards plus three board cards.
fn best_using_both(hole: &[Card], board: &[Card]) -> Option<HandScore> {
    if hole.len() != 2 || board.len() != 5 {
        return None;
    }
    let mut best = None;
    for a in 0..5 {
        for b in a + 1..5 {
            for c in b + 1..5 {
                let s = eval5(&[hole[0], hole[1], board[a], board[b], board[c]]);
                best = best.max(Some(s));
            }
        }
    }
    best
}

/// Whether a showdown hand qualifies as the losing hand of a bad beat:
/// four of a kind or better, with both hole cards playing.
pub fn qualifies(hole: &[Card], board: &[Card]) -> Option<HandScore> {
    let mut cards = hole.to_vec();
    cards.extend_from_slice(board);
    let (score, _) = best_hand(&cards);
    if score >> 20 < QUALIFYING_CATEGORY {
        return None;
    }
    (best_using_both(hole, board) == Some(score)).then_some(score)
}

impl State {
    pub fn jackpot(&self, currency: Currency) -> u64 {
        self.jackpots.get(&currency).copied().unwrap_or(0)
    }

    /// Checks a finished cash-table hand for a bad beat and pays the jackpot.
    /// `result` must be the unmucked showdown (before auto-muck hides losers).
    pub fn check_bad_beat(&mut self, table_id: u64, result: &HandResult, now: u64) {
        let Some(t) = self.tables.get(&table_id) else { return };
        if t.config.jackpot_bps == 0 || result.board.len() != 5 || result.players.len() < MIN_PLAYERS {
            return;
        }
        let (currency, table_name, hand_no) = (t.config.currency, t.config.name.clone(), result.hand_no);
        let scored: Vec<(HandScore, &crate::table::ShownHand)> = result
            .shown
            .iter()
            .map(|sh| {
                let mut cards = sh.cards.clone();
                cards.extend_from_slice(&result.board);
                (best_hand(&cards).0, sh)
            })
            .collect();
        let Some(&(top, winner)) = scored.iter().max_by_key(|(s, _)| *s) else { return };
        // The best qualifying hand that still lost.
        let Some((loser_score, loser)) = result
            .shown
            .iter()
            .filter_map(|sh| qualifies(&sh.cards, &result.board).map(|s| (s, sh)))
            .filter(|(s, _)| *s < top)
            .max_by_key(|(s, _)| *s)
        else {
            return;
        };

        let pool = self.jackpot(currency);
        let total = pool as u128 * PAYOUT_BPS as u128 / 10_000;
        let total = total as u64;
        if total == 0 {
            return;
        }
        let name = |s: &State, p: &Principal| s.users.get(p).map(|u| u.username.clone()).unwrap_or_default();
        let others: Vec<Principal> = result
            .players
            .iter()
            .map(|r| r.player)
            .filter(|p| *p != loser.player && *p != winner.player)
            .collect();
        let winner_amt = total * WINNER_BPS / 10_000;
        let table_amt = total - total * LOSER_BPS / 10_000 - winner_amt;
        let each = if others.is_empty() { 0 } else { table_amt / others.len() as u64 };
        // Rounding dust goes to the loser.
        let loser_amt = total - winner_amt - each * others.len() as u64;

        let mut payouts = vec![
            JackpotPayout { player: loser.player, username: name(self, &loser.player), amount: loser_amt, share: "loser".into() },
            JackpotPayout { player: winner.player, username: name(self, &winner.player), amount: winner_amt, share: "winner".into() },
        ];
        for p in &others {
            payouts.push(JackpotPayout { player: *p, username: name(self, p), amount: each, share: "table".into() });
        }
        for po in &payouts {
            let _ = self.credit_in(&po.player, currency, po.amount);
        }
        *self.jackpots.entry(currency).or_default() -= total;

        let hit = JackpotHit {
            time: now,
            table_id,
            table_name: table_name.clone(),
            hand_no,
            currency,
            total,
            board: result.board.clone(),
            loser: loser.player,
            loser_name: name(self, &loser.player),
            loser_hand: category(loser_score).to_string(),
            loser_cards: loser.cards.clone(),
            winner: winner.player,
            winner_name: name(self, &winner.player),
            winner_hand: category(top).to_string(),
            winner_cards: winner.cards.clone(),
            payouts,
        };
        let text = format!(
            "💥 BAD BEAT JACKPOT at {}! {}'s {} lost to {}'s {}. {} paid out to the table!",
            table_name,
            hit.loser_name,
            hit.loser_hand,
            hit.winner_name,
            hit.winner_hand,
            fmt_amount(currency, total)
        );
        self.post(Channel::Lobby, None, String::new(), text.clone(), MessageKind::System, now);
        self.post(Channel::Table(table_id), None, String::new(), text, MessageKind::System, now);
        for p in [hit.loser, hit.winner] {
            self.award(&p, "jackpot", Some(Channel::Table(table_id)), now);
        }
        self.table_jackpot_hit.insert(table_id, hit.clone());
        self.jackpot_hits.push_front(hit);
        self.jackpot_hits.truncate(RECENT_HITS);
    }

    /// The jackpot hit to celebrate at a table: only while that hand is on screen.
    pub fn current_hit(&self, table_id: u64) -> Option<JackpotHit> {
        let t = self.tables.get(&table_id)?;
        self.table_jackpot_hit.get(&table_id).filter(|h| h.hand_no == t.hand_no).cloned()
    }

    pub fn jackpot_info(&self) -> JackpotInfo {
        JackpotInfo {
            icp: self.jackpot(Currency::Icp),
            chips: self.jackpot(Currency::Chips),
            drop_bps: crate::state::JACKPOT_DROP_BPS,
            min_players: MIN_PLAYERS as u8,
            qualifying_hand: "four of a kind or better (both hole cards must play)".into(),
            payout_bps: PAYOUT_BPS,
            loser_bps: LOSER_BPS,
            winner_bps: WINNER_BPS,
            recent: self.jackpot_hits.iter().cloned().collect(),
        }
    }

    /// Moves house money into the ICP jackpot to make it bigger.
    pub fn seed_jackpot(&mut self, amount: u64) -> R<()> {
        if self.treasury < amount {
            return Err("Treasury too small".into());
        }
        self.treasury -= amount;
        *self.jackpots.entry(Currency::Icp).or_default() += amount;
        Ok(())
    }
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

    #[test]
    fn qualifying_hands() {
        // Pocket pair making quads with a paired board: both cards play.
        assert!(qualifies(&parse("7c 7d"), &parse("7h 7s Kd 2c 3d")).is_some());
        // Quads on the board: hole cards don't both play.
        assert!(qualifies(&parse("Ac Ad"), &parse("7c 7d 7h 7s Kd")).is_none());
        // Trips on board plus one hole card: only one plays.
        assert!(qualifies(&parse("7c 2d"), &parse("7h 7s 7d Kd 3c")).is_none());
        // Full house doesn't qualify.
        assert!(qualifies(&parse("Kc Kd"), &parse("Kh 7s 7d 2c 3d")).is_none());
        // Straight flush with both hole cards.
        assert!(qualifies(&parse("9h Th"), &parse("Jh Qh Kh 2c 3d")).is_some());
    }
}

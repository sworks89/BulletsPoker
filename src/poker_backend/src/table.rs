//! No-limit Texas Hold'em table state machine.
//!
//! The table is pure game logic: it knows nothing about wallets, ledgers or
//! timers. All amounts are in chips, where 1 chip = 1 e8s of ICP. The
//! canister moves chips between a player's wallet balance and their stack on
//! `sit`/`stand`/`top_up`, and sweeps `rake_accrued` into the house treasury.

use crate::cards::{best_hand, category, shuffle_legacy, Card, HandScore};
use candid::{CandidType, Deserialize, Principal};

/// What a table plays for: real ICP (1 chip = 1 e8s) or free play chips.
#[derive(CandidType, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Currency {
    Icp,
    Chips,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TableConfig {
    pub name: String,
    pub currency: Currency,
    pub small_blind: u64,
    pub big_blind: u64,
    pub min_buy_in: u64,
    pub max_buy_in: u64,
    pub max_seats: u8,
    /// Rake in basis points of the pot (250 = 2.5%). Only taken when a flop is dealt.
    pub rake_bps: u16,
    pub rake_cap: u64,
    /// Bad-beat jackpot drop in basis points of the pot after rake (flop only). 0 = no jackpot.
    pub jackpot_bps: u16,
    pub jackpot_cap: u64,
    pub action_timeout_secs: u64,
}

impl TableConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.name.len() > 40 {
            return Err("Table name must be 1-40 characters".into());
        }
        if self.small_blind == 0 || self.big_blind < self.small_blind {
            return Err("Invalid blinds".into());
        }
        if self.min_buy_in < self.big_blind * 10 || self.max_buy_in < self.min_buy_in {
            return Err("Buy-in range must be at least 10 big blinds".into());
        }
        if !(2..=10).contains(&self.max_seats) {
            return Err("Tables seat 2 to 10 players".into());
        }
        if self.rake_bps > 1000 {
            return Err("Rake may not exceed 10%".into());
        }
        if self.jackpot_bps > 300 {
            return Err("The jackpot drop may not exceed 3%".into());
        }
        if !(5..=120).contains(&self.action_timeout_secs) {
            return Err("Action timeout must be 5-120 seconds".into());
        }
        Ok(())
    }
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub enum Action {
    Fold,
    Check,
    Call,
    /// Bet or raise so that the player's total bet on this street is the given amount.
    Raise(u64),
    AllIn,
}

#[derive(CandidType, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Waiting,
    PreFlop,
    Flop,
    Turn,
    River,
    HandOver,
}

impl Phase {
    pub fn is_betting(self) -> bool {
        matches!(self, Phase::PreFlop | Phase::Flop | Phase::Turn | Phase::River)
    }
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Seat {
    pub player: Principal,
    pub stack: u64,
    pub hole: Vec<Card>,
    pub in_hand: bool,
    pub folded: bool,
    pub all_in: bool,
    /// Chips put in on the current street.
    pub bet: u64,
    /// Chips put in during the whole hand.
    pub committed: u64,
    pub acted: bool,
    pub sitting_out: bool,
    /// When the player started sitting out (cash tables stand them up after a while).
    pub sitting_out_since: Option<u64>,
    /// Player stood up mid-hand; their stack was already paid out.
    pub left: bool,
    pub last_action: Option<String>,
    /// Extra thinking time (ns), used automatically once the action clock runs
    /// out. Refills a little with every hand dealt.
    pub time_bank: u64,
    /// The bet level this player faced when they last acted on this street.
    /// They may raise again only once it has gone up by a full raise since
    /// (a short all-in alone doesn't reopen the betting).
    pub last_faced: Option<u64>,
    /// Missed hands while sitting out, or sat down at a running game: posts a
    /// big blind the next time they're dealt in (unless in the blinds anyway),
    /// so sitting out through the blinds or seat-hopping saves nothing.
    pub owes_bb: bool,
    /// Asked to leave during a live hand: sitting out until the hand ends, then
    /// stood up with whatever they have (including anything they win).
    pub leaving: Option<bool>,
}

pub const LEAVING_MSG: &str = "You're leaving this table when the hand ends";

/// What standing up did: left at once with these chips, or leaves when the hand ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandOutcome {
    Left(u64),
    AfterHand,
}

impl Seat {
    fn new(player: Principal, stack: u64) -> Self {
        Seat {
            player,
            stack,
            hole: vec![],
            in_hand: false,
            folded: false,
            all_in: false,
            bet: 0,
            committed: 0,
            acted: false,
            sitting_out: false,
            sitting_out_since: None,
            left: false,
            last_action: None,
            time_bank: TIME_BANK_START,
            last_faced: None,
            owes_bb: false,
            leaving: None,
        }
    }

    fn live(&self) -> bool {
        self.in_hand && !self.folded
    }

    pub fn is_leaving(&self) -> bool {
        self.leaving == Some(true)
    }

    fn can_act(&self) -> bool {
        self.live() && !self.all_in
    }

    /// Moves up to `amount` from the stack into the pot; returns what was moved.
    fn put(&mut self, amount: u64) -> u64 {
        let a = amount.min(self.stack);
        self.stack -= a;
        self.bet += a;
        self.committed += a;
        if self.stack == 0 {
            self.all_in = true;
        }
        a
    }
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Winner {
    pub seat: u8,
    pub player: Principal,
    pub amount: u64,
    pub hand: String,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct ShownHand {
    pub seat: u8,
    pub player: Principal,
    pub cards: Vec<Card>,
    pub hand: String,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct PlayerResult {
    pub player: Principal,
    pub committed: u64,
    pub won: u64,
}

/// One line of a hand's action log, used for hand histories and replays.
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct LogEntry {
    pub phase: Phase,
    pub seat: Option<u8>,
    pub text: String,
    /// The player's total bet on this street after the action, or the amount won/returned.
    pub amount: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct HandResult {
    pub hand_no: u64,
    pub board: Vec<Card>,
    pub winners: Vec<Winner>,
    pub shown: Vec<ShownHand>,
    pub players: Vec<PlayerResult>,
    pub pot: u64,
    pub rake: u64,
    /// Taken from the pot for the bad-beat jackpot.
    pub jackpot_drop: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Table {
    pub id: u64,
    pub config: TableConfig,
    pub seats: Vec<Option<Seat>>,
    pub button: usize,
    pub phase: Phase,
    pub deck: Vec<Card>,
    pub board: Vec<Card>,
    pub to_act: Option<usize>,
    pub current_bet: u64,
    /// Minimum raise increment on this street.
    pub min_raise: u64,
    pub hand_no: u64,
    pub action_deadline: u64,
    pub phase_changed_at: u64,
    pub last_result: Option<HandResult>,
    /// Rake collected but not yet swept into the house treasury.
    pub rake_accrued: u64,
    /// Jackpot drop collected but not yet swept into the jackpot.
    pub jackpot_accrued: u64,
    /// Action log of the current (or just finished) hand.
    pub log: Vec<LogEntry>,
    /// Tournament tables deal in players who are sitting out (they post blinds
    /// and are folded immediately), so absent players blind off and the event ends.
    pub tournament_mode: bool,
    /// The player to act ran out of regular time and is on their time bank,
    /// which they started drawing on at `bank_started_at`.
    pub time_bank_active: bool,
    pub bank_started_at: u64,
    /// When the regular time of the current turn ends. Fixed when the turn starts, so
    /// sitting out and coming back can't restart it.
    pub turn_deadline: Option<u64>,
    /// Revision of the decision on the table; moves whenever the actor or the amount
    /// to call changes, so a command made for an earlier decision can be refused.
    pub action_seq: Option<u64>,
    /// When the time bank in use ends; fixed when the bank starts.
    pub bank_deadline: Option<u64>,
    /// For a copy opened when its game filled up: the original table's id.
    pub game: Option<u64>,
    /// For a copy: its number in the game (the original is 1).
    pub copy_number: Option<u8>,
    /// For a copy: when it last became empty (it closes after a minute).
    pub empty_since: Option<u64>,
    /// Watchers waiting to be seated automatically when a seat opens, in order.
    pub auto_sit: Option<std::collections::VecDeque<AutoSitEntry>>,
}

/// A watcher waiting for a seat at a table.
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct AutoSitEntry {
    pub player: Principal,
    pub buy_in: u64,
    /// Last time their browser checked in (they're only seated while it keeps doing so).
    pub last_seen: u64,
}

const NANOS: u64 = 1_000_000_000;
/// Time bank on sitting down, and the most it can hold.
pub const TIME_BANK_START: u64 = 30 * NANOS;
pub const TIME_BANK_MAX: u64 = 30 * NANOS;
/// Added to the time bank of everyone dealt into a hand.
pub const TIME_BANK_REFILL: u64 = 2 * NANOS;

impl Table {
    pub fn new(id: u64, config: TableConfig) -> Self {
        let n = config.max_seats as usize;
        Table {
            id,
            config,
            seats: vec![None; n],
            button: n - 1,
            phase: Phase::Waiting,
            deck: vec![],
            board: vec![],
            to_act: None,
            current_bet: 0,
            min_raise: 0,
            hand_no: 0,
            action_deadline: 0,
            phase_changed_at: 0,
            last_result: None,
            rake_accrued: 0,
            jackpot_accrued: 0,
            log: vec![],
            tournament_mode: false,
            time_bank_active: false,
            bank_started_at: 0,
            turn_deadline: None,
            action_seq: None,
            bank_deadline: None,
            game: None,
            copy_number: None,
            empty_since: None,
            auto_sit: None,
        }
    }

    /// Lowest empty seat, if any.
    pub fn first_free_seat(&self) -> Option<u8> {
        self.seats.iter().position(|s| s.is_none()).map(|i| i as u8)
    }

    fn log(&mut self, seat: Option<usize>, text: impl Into<String>, amount: u64) {
        self.log.push(LogEntry {
            phase: self.phase,
            seat: seat.map(|i| i as u8),
            text: text.into(),
            amount,
        });
    }

    fn n(&self) -> usize {
        self.seats.len()
    }

    fn seat(&self, i: usize) -> &Seat {
        self.seats[i].as_ref().expect("seat occupied")
    }

    fn seat_mut(&mut self, i: usize) -> &mut Seat {
        self.seats[i].as_mut().expect("seat occupied")
    }

    pub fn seat_of(&self, p: &Principal) -> Option<usize> {
        self.seats
            .iter()
            .position(|s| s.as_ref().is_some_and(|s| s.player == *p && !s.left))
    }

    pub fn player_count(&self) -> usize {
        self.seats.iter().flatten().filter(|s| !s.left).count()
    }

    /// Chips currently on the table (stacks plus pot); used for accounting checks.
    pub fn chips_in_play(&self) -> u64 {
        self.seats
            .iter()
            .flatten()
            .map(|s| s.stack + if self.phase.is_betting() { s.committed } else { 0 })
            .sum()
    }

    pub fn pot(&self) -> u64 {
        self.seats.iter().flatten().map(|s| s.committed).sum()
    }

    /// Next occupied seat after `from` (exclusive, wrapping) matching `pred`.
    fn next_where(&self, from: usize, pred: impl Fn(&Seat) -> bool) -> Option<usize> {
        let n = self.n();
        (1..=n)
            .map(|k| (from + k) % n)
            .find(|&i| self.seats[i].as_ref().is_some_and(&pred))
    }

    fn live_count(&self) -> usize {
        self.seats.iter().flatten().filter(|s| s.live()).count()
    }

    // ---------------------------------------------------------------- seating

    pub fn sit(&mut self, player: Principal, seat: u8, buy_in: u64) -> Result<(), String> {
        self.sit_with_minimum(player, seat, buy_in, 0)
    }

    /// Like `sit`, but the buy-in must be at least `required` (a player coming
    /// back soon after leaving brings back what they left with, even above the
    /// usual maximum).
    pub fn sit_with_minimum(&mut self, player: Principal, seat: u8, buy_in: u64, required: u64) -> Result<(), String> {
        let i = seat as usize;
        if i >= self.n() {
            return Err("No such seat".into());
        }
        // Includes a seat left mid-hand that is still in play: one seat per player.
        if self.seats.iter().flatten().any(|s| s.player == player && s.is_leaving()) {
            return Err(LEAVING_MSG.into());
        }
        if self.seats.iter().flatten().any(|s| s.player == player) {
            return Err("You are already seated at this table".into());
        }
        if self.seats[i].is_some() {
            return Err("Seat is taken".into());
        }
        let (min, max) = (self.config.min_buy_in.max(required), self.config.max_buy_in.max(required));
        if buy_in < min || buy_in > max {
            return Err("Buy-in outside the table limits".into());
        }
        // Joining a game already in progress: post a big blind to play.
        let playing = self.seats.iter().flatten().filter(|s| s.stack > 0 && !s.sitting_out && !s.left).count();
        let mut seat = Seat::new(player, buy_in);
        seat.owes_bb = !self.tournament_mode && self.hand_no > 0 && playing >= 2;
        self.seats[i] = Some(seat);
        Ok(())
    }

    /// Adds chips to a stack between hands (or after folding).
    /// Seats a player with an exact stack, bypassing buy-in limits (tournament seating
    /// and table balancing).
    pub fn seat_player(&mut self, player: Principal, seat: usize, stack: u64) -> Result<(), String> {
        if self.seats.get(seat).is_none_or(|s| s.is_some()) {
            return Err("Seat unavailable".into());
        }
        self.seats[seat] = Some(Seat::new(player, stack));
        Ok(())
    }

    /// Adds chips to a seated player's stack with no buy-in cap (tournament
    /// rebuys). Not allowed while the player is in a live hand.
    pub fn add_chips(&mut self, player: &Principal, amount: u64) -> Result<(), String> {
        let i = self.seat_of(player).ok_or("You are not seated")?;
        if self.phase.is_betting() && self.seat(i).live() {
            return Err("You can rebuy once this hand is over".into());
        }
        self.seat_mut(i).stack += amount;
        Ok(())
    }

    pub fn free_seats(&self) -> Vec<usize> {
        (0..self.n()).filter(|&i| self.seats[i].is_none()).collect()
    }

    /// True between hands, when players can be moved without affecting a pot.
    pub fn between_hands(&self) -> bool {
        !self.phase.is_betting()
    }

    pub fn top_up(&mut self, player: &Principal, amount: u64) -> Result<(), String> {
        let i = self.seat_of(player).ok_or("You are not seated")?;
        let in_live_hand = self.phase.is_betting() && self.seat(i).live();
        let max_buy_in = self.config.max_buy_in;
        if self.seat(i).is_leaving() {
            return Err(LEAVING_MSG.into());
        }
        let s = self.seat_mut(i);
        if in_live_hand {
            return Err("You can add chips once your hand is over".into());
        }
        if amount == 0 || s.stack + amount > max_buy_in {
            return Err("Stack would exceed the maximum buy-in".into());
        }
        s.stack += amount;
        Ok(())
    }

    pub fn set_sitting_out(&mut self, player: &Principal, out: bool, now: u64) -> Result<(), String> {
        let i = self.seat_of(player).ok_or("You are not seated")?;
        if !out && self.seat(i).is_leaving() {
            return Err(LEAVING_MSG.into());
        }
        if self.seat(i).sitting_out == out {
            return Ok(());
        }
        let s = self.seat_mut(i);
        s.sitting_out = out;
        s.sitting_out_since = if out { s.sitting_out_since.or(Some(now)) } else { None };
        // Their turn right now: sitting out acts at once, coming back resumes the turn's
        // remaining budget. Neither ever gives them more time than they had.
        if self.phase.is_betting() && self.to_act == Some(i) {
            // Away time counts as elapsed: sitting out only acts at once, and coming
            // back restores the deadline the turn (or its time bank) already had.
            self.action_deadline = if out {
                now
            } else if self.time_bank_active {
                self.bank_deadline.unwrap_or(self.action_deadline)
            } else {
                self.turn_deadline.unwrap_or(self.action_deadline)
            };
        }
        Ok(())
    }

    /// Players who have been sitting out since before `cutoff` and aren't in a
    /// live hand (so they can be stood up without affecting a pot).
    pub fn idle_players(&self, cutoff: u64) -> Vec<Principal> {
        self.seats
            .iter()
            .flatten()
            .filter(|s| !s.left && !s.is_leaving() && s.sitting_out && s.sitting_out_since.is_some_and(|t| t <= cutoff))
            .filter(|s| !(self.phase.is_betting() && s.live()))
            .map(|s| s.player)
            .collect()
    }

    /// Stands a player up. Out of a live hand they leave at once with their
    /// stack (a player who already folded leaves their chips in the pot). In a
    /// live hand, all-in included, they are never folded for it: they sit out,
    /// their remaining turns are checked or folded for them, and they leave
    /// with whatever they have when the hand ends (`take_leavers`).
    pub fn stand(&mut self, player: &Principal, now: u64) -> Result<StandOutcome, String> {
        let i = self.seat_of(player).ok_or("You are not seated")?;
        if self.seat(i).is_leaving() {
            return Ok(StandOutcome::AfterHand);
        }
        let in_hand = self.phase.is_betting() && self.seat(i).in_hand;
        if !in_hand {
            let s = self.seats[i].take().unwrap();
            return Ok(StandOutcome::Left(s.stack));
        }
        if self.seat(i).live() {
            // On their turn this sets the clock to now, so the tick acts for them.
            self.set_sitting_out(player, true, now)?;
            self.seat_mut(i).leaving = Some(true);
            self.log(Some(i), "Leaving after this hand", 0);
            return Ok(StandOutcome::AfterHand);
        }
        // Already folded: nothing of theirs is still in play.
        let s = self.seat_mut(i);
        let chips = s.stack;
        s.stack = 0;
        s.left = true;
        s.last_action = Some("Left".into());
        self.log(Some(i), "Left the table", 0);
        Ok(StandOutcome::Left(chips))
    }

    /// Stands up the players who asked to leave during the hand, once it is
    /// over. Returns each one's chips to credit back to their wallet.
    pub fn take_leavers(&mut self) -> Vec<(Principal, u64)> {
        if self.phase.is_betting() {
            return vec![];
        }
        let mut out = vec![];
        for slot in self.seats.iter_mut() {
            if slot.as_ref().is_some_and(|s| s.is_leaving() && !s.left) {
                let s = slot.take().unwrap();
                out.push((s.player, s.stack));
            }
        }
        out
    }

    // ------------------------------------------------------------------ hands

    /// Seats that will be dealt into the next hand.
    pub fn eligible_count(&self) -> usize {
        self.seats
            .iter()
            .flatten()
            .filter(|s| s.stack > 0 && (!s.sitting_out || self.tournament_mode) && !s.left)
            .count()
    }

    /// Removes seats that left or busted. Returns principals that were removed.
    fn cleanup_seats(&mut self) -> Vec<Principal> {
        let mut removed = vec![];
        for slot in self.seats.iter_mut() {
            if slot.as_ref().is_some_and(|s| s.left || s.stack == 0) {
                removed.push(slot.take().unwrap().player);
            }
        }
        removed
    }

    pub fn ready_for_next_hand(&self, now: u64) -> bool {
        match self.phase {
            Phase::Waiting => self.eligible_count() >= 2,
            Phase::HandOver => now >= self.phase_changed_at + 5 * NANOS,
            _ => false,
        }
    }

    /// Ends the post-hand pause without dealing (used when fewer than 2 can play).
    pub fn settle_after_hand(&mut self) -> Vec<Principal> {
        let removed = self.cleanup_seats();
        if self.phase == Phase::HandOver {
            self.phase = Phase::Waiting;
            for s in self.seats.iter_mut().flatten() {
                s.hole.clear();
                s.in_hand = false;
                s.bet = 0;
                s.committed = 0;
                s.folded = false;
                s.all_in = false;
                s.last_action = None;
            }
            self.board.clear();
        }
        removed
    }

    /// Starts a new hand with a shuffled deck derived from `seed`.
    pub fn start_hand(&mut self, seed: [u8; 32], now: u64) -> Result<(), String> {
        if !matches!(self.phase, Phase::Waiting | Phase::HandOver) {
            return Err("Hand already in progress".into());
        }
        self.settle_after_hand();
        if self.eligible_count() < 2 {
            return Err("Need at least two players".into());
        }

        let tournament_mode = self.tournament_mode;
        for s in self.seats.iter_mut().flatten() {
            s.hole.clear();
            s.bet = 0;
            s.committed = 0;
            s.folded = false;
            s.all_in = false;
            s.acted = false;
            s.last_action = None;
            s.last_faced = None;
            s.in_hand = s.stack > 0 && (!s.sitting_out || tournament_mode) && !s.left;
            if s.in_hand && !s.sitting_out {
                s.time_bank = (s.time_bank + TIME_BANK_REFILL).min(TIME_BANK_MAX);
            }
            if !tournament_mode && s.sitting_out && !s.left && s.stack > 0 {
                s.owes_bb = true;
            }
        }
        self.hand_no += 1;
        self.deck = shuffle_legacy(seed);
        self.board.clear();
        self.last_result = None;
        self.log.clear();
        self.phase = Phase::PreFlop;

        let in_hand = |s: &Seat| s.in_hand;
        self.button = self.next_where(self.button, in_hand).unwrap();
        let heads_up = self.eligible_count() == 2;
        let sb = if heads_up {
            self.button
        } else {
            self.next_where(self.button, in_hand).unwrap()
        };
        let bb = self.next_where(sb, in_hand).unwrap();

        let (sb_amt, bb_amt) = (self.config.small_blind, self.config.big_blind);
        self.seat_mut(sb).put(sb_amt);
        self.seat_mut(sb).last_action = Some("Small blind".into());
        self.seat_mut(bb).put(bb_amt);
        self.seat_mut(bb).last_action = Some("Big blind".into());
        let (sb_posted, bb_posted) = (self.seat(sb).bet, self.seat(bb).bet);
        self.log(Some(sb), "Small blind", sb_posted);
        self.log(Some(bb), "Big blind", bb_posted);
        self.current_bet = bb_amt;
        self.min_raise = bb_amt;
        // Players who owe a big blind post it now, live (it counts toward
        // their call). The big blind is already paying it.
        for i in 0..self.n() {
            let Some(s) = self.seats[i].as_mut() else { continue };
            if !s.owes_bb || !s.in_hand {
                continue;
            }
            s.owes_bb = false;
            if i == bb || heads_up {
                continue;
            }
            // In the small blind they top it up to a full big blind.
            let add = bb_amt.saturating_sub(s.bet).min(s.stack);
            s.put(add);
            s.last_action = Some("Posts big blind".into());
            let posted = s.bet;
            self.log(Some(i), "Posts big blind", posted);
        }

        // Deal two cards to each player, one at a time, starting left of the button.
        for _ in 0..2 {
            let mut i = self.button;
            loop {
                i = self.next_where(i, in_hand).unwrap();
                let c = self.deck.pop().unwrap();
                self.seat_mut(i).hole.push(c);
                if i == self.button {
                    break;
                }
            }
        }

        self.set_phase(Phase::PreFlop, now);
        self.to_act = Some(bb);
        self.progress(Some(bb), now);
        Ok(())
    }

    fn bump_seq(&mut self) {
        self.action_seq = Some(self.action_seq.unwrap_or(0) + 1);
    }

    fn set_phase(&mut self, p: Phase, now: u64) {
        self.bump_seq();
        self.phase = p;
        self.phase_changed_at = now;
    }

    fn round_complete(&self) -> bool {
        let actors: Vec<&Seat> = self.seats.iter().flatten().filter(|s| s.can_act()).collect();
        if actors.is_empty() {
            return true;
        }
        // A lone player who has matched every all-in has nobody left to bet against.
        if actors.len() == 1 {
            let max_bet = self.seats.iter().flatten().filter(|s| s.live()).map(|s| s.bet).max().unwrap_or(0);
            if actors[0].bet >= max_bet {
                return true;
            }
        }
        actors.iter().all(|s| s.acted && s.bet == self.current_bet)
    }

    /// Moves the hand forward after an action. `from` is the seat that just
    /// acted (if it was their turn).
    fn progress(&mut self, from: Option<usize>, now: u64) {
        self.bump_seq();
        if self.live_count() <= 1 {
            self.finish_uncontested(now);
            return;
        }
        if self.round_complete() {
            let actors = self.seats.iter().flatten().filter(|s| s.can_act()).count();
            if actors <= 1 || self.phase == Phase::River {
                self.run_out_and_showdown(now);
            } else {
                self.next_street(now);
            }
            return;
        }
        let needs_action = |s: &Seat| s.can_act() && (!s.acted || s.bet < self.current_bet);
        let start = match from {
            Some(i) => i,
            None => match self.to_act {
                // Out-of-turn change; current player still owes an action.
                Some(t) if self.seats[t].as_ref().is_some_and(needs_action) => {
                    return;
                }
                Some(t) => t,
                None => self.button,
            },
        };
        let current_bet = self.current_bet;
        self.to_act = self.next_where(start, |s| s.can_act() && (!s.acted || s.bet < current_bet));
        self.set_deadline(now);
    }

    /// Starts the clock for whoever is to act. Players sitting out (absent tournament
    /// players, or a cash player whose clock ran out mid-hand) are checked or folded at once.
    fn set_deadline(&mut self, now: u64) {
        self.bump_seq();
        let away = self.to_act.is_some_and(|i| self.seats[i].as_ref().is_some_and(|s| s.sitting_out));
        self.time_bank_active = false;
        let regular = now + self.config.action_timeout_secs * NANOS;
        self.turn_deadline = Some(regular);
        self.bank_deadline = None;
        self.action_deadline = if away { now } else { regular };
    }

    fn deal_board(&mut self, n: usize) {
        self.deck.pop(); // burn
        for _ in 0..n {
            let c = self.deck.pop().unwrap();
            self.board.push(c);
        }
    }

    fn next_street(&mut self, now: u64) {
        for s in self.seats.iter_mut().flatten() {
            s.bet = 0;
            s.acted = false;
            s.last_faced = None;
            if s.can_act() {
                s.last_action = None;
            }
        }
        self.current_bet = 0;
        self.min_raise = self.config.big_blind;
        let next = match self.phase {
            Phase::PreFlop => {
                self.deal_board(3);
                Phase::Flop
            }
            Phase::Flop => {
                self.deal_board(1);
                Phase::Turn
            }
            _ => {
                self.deal_board(1);
                Phase::River
            }
        };
        self.set_phase(next, now);
        self.log_street();
        self.to_act = self.next_where(self.button, |s| s.can_act());
        self.set_deadline(now);
    }

    fn log_street(&mut self) {
        let name = match self.board.len() {
            3 => "Flop",
            4 => "Turn",
            _ => "River",
        };
        self.log(None, name, 0);
    }

    /// Returns the part of the largest commitment that nobody matched.
    fn return_uncalled(&mut self) {
        let mut commits: Vec<(u64, usize)> = self
            .seats
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|s| (s.committed, i)))
            .collect();
        commits.sort_by(|a, b| b.cmp(a));
        if commits.len() < 2 {
            return;
        }
        let (top, i) = commits[0];
        let excess = top - commits[1].0;
        // A player who already left forfeits the excess to the pot.
        if excess > 0 && !self.seat(i).left {
            let s = self.seat_mut(i);
            s.committed -= excess;
            s.bet = s.bet.saturating_sub(excess);
            s.stack += excess;
            s.all_in = false;
            self.log(Some(i), "Uncalled bet returned", excess);
        }
    }

    /// House rake and bad-beat jackpot drop for a pot: (rake, drop). No flop, no drop.
    fn take_rake(&mut self, pot: u64) -> (u64, u64) {
        if self.board.len() < 3 {
            return (0, 0);
        }
        let pct = |amount: u64, bps: u16, cap: u64| ((amount as u128 * bps as u128 / 10_000) as u64).min(cap);
        let rake = pct(pot, self.config.rake_bps, self.config.rake_cap);
        let drop = pct(pot - rake, self.config.jackpot_bps, self.config.jackpot_cap);
        self.rake_accrued += rake;
        self.jackpot_accrued += drop;
        (rake, drop)
    }

    fn player_results(&self, won: &[u64]) -> Vec<PlayerResult> {
        self.seats
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                s.as_ref().filter(|s| s.in_hand).map(|s| PlayerResult {
                    player: s.player,
                    committed: s.committed,
                    won: won[i],
                })
            })
            .collect()
    }

    fn finish_uncontested(&mut self, now: u64) {
        self.return_uncalled();
        let pot = self.pot();
        let (rake, jackpot_drop) = self.take_rake(pot);
        let w = self
            .seats
            .iter()
            .position(|s| s.as_ref().is_some_and(|s| s.live()))
            .expect("one live player");
        let amount = pot - rake - jackpot_drop;
        self.seat_mut(w).stack += amount;
        let mut won = vec![0; self.n()];
        won[w] = amount;
        self.log(Some(w), "Wins (uncontested)", amount);
        let s = self.seat(w);
        self.last_result = Some(HandResult {
            hand_no: self.hand_no,
            board: self.board.clone(),
            winners: vec![Winner {
                seat: w as u8,
                player: s.player,
                amount,
                hand: "Uncontested".into(),
            }],
            shown: vec![],
            players: self.player_results(&won),
            pot,
            rake,
            jackpot_drop,
        });
        self.end_hand(now);
    }

    fn run_out_and_showdown(&mut self, now: u64) {
        while self.board.len() < 5 {
            let n = if self.board.is_empty() { 3 } else { 1 };
            self.deal_board(n);
            self.phase = match self.board.len() {
                3 => Phase::Flop,
                4 => Phase::Turn,
                _ => Phase::River,
            };
            self.log_street();
        }
        self.return_uncalled();

        let n = self.n();
        let committed: Vec<u64> = (0..n)
            .map(|i| self.seats[i].as_ref().map_or(0, |s| s.committed))
            .collect();
        let total: u64 = committed.iter().sum();
        let live: Vec<usize> = (0..n)
            .filter(|&i| self.seats[i].as_ref().is_some_and(|s| s.live()))
            .collect();

        // Build main pot and side pots from the commitment levels of live players.
        let mut levels: Vec<u64> = live.iter().map(|&i| committed[i]).collect();
        levels.sort();
        levels.dedup();
        let mut pots: Vec<(u64, Vec<usize>)> = vec![];
        let mut prev = 0;
        for &level in &levels {
            let amount: u64 = committed.iter().map(|&c| c.min(level) - c.min(prev)).sum();
            let eligible: Vec<usize> = live.iter().copied().filter(|&i| committed[i] >= level).collect();
            if amount > 0 {
                pots.push((amount, eligible));
            }
            prev = level;
        }
        // Chips from players who left mid-hand above every live level.
        let assigned: u64 = pots.iter().map(|p| p.0).sum();
        if let Some(last) = pots.last_mut() {
            last.0 += total - assigned;
        }

        // Rake and jackpot drop come out of every pot in proportion to its
        // size, so a short all-in doesn't pay for chips they can't win.
        let (rake_total, jackpot_drop) = self.take_rake(total);
        let take = rake_total + jackpot_drop;
        let pot_sum: u64 = pots.iter().map(|p| p.0).sum();
        let mut left = take;
        if pot_sum > 0 {
            for pot in pots.iter_mut() {
                let r = ((take as u128 * pot.0 as u128) / pot_sum as u128) as u64;
                pot.0 -= r;
                left -= r;
            }
        }
        // Rounding leftovers: from the largest pots first.
        let mut order: Vec<usize> = (0..pots.len()).collect();
        order.sort_by_key(|&k| std::cmp::Reverse(pots[k].0));
        for k in order {
            let r = left.min(pots[k].0);
            pots[k].0 -= r;
            left -= r;
        }

        let scores: Vec<Option<(HandScore, String)>> = (0..n)
            .map(|i| {
                if !live.contains(&i) {
                    return None;
                }
                let mut cards = self.seat(i).hole.clone();
                cards.extend_from_slice(&self.board);
                let (score, _) = best_hand(&cards);
                Some((score, category(score).to_string()))
            })
            .collect();

        let mut won = vec![0u64; n];
        for (amount, eligible) in &pots {
            let best = eligible.iter().map(|&i| scores[i].as_ref().unwrap().0).max().unwrap();
            // Winners in seat order starting left of the button (odd chips go first).
            let winners: Vec<usize> = (1..=n)
                .map(|k| (self.button + k) % n)
                .filter(|i| eligible.contains(i) && scores[*i].as_ref().unwrap().0 == best)
                .collect();
            let share = amount / winners.len() as u64;
            let mut odd = amount % winners.len() as u64;
            for &w in &winners {
                let extra = if odd > 0 {
                    odd -= 1;
                    1
                } else {
                    0
                };
                won[w] += share + extra;
            }
        }

        for &i in &live {
            let hand = scores[i].as_ref().unwrap().1.clone();
            self.log(Some(i), format!("Shows {hand}"), 0);
        }
        let mut winners = vec![];
        for i in 0..n {
            if won[i] > 0 {
                let hand = scores[i].as_ref().unwrap().1.clone();
                self.log(Some(i), format!("Wins with {hand}"), won[i]);
                self.seat_mut(i).stack += won[i];
                winners.push(Winner {
                    seat: i as u8,
                    player: self.seat(i).player,
                    amount: won[i],
                    hand: scores[i].as_ref().unwrap().1.clone(),
                });
            }
        }
        let shown = live
            .iter()
            .map(|&i| ShownHand {
                seat: i as u8,
                player: self.seat(i).player,
                cards: self.seat(i).hole.clone(),
                hand: scores[i].as_ref().unwrap().1.clone(),
            })
            .collect();
        self.last_result = Some(HandResult {
            hand_no: self.hand_no,
            board: self.board.clone(),
            winners,
            shown,
            players: self.player_results(&won),
            pot: total,
            rake: rake_total,
            jackpot_drop,
        });
        self.end_hand(now);
    }

    fn end_hand(&mut self, now: u64) {
        self.to_act = None;
        self.current_bet = 0;
        for s in self.seats.iter_mut().flatten() {
            s.bet = 0;
        }
        self.set_phase(Phase::HandOver, now);
    }

    // ---------------------------------------------------------------- actions

    pub fn act(&mut self, player: &Principal, action: Action, now: u64) -> Result<(), String> {
        self.act_for(player, action, now, None)
    }

    /// Like `act`, for a command made at revision `expected` (`None`: any).
    pub fn act_for(&mut self, player: &Principal, action: Action, now: u64, expected: Option<u64>) -> Result<(), String> {
        if expected.is_some_and(|n| n != self.action_seq.unwrap_or(0)) {
            return Err("The table moved on — check the new situation and act again".into());
        }
        if !self.phase.is_betting() {
            return Err("No hand in progress".into());
        }
        let i = self.to_act.ok_or("No hand in progress")?;
        if self.seat(i).player != *player || self.seat(i).left {
            return Err("It is not your turn".into());
        }
        let current_bet = self.current_bet;
        let min_raise = self.min_raise;
        let may_raise = self.may_raise(i);
        let (bank_active, bank_start) = (self.time_bank_active, self.bank_started_at);
        let s = self.seat_mut(i);
        let to_call = current_bet - s.bet;
        if bank_active {
            // Only the time actually used comes off the bank.
            s.time_bank = s.time_bank.saturating_sub(now.saturating_sub(bank_start));
        }

        let new_bet = match action {
            Action::Fold => {
                s.folded = true;
                s.last_action = Some("Fold".into());
                None
            }
            Action::Check => {
                if to_call > 0 {
                    return Err("You cannot check facing a bet".into());
                }
                s.last_action = Some("Check".into());
                None
            }
            Action::Call => {
                if to_call == 0 {
                    s.last_action = Some("Check".into());
                } else {
                    s.put(to_call);
                    s.last_action = Some(if s.all_in { "All-in".into() } else { "Call".into() });
                }
                None
            }
            Action::AllIn => {
                let stack = s.stack;
                if stack > to_call && !may_raise {
                    return Err("The betting wasn't reopened: you can only call or fold".into());
                }
                s.put(stack);
                s.last_action = Some("All-in".into());
                Some(s.bet)
            }
            Action::Raise(to) => {
                let max_to = s.bet + s.stack;
                if to <= current_bet {
                    return Err("Raise must exceed the current bet".into());
                }
                if !may_raise {
                    return Err("The betting wasn't reopened: you can only call or fold".into());
                }
                if to > max_to {
                    return Err("Not enough chips".into());
                }
                if to - current_bet < min_raise && to != max_to {
                    return Err(format!("Minimum raise is to {}", current_bet + min_raise));
                }
                let add = to - s.bet;
                s.put(add);
                s.last_action = Some(if s.all_in {
                    "All-in".into()
                } else if current_bet == 0 {
                    "Bet".into()
                } else {
                    "Raise".into()
                });
                Some(to)
            }
        };
        s.acted = true;
        s.last_faced = Some(current_bet.max(s.bet));
        let (text, amount) = (s.last_action.clone().unwrap_or_default(), s.bet);
        let amount = if matches!(action, Action::Fold | Action::Check) { 0 } else { amount };
        self.log(Some(i), text, amount);

        if let Some(bet) = new_bet {
            if bet > current_bet {
                let increment = bet - current_bet;
                self.current_bet = bet;
                if increment >= min_raise {
                    self.min_raise = increment;
                }
                // Everyone else must respond to the new bet.
                for (j, other) in self.seats.iter_mut().enumerate() {
                    if let Some(o) = other {
                        if j != i && o.can_act() {
                            o.acted = false;
                        }
                    }
                }
            }
        }

        self.progress(Some(i), now);
        Ok(())
    }

    /// Auto-acts for a player whose clock ran out. Returns the timed-out player.
    pub fn handle_timeout(&mut self, now: u64) -> Option<Principal> {
        if !self.phase.is_betting() || now < self.action_deadline {
            return None;
        }
        let i = self.to_act?;
        let s = self.seat(i);
        let p = s.player;
        // Out of regular time: draw on the time bank before auto-acting.
        // Players already sitting out (absent tournament players) don't use it.
        let (sitting_out, bank) = (s.sitting_out, s.time_bank);
        if !self.time_bank_active && !sitting_out && bank >= NANOS {
            self.time_bank_active = true;
            self.bank_started_at = self.action_deadline;
            self.action_deadline += bank;
            self.bank_deadline = Some(self.action_deadline);
            return None;
        }
        let action = if s.bet >= self.current_bet { Action::Check } else { Action::Fold };
        let s = self.seat_mut(i);
        s.sitting_out = true;
        s.sitting_out_since = s.sitting_out_since.or(Some(now));
        self.act(&p, action, now).ok()?;
        Some(p)
    }

    /// Hole cards of everyone dealt into the current/last hand, by seat.
    pub fn dealt_hands(&self) -> Vec<(u8, Principal, Vec<Card>)> {
        self.seats
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let s = s.as_ref().filter(|s| s.in_hand && !s.hole.is_empty())?;
                Some((i as u8, s.player, s.hole.clone()))
            })
            .collect()
    }

    // ------------------------------------------------------------------- view

    /// Legal amounts for the player whose turn it is.
    pub fn betting_options(&self, i: usize) -> (u64, u64, u64) {
        let s = self.seat(i);
        let to_call = (self.current_bet - s.bet).min(s.stack);
        if !self.may_raise(i) {
            return (to_call, 0, 0);
        }
        let max_to = s.bet + s.stack;
        let min_to = (self.current_bet + self.min_raise).min(max_to);
        (to_call, min_to, max_to)
    }

    /// Whether the player at `i` may (re-)raise: they haven't acted on this
    /// street yet, or the bet has risen by at least a full raise since they did.
    pub fn may_raise(&self, i: usize) -> bool {
        match self.seat(i).last_faced {
            None => true,
            Some(faced) => self.current_bet >= faced + self.min_raise,
        }
    }

    /// Reveals the hole cards of a player who won without a showdown, at their
    /// choice, until the next hand is dealt. Returns their seat.
    pub fn show_cards(&mut self, player: &Principal) -> Result<usize, String> {
        if self.phase != Phase::HandOver {
            return Err("Cards can only be shown between hands".into());
        }
        let r = self.last_result.as_ref().ok_or("No hand to show")?;
        if !r.shown.is_empty() {
            return Err("This hand already went to a showdown or was shown".into());
        }
        let w = match r.winners.as_slice() {
            [w] if w.player == *player => w.seat as usize,
            _ => return Err("Only the winner can show their cards".into()),
        };
        let cards = self.seats[w].as_ref().map(|s| s.hole.clone()).unwrap_or_default();
        if cards.is_empty() {
            return Err("No cards to show".into());
        }
        // A hand name needs five cards; before the flop just the cards are shown.
        let hand = if self.board.len() >= 3 {
            let mut all = cards.clone();
            all.extend_from_slice(&self.board);
            category(best_hand(&all).0).to_string()
        } else {
            String::new()
        };
        self.log(Some(w), if hand.is_empty() { "Shows".to_string() } else { format!("Shows {hand}") }, 0);
        self.last_result.as_mut().unwrap().shown.push(ShownHand { seat: w as u8, player: *player, cards, hand });
        Ok(w)
    }

    /// Hole cards visible to `viewer` at seat `i`.
    pub fn visible_cards(&self, i: usize, viewer: Option<&Principal>) -> Vec<Card> {
        let s = self.seat(i);
        if viewer == Some(&s.player) {
            return s.hole.clone();
        }
        if self.phase == Phase::HandOver {
            if let Some(r) = &self.last_result {
                if let Some(sh) = r.shown.iter().find(|sh| sh.seat as usize == i) {
                    return sh.cards.clone();
                }
            }
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(n: u8) -> Principal {
        Principal::from_slice(&[n; 10])
    }

    fn config() -> TableConfig {
        TableConfig {
            name: "Test".into(),
            currency: Currency::Icp,
            small_blind: 5,
            big_blind: 10,
            min_buy_in: 100,
            max_buy_in: 1000,
            max_seats: 6,
            rake_bps: 0,
            rake_cap: 0,
            jackpot_bps: 0,
            jackpot_cap: 0,
            action_timeout_secs: 20,
        }
    }

    fn total(t: &Table) -> u64 {
        t.seats.iter().flatten().map(|s| s.stack + s.committed).sum::<u64>()
    }

    fn to_act_player(t: &Table) -> Principal {
        t.seat(t.to_act.unwrap()).player
    }

    #[test]
    fn heads_up_blinds_and_fold() {
        let mut t = Table::new(1, config());
        t.sit(p(1), 0, 500).unwrap();
        t.sit(p(2), 1, 500).unwrap();
        t.start_hand([1; 32], 0).unwrap();
        // Heads-up: button posts the small blind and acts first preflop.
        let btn = t.button;
        assert_eq!(t.seat(btn).bet, 5);
        assert_eq!(t.to_act, Some(btn));
        let pl = to_act_player(&t);
        t.act(&pl, Action::Fold, 1).unwrap();
        assert_eq!(t.phase, Phase::HandOver);
        let other = 1 - btn;
        assert_eq!(t.seat(other).stack, 505);
        assert_eq!(t.seat(btn).stack, 495);
    }

    #[test]
    fn check_down_to_showdown_conserves_chips() {
        let mut t = Table::new(1, config());
        for k in 0..4 {
            t.sit(p(k + 1), k, 300).unwrap();
        }
        t.start_hand([9; 32], 0).unwrap();
        assert_eq!(t.phase, Phase::PreFlop);
        // Everyone calls, big blind checks.
        while t.phase == Phase::PreFlop {
            let pl = to_act_player(&t);
            t.act(&pl, Action::Call, 1).unwrap();
        }
        assert_eq!(t.phase, Phase::Flop);
        assert_eq!(t.board.len(), 3);
        while t.phase.is_betting() {
            let pl = to_act_player(&t);
            t.act(&pl, Action::Check, 1).unwrap();
        }
        assert_eq!(t.phase, Phase::HandOver);
        assert_eq!(t.board.len(), 5);
        let r = t.last_result.as_ref().unwrap();
        assert_eq!(r.pot, 40);
        assert_eq!(r.shown.len(), 4);
        let stacks: u64 = t.seats.iter().flatten().map(|s| s.stack).sum();
        assert_eq!(stacks, 1200);
    }

    #[test]
    fn min_raise_enforced_and_reopens_action() {
        let mut t = Table::new(1, config());
        for k in 0..3 {
            t.sit(p(k + 1), k, 500).unwrap();
        }
        t.start_hand([3; 32], 0).unwrap();
        let utg = to_act_player(&t);
        assert!(t.act(&utg, Action::Raise(15), 1).is_err()); // below min raise to 20
        t.act(&utg, Action::Raise(30), 1).unwrap();
        assert_eq!(t.min_raise, 20);
        let sb = to_act_player(&t);
        assert!(t.act(&sb, Action::Raise(45), 1).is_err());
        t.act(&sb, Action::Raise(50), 1).unwrap();
        let bb = to_act_player(&t);
        t.act(&bb, Action::Fold, 1).unwrap();
        // Original raiser must act again.
        assert_eq!(to_act_player(&t), utg);
        t.act(&utg, Action::Call, 1).unwrap();
        assert_eq!(t.phase, Phase::Flop);
        assert_eq!(t.pot(), 110);
    }

    #[test]
    fn hand_log_records_actions_and_streets() {
        let mut t = Table::new(1, config());
        t.sit(p(1), 0, 500).unwrap();
        t.sit(p(2), 1, 500).unwrap();
        t.start_hand([1; 32], 0).unwrap();
        while t.phase.is_betting() {
            let pl = to_act_player(&t);
            t.act(&pl, Action::Call, 1).unwrap();
        }
        let texts: Vec<&str> = t.log.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(&texts[..2], &["Small blind", "Big blind"]);
        for street in ["Flop", "Turn", "River"] {
            assert!(texts.contains(&street), "{texts:?}");
        }
        assert!(texts.iter().any(|t| t.starts_with("Wins")));
        let river = t.log.iter().find(|e| e.text == "River").unwrap();
        assert_eq!(river.phase, Phase::River);
        assert_eq!(t.dealt_hands().len(), 2);
    }

    #[test]
    fn tournament_mode_blinds_off_absent_players() {
        let mut t = Table::new(1, config());
        t.tournament_mode = true;
        t.sit(p(1), 0, 100).unwrap();
        t.sit(p(2), 1, 100).unwrap();
        t.set_sitting_out(&p(1), true, 0).unwrap();
        let mut now = 0;
        let mut hands = 0;
        // p1 is away: they keep being dealt in and auto-folded until they bust.
        while t.seats[0].as_ref().is_some_and(|s| s.stack > 0) && hands < 100 {
            t.start_hand([hands as u8; 32], now).unwrap();
            while t.phase.is_betting() {
                if t.seat(t.to_act.unwrap()).player == p(1) {
                    assert_eq!(t.action_deadline, now, "absent player acts immediately");
                    t.handle_timeout(now).unwrap();
                } else {
                    t.act(&p(2), Action::Call, now).unwrap();
                }
            }
            now += 10 * NANOS;
            hands += 1;
        }
        assert!(hands < 100);
        assert_eq!(t.seats[1].as_ref().unwrap().stack, 200);
    }

    #[test]
    fn side_pots() {
        let mut t = Table::new(1, config());
        t.sit(p(1), 0, 100).unwrap();
        t.sit(p(2), 1, 300).unwrap();
        t.sit(p(3), 2, 600).unwrap();
        t.start_hand([5; 32], 0).unwrap();
        let start_total = total(&t);
        while t.phase.is_betting() {
            let pl = to_act_player(&t);
            t.act(&pl, Action::AllIn, 1).unwrap();
        }
        assert_eq!(t.phase, Phase::HandOver);
        assert_eq!(t.board.len(), 5);
        let r = t.last_result.clone().unwrap();
        // The 300 uncalled chips of the big stack are returned.
        assert_eq!(r.pot, 100 * 3 + 200 * 2);
        let stacks: u64 = t.seats.iter().flatten().map(|s| s.stack).sum();
        assert_eq!(stacks, start_total);
        // Short stack can win at most the main pot.
        let short_won = r.players.iter().find(|x| x.player == p(1)).unwrap().won;
        assert!(short_won <= 300);
        let won: u64 = r.players.iter().map(|x| x.won).sum();
        assert_eq!(won, r.pot);
    }

    /// Button seat 0 (p3), then p1, p2, p4. Everyone limps to the flop, then
    /// p1 bets 100 and p2 calls.
    fn short_all_in_table(p3_buy: u64, p4_buy: u64) -> Table {
        let mut t = Table::new(1, config());
        t.sit(p(3), 0, p3_buy).unwrap();
        t.sit(p(1), 1, 1000).unwrap();
        t.sit(p(2), 2, 1000).unwrap();
        t.sit(p(4), 3, p4_buy).unwrap();
        t.start_hand([9; 32], 0).unwrap();
        while t.phase == Phase::PreFlop {
            let pl = to_act_player(&t);
            if t.act(&pl, Action::Check, 1).is_err() {
                t.act(&pl, Action::Call, 1).unwrap();
            }
        }
        assert_eq!(to_act_player(&t), p(1));
        t.act(&p(1), Action::Raise(100), 1).unwrap();
        t.act(&p(2), Action::Call, 1).unwrap();
        t
    }

    #[test]
    fn joining_a_running_game_posts_a_big_blind() {
        let mut t = Table::new(1, config());
        t.sit(p(1), 0, 500).unwrap();
        t.sit(p(2), 1, 500).unwrap();
        assert!(!t.seat(0).owes_bb, "first players of a new game don't owe");
        t.start_hand([1; 32], 0).unwrap();
        t.sit(p(3), 2, 500).unwrap();
        assert!(t.seat(2).owes_bb);
        while t.phase.is_betting() {
            let pl = to_act_player(&t);
            t.act(&pl, Action::Fold, 1).unwrap();
        }
        t.start_hand([2; 32], 10 * NANOS).unwrap();
        assert!(!t.seat(2).owes_bb);
        assert_eq!(t.seat(2).committed, 10, "a full big blind, whatever the position");
    }

    #[test]
    fn a_short_all_in_does_not_reopen_the_betting() {
        let mut t = short_all_in_table(220, 160);
        t.act(&p(4), Action::AllIn, 1).unwrap(); // 150: a 50 raise, short of the 100 minimum
        assert_eq!(t.current_bet, 150);
        t.act(&p(3), Action::Call, 1).unwrap();
        // Back to the bettor: they may only call or fold.
        assert_eq!(to_act_player(&t), p(1));
        assert_eq!(t.betting_options(1), (50, 0, 0));
        assert!(t.act(&p(1), Action::Raise(400), 1).is_err());
        assert!(t.act(&p(1), Action::AllIn, 1).is_err());
        t.act(&p(1), Action::Call, 1).unwrap();
        assert!(t.act(&p(2), Action::Raise(400), 1).is_err());
        t.act(&p(2), Action::Call, 1).unwrap();
        assert_eq!(t.phase, Phase::Turn);
        // A new street opens the betting again.
        assert!(t.may_raise(1));
    }

    #[test]
    fn short_all_ins_that_add_up_to_a_full_raise_reopen_it() {
        let mut t = short_all_in_table(220, 160);
        t.act(&p(4), Action::AllIn, 1).unwrap(); // to 150 (+50)
        assert!(!t.may_raise(1));
        t.act(&p(3), Action::AllIn, 1).unwrap(); // to 210 (+60): 110 over p1's bet
        assert!(t.may_raise(1));
        t.act(&p(1), Action::Raise(400), 1).unwrap();
        // p3 raised (short) after p1's bet; p1's full re-raise reopens it for p2.
        assert!(t.may_raise(2));
    }

    #[test]
    fn rake_is_shared_by_main_and_side_pots() {
        let card = |r: u8, su: u8| r * 4 + su;
        let mut cfg = config();
        cfg.rake_bps = 1000; // 10%
        cfg.rake_cap = 1_000;
        let mut t = Table::new(1, cfg);
        t.sit(p(1), 0, 100).unwrap();
        t.sit(p(2), 1, 1000).unwrap();
        t.sit(p(3), 2, 1000).unwrap();
        t.start_hand([3; 32], 0).unwrap();
        // p1 aces, p2 kings, p3 queens; a dry board (deck pops from the end).
        t.seats[0].as_mut().unwrap().hole = vec![card(12, 3), card(12, 2)];
        t.seats[1].as_mut().unwrap().hole = vec![card(11, 0), card(11, 1)];
        t.seats[2].as_mut().unwrap().hole = vec![card(10, 0), card(10, 1)];
        t.deck = vec![card(2, 0), card(6, 0), card(9, 3), card(4, 0), card(7, 2), card(5, 1), card(0, 0), card(3, 0)];
        t.act(&p(1), Action::AllIn, 1).unwrap(); // 100
        t.act(&p(2), Action::Call, 1).unwrap();
        t.act(&p(3), Action::Call, 1).unwrap();
        t.act(&p(2), Action::Raise(200), 1).unwrap();
        t.act(&p(3), Action::Call, 1).unwrap();
        while t.phase.is_betting() {
            let pl = to_act_player(&t);
            t.act(&pl, Action::Check, 1).unwrap();
        }
        // Main pot 300, side pot 400, rake 70 split 30/40.
        assert_eq!(t.rake_accrued, 70);
        assert_eq!(t.seat(0).stack, 270, "the short stack pays rake only on the pot they can win");
        assert_eq!(t.seat(1).stack, 1000 - 300 + 360);
        assert_eq!(t.seat(2).stack, 700);
    }

    #[test]
    fn rake_only_after_flop() {
        let mut cfg = config();
        cfg.rake_bps = 500;
        cfg.rake_cap = 1_000;
        let mut t = Table::new(1, cfg);
        t.sit(p(1), 0, 500).unwrap();
        t.sit(p(2), 1, 500).unwrap();
        t.start_hand([2; 32], 0).unwrap();
        let pl = to_act_player(&t);
        t.act(&pl, Action::Fold, 1).unwrap();
        assert_eq!(t.rake_accrued, 0);

        t.start_hand([4; 32], 10 * NANOS).unwrap();
        while t.phase.is_betting() {
            let pl = to_act_player(&t);
            if t.phase == Phase::PreFlop {
                t.act(&pl, Action::Call, 1).unwrap();
            } else {
                t.act(&pl, Action::Check, 1).unwrap();
            }
        }
        assert_eq!(t.rake_accrued, 1); // 5% of 20
        let stacks: u64 = t.seats.iter().flatten().map(|s| s.stack).sum();
        assert_eq!(stacks + t.rake_accrued, 1000);
    }

    #[test]
    fn jackpot_drop_after_rake() {
        let mut cfg = config();
        cfg.rake_bps = 500;
        cfg.rake_cap = 1_000;
        cfg.jackpot_bps = 100;
        cfg.jackpot_cap = 10;
        let mut t = Table::new(1, cfg);
        t.sit(p(1), 0, 1_000).unwrap();
        t.sit(p(2), 1, 1_000).unwrap();
        t.start_hand([2; 32], 0).unwrap();
        // Preflop fold: no flop, no drop.
        let pl = to_act_player(&t);
        t.act(&pl, Action::Fold, 1).unwrap();
        assert_eq!((t.rake_accrued, t.jackpot_accrued), (0, 0));

        t.start_hand([4; 32], 10 * NANOS).unwrap();
        let first = to_act_player(&t);
        t.act(&first, Action::Raise(500), 1).unwrap();
        let second = to_act_player(&t);
        t.act(&second, Action::Call, 1).unwrap();
        while t.phase.is_betting() {
            let pl = to_act_player(&t);
            t.act(&pl, Action::Check, 1).unwrap();
        }
        // Pot 1,000: rake 50, then 1% of 950 = 9 (under the cap of 10).
        assert_eq!(t.rake_accrued, 50);
        assert_eq!(t.jackpot_accrued, 9);
        let r = t.last_result.as_ref().unwrap();
        assert_eq!((r.rake, r.jackpot_drop), (50, 9));
        let stacks: u64 = t.seats.iter().flatten().map(|s| s.stack).sum();
        assert_eq!(stacks + 59, 2_000);
    }

    #[test]
    fn standing_mid_hand_waits_for_the_hand_to_end() {
        let mut t = Table::new(1, config());
        for k in 0..3 {
            t.sit(p(k + 1), k, 200).unwrap();
        }
        t.start_hand([8; 32], 0).unwrap();
        let actor = t.to_act.unwrap();
        // A player who is not to act stands up: they stay in the hand.
        let leaver = (0..3).find(|&i| i != actor).unwrap();
        let lp = t.seat(leaver).player;
        assert_eq!(t.stand(&lp, 1).unwrap(), StandOutcome::AfterHand);
        assert_eq!(t.stand(&lp, 1).unwrap(), StandOutcome::AfterHand, "asking twice changes nothing");
        let s = t.seat(leaver);
        assert!(!s.folded && s.sitting_out && s.leaving == Some(true));
        assert_eq!(t.seat_of(&lp), Some(leaver));
        assert!(t.take_leavers().is_empty(), "not while the hand is on");
        while t.phase.is_betting() {
            let pl = to_act_player(&t);
            if t.seat(t.to_act.unwrap()).sitting_out {
                t.handle_timeout(1).unwrap();
            } else {
                t.act(&pl, Action::Call, 1).unwrap();
            }
        }
        let back = t.take_leavers();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].0, lp);
        assert!(t.seat_of(&lp).is_none());
        let stacks: u64 = t.seats.iter().flatten().map(|s| s.stack).sum();
        assert_eq!(stacks + back[0].1, 600);
        assert!(t.take_leavers().is_empty(), "taken once");
        t.settle_after_hand();
        assert_eq!(t.player_count(), 2);
    }

    #[test]
    fn a_player_who_already_folded_leaves_at_once() {
        let mut t = Table::new(1, config());
        for k in 0..3 {
            t.sit(p(k + 1), k, 200).unwrap();
        }
        t.start_hand([8; 32], 0).unwrap();
        let lp = to_act_player(&t);
        let committed = t.seat(t.to_act.unwrap()).committed;
        t.act(&lp, Action::Fold, 1).unwrap();
        assert_eq!(t.stand(&lp, 1).unwrap(), StandOutcome::Left(200 - committed));
        assert!(t.seat_of(&lp).is_none());
        while t.phase.is_betting() {
            let pl = to_act_player(&t);
            t.act(&pl, Action::Call, 1).unwrap();
        }
        assert!(t.take_leavers().is_empty(), "already paid out");
    }

    #[test]
    fn all_in_player_who_leaves_still_wins_the_showdown() {
        let card = |r: u8, su: u8| r * 4 + su;
        let mut t = Table::new(1, config());
        t.sit(p(1), 0, 100).unwrap();
        t.sit(p(2), 1, 500).unwrap();
        t.sit(p(3), 2, 1000).unwrap();
        t.start_hand([3; 32], 0).unwrap();
        // p1 queens, p2 aces, p3 kings; a dry board (deck pops from the end).
        t.seats[0].as_mut().unwrap().hole = vec![card(10, 0), card(10, 1)];
        t.seats[1].as_mut().unwrap().hole = vec![card(12, 3), card(12, 2)];
        t.seats[2].as_mut().unwrap().hole = vec![card(11, 0), card(11, 1)];
        t.deck = vec![card(2, 0), card(6, 0), card(9, 3), card(4, 0), card(7, 2), card(5, 1), card(0, 0), card(3, 0)];
        t.act(&p(1), Action::AllIn, 1).unwrap(); // 100
        t.act(&p(2), Action::AllIn, 1).unwrap(); // 500
        // p2 has nothing left behind and walks away before p3 decides.
        assert_eq!(t.stand(&p(2), 2).unwrap(), StandOutcome::AfterHand);
        assert!(!t.seat(1).folded, "leaving never folds");
        t.act(&p(3), Action::Call, 3).unwrap();
        assert_eq!(t.phase, Phase::HandOver);
        // Main pot 300 and side pot 800, both to p2's aces.
        let r = t.last_result.as_ref().unwrap();
        let won: u64 = r.winners.iter().filter(|w| w.player == p(2)).map(|w| w.amount).sum();
        assert_eq!(won, 1100);
        assert!(r.winners.iter().all(|w| w.player == p(2)));
        assert_eq!(t.take_leavers(), vec![(p(2), 1100)]);
        assert!(t.seat_of(&p(2)).is_none());
        let stacks: u64 = t.seats.iter().flatten().map(|s| s.stack).sum();
        assert_eq!(stacks + 1100, 1600);
    }

    #[test]
    fn leaver_whose_turn_it_is_is_checked_or_folded() {
        let mut t = Table::new(1, config());
        for k in 0..3 {
            t.sit(p(k + 1), k, 200).unwrap();
        }
        t.start_hand([8; 32], 0).unwrap();
        // Preflop, facing the big blind: folded.
        let first = to_act_player(&t);
        let i = t.to_act.unwrap();
        assert_eq!(t.stand(&first, 5).unwrap(), StandOutcome::AfterHand);
        assert_eq!(t.action_deadline, 5, "acts at once");
        assert!(t.set_sitting_out(&first, false, 5).is_err(), "no coming back while leaving");
        assert_eq!(t.handle_timeout(5), Some(first));
        assert!(t.seat(i).folded);
        assert_eq!(t.take_leavers(), vec![], "the hand is still on");
        // The others see a flop; whoever acts first there leaves and is checked.
        while t.phase == Phase::PreFlop {
            let pl = to_act_player(&t);
            t.act(&pl, Action::Call, 6).unwrap();
        }
        assert_eq!(t.phase, Phase::Flop);
        let second = to_act_player(&t);
        let j = t.to_act.unwrap();
        assert_eq!(t.stand(&second, 7).unwrap(), StandOutcome::AfterHand);
        assert_eq!(t.handle_timeout(7), Some(second));
        assert!(!t.seat(j).folded, "a free check keeps them in");
        assert_eq!(t.seat(j).last_action.as_deref(), Some("Check"));
        assert!(t.phase.is_betting());
    }

    #[test]
    fn timeout_checks_or_folds_and_sits_out() {
        let mut t = Table::new(1, config());
        t.sit(p(1), 0, 200).unwrap();
        t.sit(p(2), 1, 200).unwrap();
        t.start_hand([6; 32], 0).unwrap();
        assert!(t.handle_timeout(1).is_none());
        let late = to_act_player(&t);
        // The clock runs out, then the time bank (30s) kicks in.
        assert!(t.handle_timeout(20 * NANOS).is_none());
        assert!(t.time_bank_active);
        assert_eq!(t.action_deadline, 50 * NANOS);
        assert!(t.handle_timeout(49 * NANOS).is_none());
        let who = t.handle_timeout(50 * NANOS).unwrap();
        assert_eq!(who, late);
        assert_eq!(t.seat(t.seat_of(&late).unwrap()).time_bank, 0, "bank used up");
        assert_eq!(t.phase, Phase::HandOver);
        let i = t.seat_of(&late).unwrap();
        assert!(t.seat(i).sitting_out);
        assert_eq!(t.eligible_count(), 1);
    }

    /// Plays preflop to the flop (call, check) and times out whoever acts first there.
    fn time_out_on_the_flop(t: &mut Table) -> (Principal, u64) {
        t.sit(p(1), 0, 200).unwrap();
        t.sit(p(2), 1, 200).unwrap();
        t.start_hand([6; 32], 0).unwrap();
        let a = to_act_player(t);
        t.act(&a, Action::Call, 1).unwrap();
        let b = to_act_player(t);
        t.act(&b, Action::Check, 2).unwrap();
        assert_eq!(t.phase, Phase::Flop);
        let late = to_act_player(t);
        // Regular clock, then the time bank, then the auto-check.
        assert!(t.handle_timeout(t.action_deadline).is_none());
        let out_at = t.action_deadline;
        assert_eq!(t.handle_timeout(out_at), Some(late));
        assert!(t.seat(t.seat_of(&late).unwrap()).sitting_out);
        assert_eq!(t.phase, Phase::Flop, "a check keeps them in the hand");
        (late, out_at)
    }

    #[test]
    fn sitting_out_player_is_checked_or_folded_at_once_for_the_rest_of_the_hand() {
        let mut t = Table::new(1, config());
        let (late, out_at) = time_out_on_the_flop(&mut t);
        let other = to_act_player(&t);
        assert_ne!(other, late);
        t.act(&other, Action::Check, out_at + NANOS).unwrap();
        assert_eq!(t.phase, Phase::Turn);
        assert_eq!(to_act_player(&t), late);
        assert_eq!(t.action_deadline, out_at + NANOS, "no fresh clock while sitting out");
        assert_eq!(t.handle_timeout(out_at + NANOS), Some(late));
        assert!(!t.time_bank_active, "no time bank while sitting out");
    }

    #[test]
    fn coming_back_mid_hand_does_not_get_a_fresh_clock() {
        let mut t = Table::new(1, config());
        let (late, out_at) = time_out_on_the_flop(&mut t);
        let other = to_act_player(&t);
        t.act(&other, Action::Check, out_at + NANOS).unwrap();
        assert_eq!(to_act_player(&t), late);
        // The turn began at out_at + 1s with the regular 20s; coming back later keeps that deadline.
        t.set_sitting_out(&late, false, out_at + 2 * NANOS).unwrap();
        assert_eq!(t.action_deadline, out_at + NANOS + 20 * NANOS);
        assert!(t.handle_timeout(out_at + 2 * NANOS).is_none());
        t.act(&late, Action::Check, out_at + 3 * NANOS).unwrap();
    }

    #[test]
    fn coming_back_after_auto_sit_out_on_a_later_turn_gets_normal_time() {
        let mut t = Table::new(1, config());
        let (late, out_at) = time_out_on_the_flop(&mut t);
        let other = to_act_player(&t);
        t.act(&other, Action::Check, out_at + 5 * NANOS).unwrap();
        assert_eq!(to_act_player(&t), late);
        t.set_sitting_out(&late, false, out_at + 6 * NANOS).unwrap();
        assert_eq!(t.action_deadline, out_at + 5 * NANOS + 20 * NANOS, "a full regular turn");
        assert!(!t.time_bank_active);
    }

    #[test]
    fn repeated_back_calls_do_not_move_the_deadline() {
        let mut t = Table::new(1, config());
        t.sit(p(1), 0, 200).unwrap();
        t.sit(p(2), 1, 200).unwrap();
        t.start_hand([6; 32], 0).unwrap();
        let who = to_act_player(&t);
        let deadline = t.action_deadline;
        for k in 1..=10 {
            t.set_sitting_out(&who, false, k * NANOS).unwrap();
            assert_eq!(t.action_deadline, deadline);
        }
        // Same while on the time bank.
        assert!(t.handle_timeout(deadline).is_none());
        let bank_deadline = t.action_deadline;
        for k in 1..=10 {
            t.set_sitting_out(&who, false, deadline + k * NANOS).unwrap();
            assert_eq!(t.action_deadline, bank_deadline);
            assert!(t.time_bank_active);
        }
    }

    #[test]
    fn sit_out_and_back_does_not_extend_the_turn() {
        let mut t = Table::new(1, config());
        t.sit(p(1), 0, 200).unwrap();
        t.sit(p(2), 1, 200).unwrap();
        t.start_hand([6; 32], 0).unwrap();
        let who = to_act_player(&t);
        let deadline = t.action_deadline;
        for k in 0..5 {
            t.set_sitting_out(&who, true, (10 + 2 * k) * NANOS).unwrap();
            t.set_sitting_out(&who, false, (11 + 2 * k) * NANOS).unwrap();
            assert_eq!(t.action_deadline, deadline, "toggle {k}");
        }
        assert!(t.handle_timeout(deadline - 1).is_none());
        assert!(!t.time_bank_active);
        // Sitting out on your turn still acts at once.
        t.set_sitting_out(&who, true, 12 * NANOS).unwrap();
        assert_eq!(t.action_deadline, 12 * NANOS);
        assert_eq!(t.handle_timeout(12 * NANOS), Some(who));
    }

    /// Heads-up preflop, first actor on the time bank (started at 20s, ends at 50s).
    fn on_the_time_bank() -> (Table, Principal, usize) {
        let mut t = Table::new(1, config());
        t.sit(p(1), 0, 200).unwrap();
        t.sit(p(2), 1, 200).unwrap();
        t.start_hand([6; 32], 0).unwrap();
        let who = to_act_player(&t);
        let i = t.seat_of(&who).unwrap();
        assert!(t.handle_timeout(20 * NANOS).is_none());
        assert!(t.time_bank_active);
        assert_eq!(t.action_deadline, 50 * NANOS);
        (t, who, i)
    }

    #[test]
    fn toggling_during_the_time_bank_charges_the_bank() {
        let (mut t, who, i) = on_the_time_bank();
        t.set_sitting_out(&who, true, 30 * NANOS).unwrap();
        assert_eq!(t.action_deadline, 30 * NANOS);
        t.set_sitting_out(&who, false, 31 * NANOS).unwrap();
        assert_eq!(t.action_deadline, 50 * NANOS);
        t.set_sitting_out(&who, true, 40 * NANOS).unwrap();
        t.set_sitting_out(&who, false, 41 * NANOS).unwrap();
        assert_eq!(t.action_deadline, 50 * NANOS);
        assert!(t.time_bank_active);
        // Away time counts: acting at 45s used 25s of the bank.
        t.act(&who, Action::Call, 45 * NANOS).unwrap();
        assert_eq!(t.seat(i).time_bank, 5 * NANOS);
    }

    #[test]
    fn rapid_toggles_never_extend_the_bank() {
        let (mut t, who, i) = on_the_time_bank();
        let tenth = NANOS / 10;
        for k in 0..40u64 {
            let base = 21 * NANOS + k * NANOS;
            t.set_sitting_out(&who, true, base + tenth).unwrap();
            t.set_sitting_out(&who, false, base + 9 * tenth).unwrap();
            assert_eq!(t.action_deadline, 50 * NANOS, "cycle {k}");
        }
        // Back after the bank is gone: acted on at the next tick.
        assert_eq!(t.handle_timeout(61 * NANOS), Some(who));
        assert_eq!(t.seat(i).time_bank, 0);
    }

    #[test]
    fn rapid_toggle_then_act_charges_full_elapsed_time() {
        let (mut t, who, i) = on_the_time_bank();
        t.set_sitting_out(&who, true, 25 * NANOS).unwrap();
        t.set_sitting_out(&who, false, 26 * NANOS).unwrap();
        t.act(&who, Action::Call, 40 * NANOS).unwrap();
        assert_eq!(t.seat(i).time_bank, 10 * NANOS, "20s used since the bank started");
    }

    #[test]
    fn sitting_out_on_the_bank_acts_at_once_and_bank_runs_out_while_away() {
        let (mut t, who, i) = on_the_time_bank();
        t.set_sitting_out(&who, true, 30 * NANOS).unwrap();
        assert_eq!(t.handle_timeout(30 * NANOS), Some(who));
        assert_eq!(t.seat(i).time_bank, 20 * NANOS);
        // Away until past the bank end: comes back to an expired clock.
        let (mut t, who, _) = on_the_time_bank();
        t.set_sitting_out(&who, true, 30 * NANOS).unwrap();
        t.set_sitting_out(&who, false, 55 * NANOS).unwrap();
        assert_eq!(t.action_deadline, 50 * NANOS);
        assert_eq!(t.handle_timeout(55 * NANOS), Some(who));
    }

    #[test]
    fn one_seat_per_player_even_after_leaving_mid_hand() {
        let mut t = Table::new(1, config());
        t.sit(p(1), 0, 200).unwrap();
        t.sit(p(2), 1, 200).unwrap();
        t.sit(p(3), 2, 200).unwrap();
        t.start_hand([6; 32], 0).unwrap();
        t.stand(&p(1), 1).unwrap();
        assert!(t.sit(p(1), 3, 100).is_err(), "their old seat is still in this hand");
        while t.phase.is_betting() {
            let p = to_act_player(&t);
            t.act(&p, Action::Fold, 2).unwrap();
        }
        t.take_leavers();
        t.settle_after_hand();
        t.sit(p(1), 3, 200).unwrap();
    }

    #[test]
    fn time_bank_charges_only_what_was_used_and_refills() {
        let mut t = Table::new(1, config());
        t.sit(p(1), 0, 200).unwrap();
        t.sit(p(2), 1, 200).unwrap();
        t.start_hand([6; 32], 0).unwrap();
        let first = to_act_player(&t);
        let i = t.seat_of(&first).unwrap();
        assert_eq!(t.seat(i).time_bank, TIME_BANK_MAX, "starts full (refill is capped)");
        assert!(t.handle_timeout(20 * NANOS).is_none());
        // Acts 12 seconds into the bank.
        t.act(&first, Action::Call, 32 * NANOS).unwrap();
        assert_eq!(t.seat(i).time_bank, TIME_BANK_MAX - 12 * NANOS);
        assert!(!t.time_bank_active, "next player starts on regular time");
        assert_eq!(t.action_deadline, 52 * NANOS);
        // Finish the hand, then the next deal refills a little.
        while t.phase.is_betting() {
            let p = to_act_player(&t);
            if t.act(&p, Action::Check, 33 * NANOS).is_err() {
                t.act(&p, Action::Call, 33 * NANOS).unwrap();
            }
        }
        t.start_hand([7; 32], 40 * NANOS).unwrap();
        assert_eq!(t.seat(i).time_bank, TIME_BANK_MAX - 10 * NANOS);
        // Not sitting out: someone who used it but acted in time keeps playing.
        assert!(!t.seat(i).sitting_out);
    }

    #[test]
    fn random_hands_conserve_chips() {
        let mut t = Table::new(1, config());
        let mut bought_in = 0;
        let mut now = 0;
        let mut k: u64 = 42;
        for h in 0..500u32 {
            // Re-seat anyone who busted so the game keeps going.
            t.settle_after_hand();
            for seat in 0..6u8 {
                if t.seats[seat as usize].is_none() {
                    t.sit(p(seat + 1), seat, 1000).unwrap();
                    bought_in += 1000;
                }
            }
            let mut seed = [0u8; 32];
            seed[..4].copy_from_slice(&h.to_le_bytes());
            t.start_hand(seed, now).unwrap();
            while t.phase.is_betting() {
                k = k.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let i = t.to_act.unwrap();
                let pl = t.seat(i).player;
                let (_, min_to, max_to) = t.betting_options(i);
                let action = match (k >> 33) % 20 {
                    0..=2 => Action::Fold,
                    3 => Action::AllIn,
                    4..=6 => Action::Raise(min_to + (k >> 40) % (max_to - min_to + 1)),
                    7..=9 => Action::Check,
                    _ => Action::Call,
                };
                if t.act(&pl, action, now).is_err() {
                    t.act(&pl, Action::Call, now).unwrap();
                }
            }
            let r = t.last_result.as_ref().unwrap();
            assert_eq!(r.players.iter().map(|x| x.won).sum::<u64>(), r.pot - r.rake);
            let stacks: u64 = t.seats.iter().flatten().map(|s| s.stack).sum();
            assert_eq!(stacks, bought_in, "hand {h}");
            now += 10 * NANOS;
        }
        assert_eq!(t.hand_no, 500);
    }

    // --------------------------------------------------- stale betting commands

    const STALE: &str = "The table moved on";

    fn three_handed() -> Table {
        let mut t = Table::new(1, config());
        for k in 0..3 {
            t.sit(p(k + 1), k, 500).unwrap();
        }
        t.start_hand([3; 32], 0).unwrap();
        t
    }

    fn seq(t: &Table) -> u64 {
        t.action_seq.unwrap_or(0)
    }

    #[test]
    fn stale_call_from_an_earlier_street_is_rejected() {
        let mut t = three_handed();
        let who = to_act_player(&t);
        let old = seq(&t);
        t.act_for(&who, Action::Call, 1, Some(old)).unwrap();
        // Play on to the flop; the same command, delivered late, must not apply.
        while t.phase == Phase::PreFlop {
            let pl = to_act_player(&t);
            if t.act(&pl, Action::Check, 1).is_err() {
                t.act(&pl, Action::Call, 1).unwrap();
            }
        }
        let pl = to_act_player(&t);
        let before = (t.pot(), t.seat(t.to_act.unwrap()).stack, t.to_act);
        let err = t.act_for(&pl, Action::Call, 1, Some(old)).unwrap_err();
        assert!(err.contains(STALE), "{err}");
        assert!(t.act_for(&pl, Action::AllIn, 1, Some(old)).unwrap_err().contains(STALE));
        assert_eq!(before, (t.pot(), t.seat(t.to_act.unwrap()).stack, t.to_act));
    }

    #[test]
    fn stale_fold_from_an_earlier_hand_is_rejected() {
        let mut t = three_handed();
        let old = seq(&t);
        let first = to_act_player(&t);
        t.act_for(&first, Action::Fold, 1, Some(old)).unwrap();
        let second = to_act_player(&t);
        t.act(&second, Action::Fold, 1).unwrap();
        assert_eq!(t.phase, Phase::HandOver);
        t.start_hand([4; 32], 2).unwrap();
        let pl = to_act_player(&t);
        let err = t.act_for(&pl, Action::Fold, 3, Some(old)).unwrap_err();
        assert!(err.contains(STALE), "{err}");
        assert!(!t.seat(t.seat_of(&pl).unwrap()).folded);
        assert_eq!(t.to_act, t.seat_of(&pl));
    }

    #[test]
    fn stale_command_does_not_charge_the_time_bank() {
        let mut t = three_handed();
        let old = seq(&t);
        let pl = to_act_player(&t);
        t.act(&pl, Action::Call, 1).unwrap();
        let next = to_act_player(&t);
        let i = t.seat_of(&next).unwrap();
        t.time_bank_active = true;
        t.bank_started_at = 0;
        let bank = t.seat(i).time_bank;
        assert!(t.act_for(&next, Action::Call, 5 * NANOS, Some(old)).is_err());
        assert_eq!(t.seat(i).time_bank, bank);
    }

    #[test]
    fn duplicate_delivery_applies_once() {
        let mut t = three_handed();
        let who = to_act_player(&t);
        let old = seq(&t);
        t.act_for(&who, Action::Call, 1, Some(old)).unwrap();
        let after = (t.pot(), t.to_act);
        for pl in [who, to_act_player(&t)] {
            assert!(t.act_for(&pl, Action::Call, 1, Some(old)).is_err());
        }
        assert_eq!((t.pot(), t.to_act), after);
    }

    #[test]
    fn current_revision_is_accepted() {
        let mut t = three_handed();
        let pl = to_act_player(&t);
        let s1 = seq(&t);
        t.act_for(&pl, Action::Call, 1, Some(s1)).unwrap();
        let pl = to_act_player(&t);
        let s2 = seq(&t);
        assert!(s2 > s1);
        t.act_for(&pl, Action::Call, 1, Some(s2)).unwrap();
        // None keeps the old behaviour.
        let pl = to_act_player(&t);
        t.act_for(&pl, Action::Call, 1, None).unwrap();
    }

    #[test]
    fn revision_changes_when_the_amount_to_call_changes() {
        let mut t = three_handed();
        let a = to_act_player(&t);
        let old = seq(&t);
        t.act(&a, Action::Raise(40), 1).unwrap();
        let b = to_act_player(&t);
        assert_ne!(seq(&t), old);
        // The next player's revision predates the raise: their Call for 10 is rejected.
        assert!(t.act_for(&b, Action::Call, 1, Some(old)).unwrap_err().contains(STALE));
        // Each action, and an auto-act, moves it on too.
        let s1 = seq(&t);
        t.act(&b, Action::Call, 1).unwrap();
        assert_ne!(seq(&t), s1);
        let c = to_act_player(&t);
        let s2 = seq(&t);
        let c_seat = t.seat_of(&c).unwrap();
        t.seat_mut(c_seat).sitting_out = true;
        t.handle_timeout(t.action_deadline + 1);
        assert_ne!(seq(&t), s2);
    }
}

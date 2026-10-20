//! Sit-and-go and scheduled multi-table tournaments.
//!
//! A tournament runs on ordinary tables in `tournament_mode`. Stacks on those
//! tables are tournament chips, not ICP or play chips: real value lives in the
//! tournament's prize pool and is paid out to wallets as players finish.

use crate::state::{Channel, MessageKind, State, E8S, R};
use crate::table::{Currency, Table, TableConfig};
use candid::{CandidType, Deserialize, Principal};
use sha2::{Digest, Sha256};

const NANOS: u64 = 1_000_000_000;
/// Finished tournament tables stay viewable this long before being removed.
const CLOSE_AFTER_NS: u64 = 60 * NANOS;
/// How long a busted player has to rebuy before being knocked out.
pub const REBUY_WINDOW_NS: u64 = 30 * NANOS;

#[derive(CandidType, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum TournamentKind {
    /// Starts as soon as it fills up.
    SitAndGo,
    /// Starts at a set time if enough players registered.
    Scheduled,
}

#[derive(CandidType, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum TournamentStatus {
    Registering,
    Running,
    Finished,
    Cancelled,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TournamentTemplate {
    pub name: String,
    pub kind: TournamentKind,
    pub currency: Currency,
    /// Goes into the prize pool.
    pub buy_in: u64,
    /// Goes to the house (ICP) or leaves circulation (chips).
    pub fee: u64,
    /// Minimum prize pool, only for play-chip events (the house mints chips).
    pub guarantee: u64,
    pub starting_stack: u64,
    pub min_players: u32,
    pub max_players: u32,
    pub table_size: u8,
    pub level_secs: u64,
    /// Scheduled events: time between runs.
    pub interval_secs: u64,
    /// Registration stays open while the level (0-based) is below this. 0 = no late registration.
    pub late_reg_levels: u32,
    /// Rebuys are allowed while the level (0-based) is below this. 0 = freezeout.
    pub rebuy_levels: u32,
    pub max_rebuys: u32,
    /// One-time add-on at the end of the rebuy period: price (into the pool, no fee)
    /// and the chips it buys. 0 = no add-on.
    pub addon_cost: u64,
    pub addon_chips: u64,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Entrant {
    pub player: Principal,
    pub place: Option<u32>,
    pub prize: u64,
    pub rebuys: u32,
    pub addon: bool,
    /// Busted during the rebuy period: knocked out at this time unless they rebuy.
    pub rebuy_deadline: u64,
    /// Knocked out while late registration was still open. Their place depends
    /// on how many more players enter, so it is assigned (and paid) when late
    /// registration closes. Higher = busted later = finishes higher.
    pub out_seq: Option<u32>,
}

impl Entrant {
    /// Still playing: not placed and not waiting for a place.
    pub fn active(&self) -> bool {
        self.place.is_none() && self.out_seq.is_none()
    }
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Tournament {
    pub id: u64,
    pub template_index: u32,
    pub template: TournamentTemplate,
    pub status: TournamentStatus,
    /// Scheduled start time (ns); zero for sit-and-go's.
    pub start_at: u64,
    pub started_at: u64,
    pub finished_at: u64,
    pub level: u32,
    pub level_started_at: u64,
    pub entrants: Vec<Entrant>,
    pub tables: Vec<u64>,
    /// Prize pool: entries collected, or the guarantee if larger (play-chip events).
    pub prize_pool: u64,
    /// Buy-ins collected from registrations, late registrations and rebuys.
    pub collected: u64,
    /// Prize for each finishing place, 1st first.
    pub payouts: Vec<u64>,
    pub paid: u64,
    /// ICP entry fees from players registered before the start. They stay
    /// refundable (and counted as owed) until the event starts, when they move
    /// to the treasury.
    pub fees_held: u64,
}

// ------------------------------------------------------------------- views

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TournamentSummary {
    pub id: u64,
    pub name: String,
    pub kind: TournamentKind,
    pub currency: Currency,
    pub status: TournamentStatus,
    pub buy_in: u64,
    pub fee: u64,
    pub starting_stack: u64,
    pub start_at: u64,
    pub registered: u32,
    pub min_players: u32,
    pub max_players: u32,
    pub players_left: u32,
    pub prize_pool: u64,
    pub level_secs: u64,
    pub registered_me: bool,
    /// The table the viewer is playing at, if any.
    pub my_table: Option<u64>,
    /// Registration is open right now (before the start, or during late registration).
    pub can_register: bool,
    pub late_reg_levels: u32,
    pub rebuy_levels: u32,
    pub max_rebuys: u32,
    pub addon_cost: u64,
    pub addon_chips: u64,
    /// Total buy-ins: registrations plus rebuys.
    pub entries: u32,
    pub level: u32,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Standing {
    pub player: Principal,
    pub username: String,
    pub avatar: String,
    pub chips: u64,
    pub place: Option<u32>,
    pub prize: u64,
    pub table: Option<u64>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TournamentDetail {
    pub summary: TournamentSummary,
    pub level: u32,
    pub small_blind: u64,
    pub big_blind: u64,
    pub next_level_at: u64,
    pub payouts: Vec<u64>,
    pub standings: Vec<Standing>,
    pub tables: Vec<u64>,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct TournamentBadge {
    pub id: u64,
    pub name: String,
    pub level: u32,
    pub small_blind: u64,
    pub big_blind: u64,
    pub next_level_at: u64,
    pub players_left: u32,
    pub entrants: u32,
    pub my_place: Option<u32>,
    pub my_prize: u64,
    pub finished: bool,
    /// The viewer may rebuy right now.
    pub can_rebuy: bool,
    /// Busted viewer's deadline to rebuy (0 if not waiting).
    pub rebuy_deadline: u64,
    pub rebuy_cost: u64,
    pub rebuys_left: u32,
    pub currency: Currency,
    /// The viewer may take the add-on right now.
    pub can_addon: bool,
    pub addon_cost: u64,
    pub addon_chips: u64,
}

// ----------------------------------------------------------------- structure

/// Big blind multipliers (x2) of the level-1 big blind.
const LEVELS_X2: &[u64] = &[2, 3, 4, 6, 8, 10, 15, 20, 30, 40, 60, 80, 100, 150, 200, 300, 400, 600, 800, 1000];

/// Blinds at a level (0-based). Level 1 big blind is 1/75 of the starting stack.
pub fn blinds_at(level: u32, starting_stack: u64) -> (u64, u64) {
    let base = (starting_stack / 75).max(2);
    let l = level as usize;
    let mut bb = if l < LEVELS_X2.len() {
        base * LEVELS_X2[l] / 2
    } else {
        let mut bb = base * LEVELS_X2[LEVELS_X2.len() - 1] / 2;
        for _ in LEVELS_X2.len()..=l {
            bb = bb * 3 / 2;
        }
        bb
    };
    bb = bb.max(2) & !1; // even, so the small blind is exactly half
    (bb / 2, bb)
}

/// Share of the prize pool (basis points) for each paid place.
pub fn payout_bps(entrants: usize) -> &'static [u64] {
    match entrants {
        0..=2 => &[10_000],
        3..=6 => &[6_500, 3_500],
        7..=10 => &[5_000, 3_000, 2_000],
        11..=20 => &[4_000, 2_500, 1_600, 1_100, 800],
        21..=50 => &[3_000, 2_000, 1_400, 1_000, 800, 650, 500, 350, 300],
        _ => &[2_500, 1_600, 1_150, 850, 700, 600, 500, 400, 350, 300, 250, 200, 200, 200, 200],
    }
}

pub fn payouts_for(pool: u64, entrants: usize) -> Vec<u64> {
    let bps = payout_bps(entrants);
    let mut v: Vec<u64> = bps.iter().map(|b| (pool as u128 * *b as u128 / 10_000) as u64).collect();
    let rest = pool - v.iter().sum::<u64>();
    v[0] += rest;
    v
}

pub fn default_templates() -> Vec<TournamentTemplate> {
    use Currency::*;
    use TournamentKind::*;
    let sng = |name: &str, currency, buy_in, fee, players, table_size| TournamentTemplate {
        late_reg_levels: 0,
        rebuy_levels: 0,
        max_rebuys: 0,
        addon_cost: 0,
        addon_chips: 0,
        name: name.into(),
        kind: SitAndGo,
        currency,
        buy_in,
        fee,
        guarantee: 0,
        starting_stack: 1_500,
        min_players: players,
        max_players: players,
        table_size,
        level_secs: 180,
        interval_secs: 0,
    };
    vec![
        sng("Chip Sit & Go · 6 max", Chips, 1_000, 0, 6, 6),
        sng("Chip Heads-Up Duel", Chips, 500, 0, 2, 2),
        sng("Chip Sit & Go · 9 max", Chips, 2_500, 0, 9, 9),
        sng("ICP Sit & Go · 6 max", Icp, E8S / 10, E8S / 200, 6, 6),
        sng("ICP Heads-Up Duel", Icp, E8S / 20, E8S / 400, 2, 2),
        TournamentTemplate {
            name: "Hourly Freeroll".into(),
            kind: Scheduled,
            currency: Chips,
            buy_in: 0,
            fee: 0,
            guarantee: 50_000,
            starting_stack: 3_000,
            min_players: 2,
            max_players: 180,
            table_size: 9,
            level_secs: 300,
            interval_secs: 3_600,
            late_reg_levels: 3,
            rebuy_levels: 0,
            max_rebuys: 0,
            addon_cost: 0,
            addon_chips: 0,
        },
        TournamentTemplate {
            name: "Chip Rebuy Madness".into(),
            kind: Scheduled,
            currency: Chips,
            buy_in: 2_000,
            fee: 0,
            guarantee: 25_000,
            starting_stack: 3_000,
            min_players: 2,
            max_players: 180,
            table_size: 9,
            level_secs: 300,
            interval_secs: 7_200,
            late_reg_levels: 4,
            rebuy_levels: 4,
            max_rebuys: 3,
            addon_cost: 2_000,
            addon_chips: 4_500,
        },
        TournamentTemplate {
            name: "ICP Nightly".into(),
            kind: Scheduled,
            currency: Icp,
            buy_in: E8S / 2,
            fee: E8S / 40,
            guarantee: 0,
            starting_stack: 5_000,
            min_players: 3,
            max_players: 360,
            table_size: 9,
            level_secs: 480,
            interval_secs: 86_400,
            late_reg_levels: 4,
            rebuy_levels: 3,
            max_rebuys: 2,
            addon_cost: E8S / 2,
            addon_chips: 7_500,
        },
    ]
}

pub fn validate_template(t: &TournamentTemplate) -> R<()> {
    if t.name.trim().is_empty() || t.name.len() > 40 {
        return Err("Name must be 1-40 characters".into());
    }
    if !(2..=10).contains(&t.table_size) || t.min_players < 2 || t.max_players < t.min_players || t.max_players > 1_000 {
        return Err("Invalid player limits".into());
    }
    if t.kind == TournamentKind::SitAndGo && t.max_players > t.table_size as u32 {
        return Err("Sit-and-go's are single table".into());
    }
    if t.kind == TournamentKind::Scheduled && t.interval_secs < 600 {
        return Err("Scheduled events need an interval of at least 10 minutes".into());
    }
    if t.currency == Currency::Icp && t.guarantee > 0 {
        return Err("Guarantees are only available for play-chip events".into());
    }
    if t.kind == TournamentKind::SitAndGo && (t.late_reg_levels > 0 || t.rebuy_levels > 0) {
        return Err("Sit-and-go's have no late registration or rebuys".into());
    }
    if t.late_reg_levels > 20 || t.rebuy_levels > 20 || t.max_rebuys > 10 || (t.rebuy_levels > 0) != (t.max_rebuys > 0) {
        return Err("Invalid late registration or rebuy settings".into());
    }
    if (t.addon_cost > 0) != (t.addon_chips > 0) || (t.addon_cost > 0 && t.rebuy_levels == 0) {
        return Err("An add-on needs a price, a chip amount and a rebuy period".into());
    }
    if t.starting_stack < 150 || !(30..=3_600).contains(&t.level_secs) {
        return Err("Invalid structure".into());
    }
    Ok(())
}

impl Tournament {
    fn players_left(&self) -> u32 {
        self.entrants.iter().filter(|e| e.active()).count() as u32
    }

    fn blinds(&self) -> (u64, u64) {
        blinds_at(self.level, self.template.starting_stack)
    }

    fn next_level_at(&self) -> u64 {
        self.level_started_at + self.template.level_secs * NANOS
    }

    fn late_reg_open(&self) -> bool {
        self.status == TournamentStatus::Running && self.level < self.template.late_reg_levels
    }

    fn rebuys_open(&self) -> bool {
        self.status == TournamentStatus::Running && self.level < self.template.rebuy_levels
    }

    /// Add-ons are sold during the last rebuy level and the level after it.
    fn addon_open(&self) -> bool {
        let r = self.template.rebuy_levels;
        self.status == TournamentStatus::Running && self.template.addon_cost > 0 && self.level + 1 >= r && self.level <= r
    }

    pub fn entries(&self) -> u32 {
        self.entrants.iter().map(|e| 1 + e.rebuys).sum()
    }

    /// Pool and payouts follow the entries; chip events never pay less than the guarantee.
    fn recompute_payouts(&mut self) {
        self.prize_pool = if self.template.currency == Currency::Chips {
            self.collected.max(self.template.guarantee)
        } else {
            self.collected
        };
        self.payouts = payouts_for(self.prize_pool, self.entrants.len());
    }
}

impl State {
    fn tournament_mut(&mut self, id: u64) -> R<&mut Tournament> {
        self.tournaments.get_mut(&id).ok_or_else(|| "No such tournament".to_string())
    }

    fn seat_table_of(&self, t: &Tournament, p: &Principal) -> Option<u64> {
        t.tables.iter().copied().find(|tid| self.tables.get(tid).is_some_and(|tb| tb.seat_of(p).is_some()))
    }

    /// Opens a registering instance for every template that doesn't have one.
    pub fn ensure_tournaments(&mut self, now: u64) {
        for (idx, tpl) in self.templates.clone().into_iter().enumerate() {
            let open = self
                .tournaments
                .values()
                .any(|t| t.template_index == idx as u32 && t.status == TournamentStatus::Registering);
            if open {
                continue;
            }
            let start_at = match tpl.kind {
                TournamentKind::SitAndGo => 0,
                TournamentKind::Scheduled => {
                    let interval = tpl.interval_secs * NANOS;
                    (now / interval + 1) * interval
                }
            };
            let id = self.next_tournament_id;
            self.next_tournament_id += 1;
            self.tournaments.insert(
                id,
                Tournament {
                    id,
                    template_index: idx as u32,
                    prize_pool: 0,
                    collected: 0,
                    fees_held: 0,
                    template: tpl,
                    status: TournamentStatus::Registering,
                    start_at,
                    started_at: 0,
                    finished_at: 0,
                    level: 0,
                    level_started_at: 0,
                    entrants: vec![],
                    tables: vec![],
                    payouts: vec![],
                    paid: 0,
                },
            );
        }
        // Forget old finished events (keep the most recent 200).
        let done: Vec<u64> = self
            .tournaments
            .values()
            .filter(|t| matches!(t.status, TournamentStatus::Finished | TournamentStatus::Cancelled))
            .map(|t| t.id)
            .collect();
        if done.len() > 200 {
            for id in &done[..done.len() - 200] {
                self.tournaments.remove(id);
            }
        }
    }

    pub fn register_tournament(&mut self, caller: Principal, id: u64, now: u64) -> R<()> {
        self.user(&caller)?;
        self.check_not_banned(&caller, now)?;
        if self.pause.joins {
            return Err(self.paused_msg("Tournament registrations"));
        }
        let t = self.tournaments.get(&id).ok_or("No such tournament")?;
        let late = t.late_reg_open();
        if t.status != TournamentStatus::Registering && !late {
            return Err("Registration is closed".into());
        }
        if t.entrants.iter().any(|e| e.player == caller) {
            return Err("You are already registered".into());
        }
        if t.entrants.len() as u32 >= t.template.max_players {
            return Err("Tournament is full".into());
        }
        let (currency, buy_in, fee) = (t.template.currency, t.template.buy_in, t.template.fee);
        self.debit_in(&caller, currency, buy_in.checked_add(fee).ok_or("Amount too large")?)?;
        if currency == Currency::Icp {
            if late {
                // Late entries can't be refunded: the fee is the house's at once.
                self.treasury += fee;
            } else {
                self.tournament_mut(id)?.fees_held += fee;
            }
        }
        let t = self.tournament_mut(id)?;
        t.collected += buy_in;
        t.prize_pool = t.collected;
        t.entrants.push(Entrant {
            player: caller,
            place: None,
            prize: 0,
            rebuys: 0,
            addon: false,
            rebuy_deadline: 0,
            out_seq: None,
        });
        if late {
            t.recompute_payouts();
            let stack = t.template.starting_stack;
            let tid = self.seat_entrant(id, caller, stack);
            let name = self.users.get(&caller).map(|u| u.username.clone()).unwrap_or_default();
            self.post(Channel::Table(tid), None, String::new(), format!("{name} joins late"), MessageKind::System, now);
            return Ok(());
        }
        let t = self.tournament_mut(id)?;
        if t.template.kind == TournamentKind::SitAndGo && t.entrants.len() as u32 == t.template.max_players {
            self.start_tournament(id, now);
        }
        Ok(())
    }

    pub fn unregister_tournament(&mut self, caller: Principal, id: u64) -> R<()> {
        let t = self.tournament_mut(id)?;
        if t.status != TournamentStatus::Registering {
            return Err("The tournament has already started".into());
        }
        let pos = t.entrants.iter().position(|e| e.player == caller).ok_or("You are not registered")?;
        t.entrants.remove(pos);
        t.collected = t.collected.saturating_sub(t.template.buy_in);
        t.prize_pool = t.collected;
        self.refund_entry(id, &caller);
        Ok(())
    }

    /// Returns a pre-start entry: the buy-in and the fee held for it.
    fn refund_entry(&mut self, id: u64, p: &Principal) {
        let Some(t) = self.tournaments.get_mut(&id) else { return };
        let (currency, buy_in, mut fee) = (t.template.currency, t.template.buy_in, t.template.fee);
        if currency == Currency::Icp {
            fee = fee.min(t.fees_held);
            t.fees_held -= fee;
        }
        let _ = self.credit_in(p, currency, buy_in + fee);
    }

    fn start_tournament(&mut self, id: u64, now: u64) {
        let t = self.tournaments.get(&id).unwrap().clone();
        if (t.entrants.len() as u32) < t.template.min_players {
            for e in &t.entrants {
                self.refund_entry(id, &e.player);
            }
            let t = self.tournaments.get_mut(&id).unwrap();
            t.status = TournamentStatus::Cancelled;
            t.prize_pool = 0;
            t.collected = 0;
            t.finished_at = now;
            return;
        }
        let tpl = &t.template;
        let n = t.entrants.len();

        // Random seating, from secret on-chain randomness (not just the start
        // time), so nobody can work out or steer where they'll sit.
        let entropy = self.entropy().to_vec();
        let mut order: Vec<Principal> = t.entrants.iter().map(|e| e.player).collect();
        order.sort_by_key(|p| {
            let mut h = Sha256::new();
            h.update(&entropy);
            h.update(now.to_le_bytes());
            h.update(id.to_le_bytes());
            h.update(p.as_slice());
            h.finalize().to_vec()
        });

        let size = tpl.table_size as usize;
        let n_tables = n.div_ceil(size);
        let named = n_tables > 1 || tpl.late_reg_levels > 0;
        let table_ids: Vec<u64> = (0..n_tables).map(|k| self.new_tournament_table(id, &t.template, 0, named.then_some(k + 1))).collect();
        // Deal players round-robin so tables are balanced, spread around each table.
        for (k, tid) in table_ids.iter().enumerate() {
            let group: Vec<&Principal> = order.iter().skip(k).step_by(n_tables).collect();
            let table = self.tables.get_mut(tid).unwrap();
            for (j, p) in group.iter().enumerate() {
                table.seat_player(**p, j * size / group.len(), tpl.starting_stack).unwrap();
            }
        }
        for tid in &table_ids {
            self.post(Channel::Table(*tid), None, String::new(), format!("{} has started. Good luck!", tpl.name), MessageKind::System, now);
        }
        let t = self.tournaments.get_mut(&id).unwrap();
        t.status = TournamentStatus::Running;
        // Entries are final now: the fees become house revenue.
        let fees = std::mem::take(&mut t.fees_held);
        t.started_at = now;
        t.level = 0;
        t.level_started_at = now;
        t.tables = table_ids;
        t.recompute_payouts();
        let name = t.template.name.clone();
        self.fund_rewards(fees);
        self.post(Channel::Lobby, None, String::new(), format!("🏁 {name} is under way with {n} players"), MessageKind::System, now);
    }

    fn new_tournament_table(&mut self, id: u64, tpl: &TournamentTemplate, level: u32, number: Option<usize>) -> u64 {
        let tid = self.next_table_id;
        self.next_table_id += 1;
        let (sb, bb) = blinds_at(level, tpl.starting_stack);
        let cfg = TableConfig {
            name: match number {
                Some(k) => format!("{} · Table {}", tpl.name, k),
                None => tpl.name.clone(),
            },
            currency: Currency::Chips,
            small_blind: sb,
            big_blind: bb,
            min_buy_in: tpl.starting_stack,
            max_buy_in: tpl.starting_stack,
            max_seats: tpl.table_size,
            rake_bps: 0,
            rake_cap: 0,
            // Tournaments don't feed (or win) the cash-game jackpot.
            jackpot_bps: 0,
            jackpot_cap: 0,
            action_timeout_secs: 20,
        };
        let mut table = Table::new(tid, cfg);
        table.tournament_mode = true;
        self.tables.insert(tid, table);
        self.table_tournament.insert(tid, id);
        tid
    }

    /// Seats a (late or re-entering) player at the emptiest table with room,
    /// opening a new table when all are full. Returns the table id.
    fn seat_entrant(&mut self, id: u64, p: Principal, stack: u64) -> u64 {
        let t = &self.tournaments[&id];
        let dest = t
            .tables
            .iter()
            .filter_map(|tid| self.tables.get(tid).map(|tb| (*tid, tb)))
            .filter(|(_, tb)| !tb.free_seats().is_empty())
            .min_by_key(|(_, tb)| tb.seats.iter().flatten().filter(|s| s.stack > 0).count())
            .map(|(tid, _)| tid);
        let tid = match dest {
            Some(tid) => tid,
            None => {
                let (tpl, level, k) = (t.template.clone(), t.level, t.tables.len() + 1);
                let tid = self.new_tournament_table(id, &tpl, level, Some(k));
                self.tournaments.get_mut(&id).unwrap().tables.push(tid);
                tid
            }
        };
        let table = self.tables.get_mut(&tid).unwrap();
        let seat = table.free_seats()[0];
        table.seat_player(p, seat, stack).unwrap();
        tid
    }

    /// Buys a fresh starting stack during the rebuy period: when busted (within
    /// the rebuy window) or when at or below the starting stack between hands.
    pub fn rebuy(&mut self, caller: Principal, id: u64, now: u64) -> R<()> {
        self.check_not_banned(&caller, now)?;
        let t = self.tournaments.get(&id).ok_or("No such tournament")?;
        if !t.rebuys_open() {
            return Err("The rebuy period is over".into());
        }
        let e = t.entrants.iter().find(|e| e.player == caller).ok_or("You are not in this tournament")?;
        if !e.active() {
            return Err("You are out of this tournament".into());
        }
        if e.rebuys >= t.template.max_rebuys {
            return Err("You have used all your rebuys".into());
        }
        let stack = t.template.starting_stack;
        let table = self.seat_table_of(t, &caller);
        let chips = table
            .and_then(|tid| self.tables[&tid].seat_of(&caller).and_then(|i| self.tables[&tid].seats[i].as_ref()).map(|s| s.stack))
            .unwrap_or(0);
        if e.rebuy_deadline == 0 && chips > stack {
            return Err("You can rebuy when you're at or below the starting stack".into());
        }
        // Rebuys cost the buy-in only: the house fee is charged once, on entry.
        let (currency, buy_in) = (t.template.currency, t.template.buy_in);
        self.buy_chips(&caller, table, stack, currency, buy_in)?;
        let t = self.tournaments.get_mut(&id).unwrap();
        t.collected += buy_in;
        let e = t.entrants.iter_mut().find(|e| e.player == caller).unwrap();
        e.rebuys += 1;
        e.rebuy_deadline = 0;
        t.recompute_payouts();
        let tid = match table {
            Some(tid) => tid,
            None => self.seat_entrant(id, caller, stack),
        };
        let name = self.users.get(&caller).map(|u| u.username.clone()).unwrap_or_default();
        self.post(Channel::Table(tid), None, String::new(), format!("{name} rebuys"), MessageKind::System, now);
        Ok(())
    }

    /// Adds `chips` to the caller's seat (if seated) and charges `cost`, undoing
    /// the chips if the charge fails. Chips go on first so a mid-hand player
    /// is refused before anything is charged.
    fn buy_chips(&mut self, caller: &Principal, table: Option<u64>, chips: u64, currency: Currency, cost: u64) -> R<()> {
        if let Some(tid) = table {
            self.tables.get_mut(&tid).unwrap().add_chips(caller, chips)?;
        }
        if let Err(e) = self.debit_in(caller, currency, cost) {
            if let Some(tid) = table {
                let tb = self.tables.get_mut(&tid).unwrap();
                let i = tb.seat_of(caller).unwrap();
                tb.seats[i].as_mut().unwrap().stack -= chips;
            }
            return Err(e);
        }
        Ok(())
    }

    /// One-time add-on at the end of the rebuy period. The whole price goes into the pool.
    pub fn addon(&mut self, caller: Principal, id: u64, now: u64) -> R<()> {
        self.check_not_banned(&caller, now)?;
        let t = self.tournaments.get(&id).ok_or("No such tournament")?;
        if t.template.addon_cost == 0 {
            return Err("This tournament has no add-on".into());
        }
        if !t.addon_open() {
            return Err(format!("The add-on is available in levels {} and {}", t.template.rebuy_levels, t.template.rebuy_levels + 1));
        }
        let e = t.entrants.iter().find(|e| e.player == caller).ok_or("You are not in this tournament")?;
        if !e.active() {
            return Err("You are out of this tournament".into());
        }
        if e.addon {
            return Err("You already took the add-on".into());
        }
        if e.rebuy_deadline != 0 {
            return Err("You're busted: rebuy first, then take the add-on".into());
        }
        let table = self.seat_table_of(t, &caller).ok_or("You are not seated")?;
        let (currency, cost, chips) = (t.template.currency, t.template.addon_cost, t.template.addon_chips);
        self.buy_chips(&caller, Some(table), chips, currency, cost)?;
        let t = self.tournaments.get_mut(&id).unwrap();
        t.collected += cost;
        t.entrants.iter_mut().find(|e| e.player == caller).unwrap().addon = true;
        t.recompute_payouts();
        let name = self.users.get(&caller).map(|u| u.username.clone()).unwrap_or_default();
        self.post(Channel::Table(table), None, String::new(), format!("{name} takes the add-on (+{chips} chips)"), MessageKind::System, now);
        Ok(())
    }

    /// Ends the tournament when one player is left standing.
    fn check_finished(&mut self, id: u64, table_id: u64, now: u64) -> bool {
        let t = &self.tournaments[&id];
        if t.status != TournamentStatus::Running || t.players_left() != 1 {
            return false;
        }
        let winner = t.entrants.iter().find(|e| e.active()).unwrap().player;
        // Players waiting on a late-registration place finish 2nd, 3rd, ...
        self.place_late_busts(id, Some(table_id), now);
        self.finish_player(id, &winner, 1, Some(table_id), now);
        let t = self.tournaments.get_mut(&id).unwrap();
        t.status = TournamentStatus::Finished;
        t.finished_at = now;
        let kind = t.template.kind;
        let name = t.template.name.clone();
        let winner_name = self.users.get(&winner).map(|u| u.username.clone()).unwrap_or_default();
        self.post(Channel::Lobby, None, String::new(), format!("🏆 {winner_name} won the {name}!"), MessageKind::System, now);
        let medal = if kind == TournamentKind::SitAndGo { "sng_champ" } else { "champion" };
        self.award(&winner, medal, Some(Channel::Table(table_id)), now);
        true
    }

    /// Knocks out busted players whose rebuy window has run out.
    fn expire_rebuy_windows(&mut self, id: u64, now: u64) {
        let expired: Vec<Principal> = self.tournaments[&id]
            .entrants
            .iter()
            .filter(|e| e.active() && e.rebuy_deadline != 0 && e.rebuy_deadline <= now)
            .map(|e| e.player)
            .collect();
        for p in expired {
            let t = &self.tournaments[&id];
            let table = self.seat_table_of(t, &p).or(t.tables.first().copied());
            self.tournaments.get_mut(&id).unwrap().entrants.iter_mut().find(|e| e.player == p).unwrap().rebuy_deadline = 0;
            self.knock_out(id, &p, table, now);
            if let Some(tid) = table {
                if self.check_finished(id, tid, now) {
                    return;
                }
            }
        }
    }

    /// Runs every tick: opens new events, starts scheduled ones, raises blinds,
    /// keeps tables balanced and removes finished tables.
    pub fn tournament_tick(&mut self, now: u64) {
        self.ensure_tournaments(now);
        let ids: Vec<u64> = self.tournaments.keys().copied().collect();
        for id in ids {
            let t = &self.tournaments[&id];
            match t.status {
                TournamentStatus::Registering if t.template.kind == TournamentKind::Scheduled && now >= t.start_at => {
                    self.start_tournament(id, now);
                }
                TournamentStatus::Running => {
                    if now >= t.next_level_at() {
                        // Catch up on every level that has elapsed (levels stay on schedule).
                        let t = self.tournaments.get_mut(&id).unwrap();
                        while now >= t.next_level_at() {
                            t.level += 1;
                            t.level_started_at += t.template.level_secs * NANOS;
                        }
                        let (sb, bb) = t.blinds();
                        let tables = t.tables.clone();
                        let notice = if t.template.addon_cost > 0 && t.level + 1 == t.template.rebuy_levels {
                            Some(format!("Last rebuy level: the add-on (+{} chips) is now available", t.template.addon_chips))
                        } else if t.template.rebuy_levels > 0 && t.level == t.template.rebuy_levels {
                            Some(if t.template.addon_cost > 0 {
                                "Rebuys are closed. The add-on is available until the end of this level".to_string()
                            } else {
                                "Rebuys are closed".to_string()
                            })
                        } else {
                            None
                        };
                        for tid in tables {
                            if let Some(tb) = self.tables.get_mut(&tid) {
                                tb.config.small_blind = sb;
                                tb.config.big_blind = bb;
                                self.post(Channel::Table(tid), None, String::new(), format!("Blinds are now {sb}/{bb}"), MessageKind::System, now);
                                if let Some(n) = &notice {
                                    self.post(Channel::Table(tid), None, String::new(), n.clone(), MessageKind::System, now);
                                }
                            }
                        }
                    }
                    self.expire_rebuy_windows(id, now);
                    let t = &self.tournaments[&id];
                    if t.status == TournamentStatus::Running && !t.late_reg_open() {
                        self.place_late_busts(id, t.tables.first().copied(), now);
                    }
                    if self.tournaments[&id].status == TournamentStatus::Running {
                        self.balance_tables(id);
                    }
                }
                TournamentStatus::Finished if t.finished_at + CLOSE_AFTER_NS <= now && !t.tables.is_empty() => {
                    let tables = std::mem::take(&mut self.tournaments.get_mut(&id).unwrap().tables);
                    for tid in tables {
                        self.tables.remove(&tid);
                        self.table_tournament.remove(&tid);
                        self.applied_hand.remove(&tid);
                        self.throws.remove(&tid);
                        self.chats.remove(&Channel::Table(tid));
                    }
                }
                _ => {}
            }
        }
    }

    /// After a hand at a tournament table: knocks out busted players, pays
    /// prizes, ends the event or rebalances tables.
    pub fn after_tournament_hand(&mut self, table_id: u64, now: u64) {
        let Some(&id) = self.table_tournament.get(&table_id) else { return };
        let Some(result) = self.tables.get(&table_id).and_then(|t| t.last_result.clone()) else { return };
        let t = &self.tournaments[&id];
        if t.status != TournamentStatus::Running {
            return;
        }
        let tb = &self.tables[&table_id];
        // Busted this hand: larger starting stack finishes higher.
        let mut busted: Vec<(u64, Principal)> = tb
            .seats
            .iter()
            .flatten()
            .filter(|s| s.stack == 0 && s.in_hand)
            .filter(|s| t.entrants.iter().any(|e| e.player == s.player && e.active()))
            .map(|s| (result.players.iter().find(|r| r.player == s.player).map_or(0, |r| r.committed), s.player))
            .collect();
        busted.sort();
        let rebuys_open = t.rebuys_open();
        let max_rebuys = t.template.max_rebuys;
        for (_, p) in busted {
            let t = self.tournaments.get_mut(&id).unwrap();
            let e = t.entrants.iter_mut().find(|e| e.player == p).unwrap();
            if e.rebuy_deadline != 0 {
                continue;
            }
            if rebuys_open && e.rebuys < max_rebuys {
                // Give them a moment to rebuy before knocking them out.
                e.rebuy_deadline = now + REBUY_WINDOW_NS;
                let name = self.users.get(&p).map(|u| u.username.clone()).unwrap_or_default();
                self.post(Channel::Table(table_id), None, String::new(), format!("{name} is busted and has 30 seconds to rebuy"), MessageKind::System, now);
                continue;
            }
            self.knock_out(id, &p, Some(table_id), now);
        }
        // Leave the final table seated so everyone sees the result; it closes shortly.
        if !self.check_finished(id, table_id, now) {
            self.balance_tables(id);
        }
    }

    /// Knocks a player out. While late registration is open, more entries can
    /// still arrive, so their place is decided (and paid) once it closes.
    pub(crate) fn knock_out(&mut self, id: u64, p: &Principal, table: Option<u64>, now: u64) {
        let t = self.tournaments.get_mut(&id).unwrap();
        if t.late_reg_open() {
            let seq = t.entrants.iter().filter_map(|e| e.out_seq).max().map_or(0, |m| m + 1);
            if let Some(e) = t.entrants.iter_mut().find(|e| e.player == *p) {
                e.out_seq = Some(seq);
            }
            let username = self.users.get(p).map(|u| u.username.clone()).unwrap_or_default();
            if let Some(tid) = table {
                let text = format!("{username} is out. Final places are set when late registration closes");
                self.post(Channel::Table(tid), None, String::new(), text, MessageKind::System, now);
            }
            return;
        }
        let left = t.players_left();
        self.finish_player(id, p, left, table, now);
    }

    /// Gives players knocked out during late registration their places, now
    /// that the field is final: the last one out finishes just below everyone
    /// still playing.
    fn place_late_busts(&mut self, id: u64, table: Option<u64>, now: u64) {
        let t = &self.tournaments[&id];
        let mut waiting: Vec<(u32, Principal)> = t.entrants.iter().filter(|e| e.place.is_none()).filter_map(|e| Some((e.out_seq?, e.player))).collect();
        if waiting.is_empty() {
            return;
        }
        waiting.sort_by_key(|w| std::cmp::Reverse(w.0));
        for (place, (_, p)) in (t.players_left() + 1..).zip(waiting) {
            let tb = self.seat_table_of(&self.tournaments[&id], &p).or(table);
            self.finish_player(id, &p, place, tb, now);
        }
    }

    fn finish_player(&mut self, id: u64, p: &Principal, place: u32, table: Option<u64>, now: u64) {
        let t = self.tournaments.get_mut(&id).unwrap();
        // The winner takes whatever is left, so the pool always pays out exactly
        // even if late entries changed the structure after someone cashed.
        let prize = if place == 1 {
            t.prize_pool - t.paid
        } else {
            t.payouts.get(place as usize - 1).copied().unwrap_or(0).min(t.prize_pool - t.paid)
        };
        let currency = t.template.currency;
        if let Some(e) = t.entrants.iter_mut().find(|e| e.player == *p) {
            e.place = Some(place);
            e.prize = prize;
        }
        t.paid += prize;
        let name = t.template.name.clone();
        let username = self.users.get(p).map(|u| u.username.clone()).unwrap_or_default();
        if prize > 0 {
            let _ = self.credit_in(p, currency, prize);
            self.award(p, "in_the_money", table.map(Channel::Table), now);
        }
        if let Some(tid) = table {
            let text = if prize > 0 {
                format!("{username} finishes #{place} in {name} and wins {}", crate::state::fmt_amount(currency, prize))
            } else {
                format!("{username} finishes #{place}")
            };
            self.post(Channel::Table(tid), None, String::new(), text, MessageKind::System, now);
        }
    }

    /// Breaks a table when the others have room for its players, otherwise
    /// moves players from the fullest table to the emptiest. Only moves players
    /// from tables that are between hands.
    fn balance_tables(&mut self, id: u64) {
        let size = self.tournaments[&id].template.table_size as usize;
        for _ in 0..size * 2 {
            let tables = self.tournaments[&id].tables.clone();
            let counts: Vec<(u64, usize)> = tables
                .iter()
                .filter_map(|tid| {
                    let tb = self.tables.get(tid)?;
                    Some((*tid, tb.seats.iter().flatten().filter(|s| s.stack > 0 && !s.left).count()))
                })
                .collect();
            if counts.len() < 2 {
                return;
            }
            let total: usize = counts.iter().map(|c| c.1).sum();
            let (min_t, min_c) = *counts.iter().min_by_key(|c| c.1).unwrap();
            let (max_t, max_c) = *counts.iter().max_by_key(|c| c.1).unwrap();

            if total <= (counts.len() - 1) * size {
                // Break the smallest table once it is between hands.
                if !self.tables[&min_t].between_hands() {
                    return;
                }
                let movers: Vec<Principal> = self.tables[&min_t]
                    .seats
                    .iter()
                    .flatten()
                    .filter(|s| s.stack > 0 && !s.left)
                    .map(|s| s.player)
                    .collect();
                for p in movers {
                    let dest = counts
                        .iter()
                        .filter(|c| c.0 != min_t)
                        .filter(|c| !self.tables[&c.0].free_seats().is_empty())
                        .min_by_key(|c| self.tables[&c.0].seats.iter().flatten().count())
                        .map(|c| c.0);
                    let Some(dest) = dest else { return };
                    self.move_player(&p, min_t, dest);
                }
                self.tables.remove(&min_t);
                self.table_tournament.remove(&min_t);
                self.tournaments.get_mut(&id).unwrap().tables.retain(|t| *t != min_t);
                continue;
            }
            if max_c >= min_c + 2 && self.tables[&max_t].between_hands() {
                let mover = self.tables[&max_t]
                    .seats
                    .iter()
                    .flatten()
                    .find(|s| s.stack > 0 && !s.left)
                    .map(|s| s.player);
                if let Some(p) = mover {
                    self.move_player(&p, max_t, min_t);
                    continue;
                }
            }
            return;
        }
    }

    fn move_player(&mut self, p: &Principal, from: u64, to: u64) {
        let away = self.tables[&from].seat_of(p).and_then(|i| self.tables[&from].seats[i].as_ref()).is_some_and(|s| s.sitting_out);
        let Ok(stack) = self.tables.get_mut(&from).unwrap().stand(p, 0) else { return };
        let dest = self.tables.get_mut(&to).unwrap();
        let seat = dest.free_seats()[0];
        dest.seat_player(*p, seat, stack).unwrap();
        if away {
            let _ = dest.set_sitting_out(p, true, 0);
        }
    }

    // ----------------------------------------------------------------- views

    fn summary(&self, t: &Tournament, viewer: Option<Principal>) -> TournamentSummary {
        TournamentSummary {
            id: t.id,
            name: t.template.name.clone(),
            kind: t.template.kind,
            currency: t.template.currency,
            status: t.status,
            buy_in: t.template.buy_in,
            fee: t.template.fee,
            starting_stack: t.template.starting_stack,
            start_at: t.start_at,
            registered: t.entrants.len() as u32,
            min_players: t.template.min_players,
            max_players: t.template.max_players,
            players_left: t.players_left(),
            prize_pool: if t.status == TournamentStatus::Registering && t.template.currency == Currency::Chips {
                t.prize_pool.max(t.template.guarantee)
            } else {
                t.prize_pool
            },
            level_secs: t.template.level_secs,
            registered_me: viewer.is_some_and(|v| t.entrants.iter().any(|e| e.player == v)),
            my_table: viewer.and_then(|v| self.seat_table_of(t, &v)),
            can_register: (t.status == TournamentStatus::Registering || t.late_reg_open())
                && (t.entrants.len() as u32) < t.template.max_players,
            late_reg_levels: t.template.late_reg_levels,
            rebuy_levels: t.template.rebuy_levels,
            max_rebuys: t.template.max_rebuys,
            addon_cost: t.template.addon_cost,
            addon_chips: t.template.addon_chips,
            entries: t.entries(),
            level: t.level + 1,
        }
    }

    /// Chips a player has in a tournament right now (stack plus anything in the pot).
    fn tournament_chips(&self, t: &Tournament, p: &Principal) -> u64 {
        self.seat_table_of(t, p)
            .and_then(|tid| {
                let tb = &self.tables[&tid];
                tb.seat_of(p).and_then(|i| tb.seats[i].as_ref()).map(|s| s.stack + s.committed * tb.phase.is_betting() as u64)
            })
            .unwrap_or(0)
    }

    pub fn list_tournaments(&self, viewer: Option<Principal>) -> Vec<TournamentSummary> {
        let mut v: Vec<TournamentSummary> = self
            .tournaments
            .values()
            .filter(|t| {
                matches!(t.status, TournamentStatus::Registering | TournamentStatus::Running)
                    || viewer.is_some_and(|v| t.entrants.iter().any(|e| e.player == v)) && t.status == TournamentStatus::Finished
            })
            .map(|t| self.summary(t, viewer))
            .collect();
        v.sort_by_key(|s| (s.status != TournamentStatus::Running, s.start_at, s.id));
        v
    }

    pub fn tournament_detail(&self, id: u64, viewer: Option<Principal>) -> Option<TournamentDetail> {
        let t = self.tournaments.get(&id)?;
        let mut standings: Vec<Standing> = t
            .entrants
            .iter()
            .map(|e| {
                let table = self.seat_table_of(t, &e.player);
                let chips = table
                    .and_then(|tid| {
                        let tb = &self.tables[&tid];
                        tb.seat_of(&e.player).and_then(|i| tb.seats[i].as_ref()).map(|s| s.stack + s.committed * tb.phase.is_betting() as u64)
                    })
                    .unwrap_or(0);
                let u = self.users.get(&e.player);
                Standing {
                    player: e.player,
                    username: u.as_ref().map(|u| u.username.clone()).unwrap_or_default(),
                    avatar: u.as_ref().map(|u| self.icon(u.avatar)).unwrap_or_default(),
                    chips: if e.active() { chips } else { 0 },
                    place: e.place,
                    prize: e.prize,
                    table: if e.active() { table } else { None },
                }
            })
            .collect();
        standings.sort_by_key(|s| (s.place.unwrap_or(0), std::cmp::Reverse(s.chips)));
        let (small_blind, big_blind) = t.blinds();
        Some(TournamentDetail {
            summary: self.summary(t, viewer),
            level: t.level + 1,
            small_blind,
            big_blind,
            next_level_at: if t.status == TournamentStatus::Running { t.next_level_at() } else { 0 },
            payouts: if t.payouts.is_empty() {
                payouts_for(self.summary(t, None).prize_pool, (t.entrants.len()).max(t.template.min_players as usize))
            } else {
                t.payouts.clone()
            },
            standings,
            tables: t.tables.clone(),
        })
    }

    pub fn tournament_badge(&self, id: u64, viewer: Option<Principal>) -> Option<TournamentBadge> {
        let t = self.tournaments.get(&id)?;
        let me = viewer.and_then(|v| t.entrants.iter().find(|e| e.player == v));
        let (small_blind, big_blind) = t.blinds();
        Some(TournamentBadge {
            id,
            name: t.template.name.clone(),
            level: t.level + 1,
            small_blind,
            big_blind,
            next_level_at: t.next_level_at(),
            players_left: t.players_left(),
            entrants: t.entrants.len() as u32,
            my_place: me.and_then(|e| e.place),
            my_prize: me.map_or(0, |e| e.prize),
            finished: t.status == TournamentStatus::Finished,
            can_rebuy: me.is_some_and(|e| {
                t.rebuys_open()
                    && e.active()
                    && e.rebuys < t.template.max_rebuys
                    && (e.rebuy_deadline != 0 || self.tournament_chips(t, &e.player) <= t.template.starting_stack)
            }),
            rebuy_deadline: me.map_or(0, |e| e.rebuy_deadline),
            rebuy_cost: t.template.buy_in,
            rebuys_left: me.map_or(0, |e| t.template.max_rebuys.saturating_sub(e.rebuys)),
            currency: t.template.currency,
            can_addon: me.is_some_and(|e| t.addon_open() && e.active() && !e.addon && e.rebuy_deadline == 0),
            addon_cost: t.template.addon_cost,
            addon_chips: t.template.addon_chips,
        })
    }

    /// ICP held for tournaments: entries not yet paid out as prizes.
    pub fn icp_in_tournaments(&self) -> u64 {
        self.tournaments
            .values()
            .filter(|t| t.template.currency == Currency::Icp)
            .filter(|t| matches!(t.status, TournamentStatus::Registering | TournamentStatus::Running))
            .map(|t| t.prize_pool.saturating_sub(t.paid) + t.fees_held)
            .sum()
    }

    pub fn add_template(&mut self, tpl: TournamentTemplate) -> R<u32> {
        validate_template(&tpl)?;
        self.templates.push(tpl);
        Ok(self.templates.len() as u32 - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blind_schedule_rises() {
        assert_eq!(blinds_at(0, 1_500), (10, 20));
        assert_eq!(blinds_at(1, 1_500), (15, 30));
        let mut last = 0;
        for l in 0..30 {
            let (sb, bb) = blinds_at(l, 1_500);
            assert_eq!(sb * 2, bb);
            assert!(bb > last, "level {l}");
            last = bb;
        }
    }

    #[test]
    fn payouts_sum_to_pool() {
        for n in [2usize, 3, 6, 9, 15, 40, 120, 500] {
            for pool in [0u64, 1, 999, 6_000, 123_456_789] {
                let p = payouts_for(pool, n);
                assert_eq!(p.iter().sum::<u64>(), pool);
                assert!(p.windows(2).all(|w| w[0] >= w[1]));
            }
        }
        for n in 2..600 {
            assert_eq!(payout_bps(n).iter().sum::<u64>(), 10_000);
        }
    }

    #[test]
    fn default_templates_are_valid() {
        for t in default_templates() {
            validate_template(&t).unwrap();
        }
    }
}

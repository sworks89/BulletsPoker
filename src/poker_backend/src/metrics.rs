//! Operational metrics, served as Prometheus text on `/metrics`.

use crate::state::State;
use crate::table::Currency;
use candid::{CandidType, Deserialize};
use serde_bytes::ByteBuf;

#[derive(CandidType, Deserialize, Clone, Debug, Default)]
pub struct Metrics {
    pub time: u64,
    pub users: u64,
    /// Players who made an update call in the last 5 minutes.
    pub active_players_5m: u64,
    pub seated_players: u64,
    /// Tables with at least two players seated.
    pub active_tables: u64,
    pub tables: u64,
    pub running_tournaments: u64,
    pub hands_dealt: u64,
    pub cycles: u64,
    pub heap_bytes: u64,
    pub stable_bytes: u64,
    pub treasury_e8s: u64,
    pub wallet_total_e8s: u64,
    pub liabilities_e8s: u64,
    pub jackpot_icp_e8s: u64,
    pub jackpot_chips: u64,
    pub unresolved_ledger_ops: u64,
    pub ledger_ops_failed: u64,
    pub ledger_ops_unknown: u64,
    pub last_solvency_ok: Option<bool>,
    pub last_solvency_at: Option<u64>,
    pub last_solvency_surplus_e8s: Option<i64>,
    pub paused_deposits: bool,
    pub paused_withdrawals: bool,
    pub paused_dealing: bool,
    pub paused_joins: bool,
    pub open_reports: u64,
    pub reports_filed: u64,
    pub bans: u64,
    pub idle_stand_ups: u64,
    pub rate_limited_calls: u64,
}

impl Metrics {
    /// Everything that comes from game state (the canister fills in the rest).
    pub fn from_state(s: &State, now: u64) -> Self {
        let seated = |t: &crate::table::Table| t.seats.iter().flatten().filter(|seat| !seat.left).count() as u64;
        let sol = s.last_solvency.as_ref();
        Metrics {
            time: now,
            users: s.users.len() as u64,
            seated_players: s.tables.values().map(seated).sum(),
            active_tables: s.tables.values().filter(|t| seated(t) >= 2).count() as u64,
            tables: s.tables.len() as u64,
            running_tournaments: s.tournaments.values().filter(|t| t.status == crate::tournament::TournamentStatus::Running).count() as u64,
            hands_dealt: s.counters.hands_dealt,
            treasury_e8s: s.treasury,
            wallet_total_e8s: s.wallet_total,
            liabilities_e8s: s.house_stats().liabilities_total,
            jackpot_icp_e8s: s.jackpot(Currency::Icp),
            jackpot_chips: s.jackpot(Currency::Chips),
            unresolved_ledger_ops: s.unresolved.len() as u64,
            ledger_ops_failed: s.counters.ledger_ops_failed,
            ledger_ops_unknown: s.counters.ledger_ops_unknown,
            last_solvency_ok: sol.map(|r| r.ok),
            last_solvency_at: sol.map(|r| r.checked_at),
            last_solvency_surplus_e8s: sol.map(|r| r.surplus.clamp(i64::MIN as i128, i64::MAX as i128) as i64),
            paused_deposits: s.pause.deposits,
            paused_withdrawals: s.pause.withdrawals,
            paused_dealing: s.pause.dealing,
            paused_joins: s.pause.joins,
            open_reports: s.open_reports.len() as u64,
            reports_filed: s.counters.reports_filed,
            bans: s.counters.bans,
            idle_stand_ups: s.counters.idle_stand_ups,
            ..Default::default()
        }
    }

    pub fn to_prometheus(&self) -> String {
        let mut out = String::new();
        let mut gauge = |name: &str, help: &str, kind: &str, v: f64| {
            out.push_str(&format!("# HELP bp_{name} {help}\n# TYPE bp_{name} {kind}\nbp_{name} {v}\n"));
        };
        let b = |x: bool| if x { 1.0 } else { 0.0 };
        gauge("users", "Registered players", "gauge", self.users as f64);
        gauge("active_players_5m", "Players who made an update call in the last 5 minutes", "gauge", self.active_players_5m as f64);
        gauge("seated_players", "Players seated at a table", "gauge", self.seated_players as f64);
        gauge("active_tables", "Tables with two or more players", "gauge", self.active_tables as f64);
        gauge("tables", "Open tables", "gauge", self.tables as f64);
        gauge("running_tournaments", "Tournaments in progress", "gauge", self.running_tournaments as f64);
        gauge("hands_dealt_total", "Hands dealt since install", "counter", self.hands_dealt as f64);
        gauge("cycles", "Canister cycle balance", "gauge", self.cycles as f64);
        gauge("heap_bytes", "Wasm heap size", "gauge", self.heap_bytes as f64);
        gauge("stable_bytes", "Stable memory size", "gauge", self.stable_bytes as f64);
        gauge("treasury_e8s", "House treasury", "gauge", self.treasury_e8s as f64);
        gauge("wallet_total_e8s", "Sum of player ICP balances", "gauge", self.wallet_total_e8s as f64);
        gauge("liabilities_e8s", "ICP the canister owes in total", "gauge", self.liabilities_e8s as f64);
        gauge("jackpot_icp_e8s", "ICP bad-beat jackpot", "gauge", self.jackpot_icp_e8s as f64);
        gauge("jackpot_chips", "Play-chip bad-beat jackpot", "gauge", self.jackpot_chips as f64);
        gauge("unresolved_ledger_ops", "Ledger operations with an unknown outcome", "gauge", self.unresolved_ledger_ops as f64);
        gauge("ledger_ops_failed_total", "Ledger operations that failed", "counter", self.ledger_ops_failed as f64);
        gauge("ledger_ops_unknown_total", "Ledger operations whose outcome became unknown", "counter", self.ledger_ops_unknown as f64);
        if let Some(ok) = self.last_solvency_ok {
            gauge("solvency_ok", "Last solvency check passed", "gauge", b(ok));
        }
        if let Some(at) = self.last_solvency_at {
            gauge("solvency_checked_seconds", "Time of the last solvency check (unix seconds)", "gauge", (at / 1_000_000_000) as f64);
        }
        if let Some(surplus) = self.last_solvency_surplus_e8s {
            gauge("solvency_surplus_e8s", "Ledger balance minus liabilities at the last check", "gauge", surplus as f64);
        }
        gauge("paused_deposits", "Deposits paused", "gauge", b(self.paused_deposits));
        gauge("paused_withdrawals", "Withdrawals paused", "gauge", b(self.paused_withdrawals));
        gauge("paused_dealing", "Dealing paused", "gauge", b(self.paused_dealing));
        gauge("paused_joins", "New seats paused", "gauge", b(self.paused_joins));
        gauge("open_reports", "Player reports awaiting a moderator", "gauge", self.open_reports as f64);
        gauge("reports_filed_total", "Player reports filed", "counter", self.reports_filed as f64);
        gauge("bans_total", "Suspensions issued", "counter", self.bans as f64);
        gauge("idle_stand_ups_total", "Players stood up for sitting out too long", "counter", self.idle_stand_ups as f64);
        gauge("rate_limited_calls_total", "Calls refused by the rate limiter since the last upgrade", "counter", self.rate_limited_calls as f64);
        out
    }
}

/// Current wasm heap size.
pub fn heap_bytes() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        core::arch::wasm32::memory_size(0) as u64 * 65_536
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

#[derive(CandidType, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: ByteBuf,
}

#[derive(CandidType, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body: ByteBuf,
}

impl HttpResponse {
    pub fn text(status_code: u16, content_type: &str, body: String) -> Self {
        HttpResponse {
            status_code,
            headers: vec![
                ("Content-Type".into(), content_type.into()),
                ("Cache-Control".into(), "no-store".into()),
            ],
            body: ByteBuf::from(body.into_bytes()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prometheus_text() {
        let s = State::new(candid::Principal::anonymous(), 10_000);
        let m = Metrics::from_state(&s, 0);
        let text = m.to_prometheus();
        assert!(text.contains("# TYPE bp_users gauge\nbp_users 0\n"));
        assert!(text.contains("bp_jackpot_chips 100000\n"));
        assert!(!text.contains("solvency_ok"), "no solvency line before the first check");
    }
}

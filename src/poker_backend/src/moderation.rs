//! Player safety: blocking, reports, chat mutes, bans, a word filter,
//! moderators and an audit log of every privileged action.

use crate::state::{Channel, ChatMessage, State, R};
use candid::{CandidType, Deserialize, Principal};

const MAX_BLOCKED: usize = 500;
const REPORTS_PER_DAY: u32 = 10;
const MAX_REPORT_DETAILS: usize = 1_000;
const EVIDENCE_MESSAGES: usize = 20;
const AUDIT_LOG_LEN: usize = 1_000;
const DAY_NS: u64 = 24 * 60 * 60 * 1_000_000_000;
const HOUR_NS: u64 = 60 * 60 * 1_000_000_000;

/// Words masked in chat and refused in usernames. Admins can extend the list.
pub fn default_banned_words() -> Vec<String> {
    ["fuck", "fucking", "fucker", "shit", "cunt", "bitch", "asshole", "bastard", "dickhead", "pussy", "whore", "slut", "motherfucker"]
        .iter()
        .map(|w| w.to_string())
        .collect()
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub struct Ban {
    /// 0 = permanent.
    pub until: u64,
    pub reason: String,
}

#[derive(CandidType, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum ReportReason {
    AbusiveChat,
    Cheating,
    Collusion,
    OffensiveName,
    Spam,
    Other,
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub enum ModAction {
    Dismiss,
    Warn,
    /// Chat mute for this many hours.
    MuteChat(u32),
    /// Suspension for this many hours (0 = permanent).
    Ban(u32),
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq)]
pub enum ReportStatus {
    Open,
    Resolved { by: Principal, action: ModAction, note: String, at: u64 },
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Report {
    pub id: u64,
    pub reporter: Principal,
    pub reporter_name: String,
    pub target: Principal,
    pub target_name: String,
    pub reason: ReportReason,
    pub details: String,
    pub channel: Option<Channel>,
    /// The target's recent messages in that channel, captured when reported.
    pub evidence: Vec<ChatMessage>,
    pub created_at: u64,
    pub status: ReportStatus,
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct AuditEntry {
    pub time: u64,
    pub actor: Principal,
    pub action: String,
}

/// Masks banned words (whole words, case-insensitive) with asterisks.
pub fn mask_words(text: &str, banned: &[String]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if banned.iter().any(|b| b.eq_ignore_ascii_case(word)) {
            out.extend(std::iter::repeat_n('*', word.chars().count()));
        } else {
            out.push_str(word);
        }
        word.clear();
    };
    for c in text.chars() {
        if c.is_alphanumeric() {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// True when a username contains a banned word anywhere (names have no spaces).
pub fn name_is_offensive(name: &str, banned: &[String]) -> bool {
    let lower = name.to_lowercase().replace('_', "");
    banned.iter().any(|b| lower.contains(b.as_str()))
}

impl State {
    pub fn audit(&mut self, actor: Principal, action: impl Into<String>, now: u64) {
        self.audit_log.push_front(AuditEntry { time: now, actor, action: action.into() });
        self.audit_log.truncate(AUDIT_LOG_LEN);
    }

    pub fn is_moderator(&self, p: &Principal) -> bool {
        self.moderators.contains(p)
    }

    /// Err if the player is currently suspended.
    pub fn check_not_banned(&self, p: &Principal, now: u64) -> R<()> {
        if let Some(ban) = self.users.get(p).and_then(|u| u.ban) {
            if ban.until == 0 || ban.until > now {
                let when = if ban.until == 0 { "permanently".to_string() } else { "for now".to_string() };
                return Err(format!("Your account is suspended {when}: {}. You can still withdraw your balance.", ban.reason));
            }
        }
        Ok(())
    }

    pub fn check_can_chat(&self, p: &Principal, now: u64) -> R<()> {
        self.check_not_banned(p, now)?;
        let muted_until = self.users.get(p).map_or(0, |u| u.chat_muted_until);
        if muted_until > now {
            let mins = (muted_until - now).div_ceil(60_000_000_000);
            return Err(format!("You are muted in chat for another {mins} minutes"));
        }
        Ok(())
    }

    /// Has `p` blocked `other`?
    pub fn has_blocked(&self, p: &Principal, other: &Principal) -> bool {
        self.users.get(p).is_some_and(|u| u.blocked.contains(other))
    }

    pub fn either_blocked(&self, a: &Principal, b: &Principal) -> bool {
        self.has_blocked(a, b) || self.has_blocked(b, a)
    }

    // ------------------------------------------------------------- blocking

    /// Blocking hides their chat from you, stops DMs, friend requests, gifts and
    /// throws between you, and ends any friendship.
    pub fn block(&mut self, caller: Principal, other: Principal, block: bool) -> R<()> {
        if caller == other {
            return Err("You can't block yourself".into());
        }
        self.user(&other).map_err(|_| "No such player".to_string())?;
        {
            let mut u = self.user_mut(&caller)?;
            if block {
                if u.blocked.len() >= MAX_BLOCKED && !u.blocked.contains(&other) {
                    return Err(format!("You can block up to {MAX_BLOCKED} players"));
                }
                u.blocked.insert(other);
                u.friends.remove(&other);
                u.friend_requests.remove(&other);
            } else {
                u.blocked.remove(&other);
            }
        }
        if block {
            if let Some(mut o) = self.users.get_mut(&other) {
                o.friends.remove(&caller);
                o.friend_requests.remove(&caller);
            }
        }
        Ok(())
    }

    // -------------------------------------------------------------- reports

    pub fn report(
        &mut self,
        caller: Principal,
        target: Principal,
        reason: ReportReason,
        details: String,
        channel: Option<Channel>,
        now: u64,
    ) -> R<u64> {
        if caller == target {
            return Err("You can't report yourself".into());
        }
        let target_name = self.user(&target).map_err(|_| "No such player".to_string())?.username;
        if details.chars().count() > MAX_REPORT_DETAILS {
            return Err(format!("Keep it under {MAX_REPORT_DETAILS} characters"));
        }
        let reporter_name = {
            let mut u = self.user_mut(&caller)?;
            let today = now / DAY_NS;
            if u.reports_day != today {
                u.reports_day = today;
                u.reports_today = 0;
            }
            if u.reports_today >= REPORTS_PER_DAY {
                return Err("You've sent a lot of reports today; our moderators will review them".into());
            }
            u.reports_today += 1;
            u.username.clone()
        };
        let evidence = channel
            .map(|ch| {
                let msgs = self.chat_since(ch, 0);
                let mut theirs: Vec<ChatMessage> = msgs.into_iter().filter(|m| m.from == Some(target)).collect();
                let excess = theirs.len().saturating_sub(EVIDENCE_MESSAGES);
                theirs.drain(..excess);
                theirs
            })
            .unwrap_or_default();
        let id = self.next_report_id;
        self.next_report_id += 1;
        self.stores.reports.insert(
            id,
            Report {
                id,
                reporter: caller,
                reporter_name,
                target,
                target_name,
                reason,
                details: details.trim().to_string(),
                channel,
                evidence,
                created_at: now,
                status: ReportStatus::Open,
            },
        );
        self.open_reports.insert(id);
        self.counters.reports_filed += 1;
        Ok(id)
    }

    pub fn open_reports(&self) -> Vec<Report> {
        self.open_reports.iter().filter_map(|id| self.stores.reports.get(id)).collect()
    }

    pub fn resolve_report(&mut self, moderator: Principal, id: u64, action: ModAction, note: String, now: u64) -> R<()> {
        let mut report = self.stores.reports.get(&id).ok_or("No such report")?;
        if report.status != ReportStatus::Open {
            return Err("Report already resolved".into());
        }
        let target = report.target;
        match &action {
            ModAction::Dismiss | ModAction::Warn => {}
            ModAction::MuteChat(hours) => self.mute(moderator, target, *hours, now)?,
            ModAction::Ban(hours) => self.ban(moderator, target, *hours, &note, now)?,
        }
        report.status = ReportStatus::Resolved { by: moderator, action: action.clone(), note: note.clone(), at: now };
        self.stores.reports.insert(id, report);
        self.open_reports.remove(&id);
        self.audit(moderator, format!("Resolved report #{id} against {}: {action:?} ({note})", target.to_text()), now);
        Ok(())
    }

    /// Mutes a player in all chat for `hours` (0 lifts the mute).
    pub fn mute(&mut self, moderator: Principal, target: Principal, hours: u32, now: u64) -> R<()> {
        self.user_mut(&target)?.chat_muted_until = if hours == 0 { 0 } else { now + hours as u64 * HOUR_NS };
        self.audit(moderator, format!("Chat mute {}h for {}", hours, target.to_text()), now);
        Ok(())
    }

    /// Suspends a player (`hours` = 0 is permanent). They are stood up from
    /// cash tables (chips back to their wallet) and sat out in tournaments.
    /// Withdrawals stay open so they can always take their money.
    pub fn ban(&mut self, moderator: Principal, target: Principal, hours: u32, reason: &str, now: u64) -> R<()> {
        let until = if hours == 0 { 0 } else { now + hours as u64 * HOUR_NS };
        self.user_mut(&target)?.ban = Some(crate::moderation::Ban {
            until,
            reason: if reason.is_empty() { "breaking the rules".into() } else { reason.to_string() },
        });
        let seated: Vec<u64> = self.tables.values().filter(|t| t.seat_of(&target).is_some()).map(|t| t.id).collect();
        for id in seated {
            if self.table_tournament.contains_key(&id) {
                let _ = self.tables.get_mut(&id).unwrap().set_sitting_out(&target, true, now);
            } else {
                let _ = self.leave_table(target, id, now);
            }
        }
        self.counters.bans += 1;
        self.audit(moderator, format!("Suspended {} for {} ({reason})", target.to_text(), if hours == 0 { "ever".into() } else { format!("{hours}h") }), now);
        Ok(())
    }

    pub fn unban(&mut self, moderator: Principal, target: Principal, now: u64) -> R<()> {
        self.user_mut(&target)?.ban = None;
        self.audit(moderator, format!("Lifted suspension for {}", target.to_text()), now);
        Ok(())
    }

    pub fn filter_text(&self, text: &str) -> String {
        mask_words(text, &self.banned_words)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masking_whole_words_only() {
        let banned = default_banned_words();
        assert_eq!(mask_words("well SHIT, nice hand", &banned), "well ****, nice hand");
        assert_eq!(mask_words("Scunthorpe classic", &banned), "Scunthorpe classic", "no substring false positives");
        assert_eq!(mask_words("gg wp", &banned), "gg wp");
        assert!(name_is_offensive("Big_Shit99", &banned));
        assert!(!name_is_offensive("shark", &banned));
    }
}

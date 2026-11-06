//! Alerts on players' devices, delivered by Internet Identity to anyone who
//! allowed them there. A send can't fail the game: anyone who hasn't allowed
//! alerts is simply answered "no channel" by Internet Identity.

use crate::metrics::Metrics;
use crate::tournament::StartAlert;
use candid::Principal;
use identity_notifications::{self as notifications, Notification};
use std::cell::RefCell;
use std::collections::BTreeMap;

/// Where an alert opens: the canister address, not www.bulletspoker.com. Internet
/// Identity opens a custom domain only if it can read the domain from
/// `.well-known/ii-alternative-origins`, and it reads that from the canister's
/// `ic0.app` address, which the gateway refuses for this canister. It then sends the
/// player to that same refused address, an error page. A link on the origin alerts
/// are sent for needs no list and opens as written, and the frontend then forwards
/// the player to www.bulletspoker.com (`alertForward` in `links.ts`).
pub const SITE: &str = "https://byggy-dyaaa-aaaad-aha7q-cai.icp0.io";

/// Each kind of alert counts separately in the library's delivery figures.
const START_FUNNEL: &str = "tournament_start";
const DAILY_FUNNEL: &str = "daily_chips";
const MESSAGE_FUNNEL: &str = "friend_message";
const REQUEST_FUNNEL: &str = "friend_request";

/// Most daily reminders to send in one minute's check; the rest wait for the next.
pub const DAILY_ALERTS_PER_TICK: usize = 500;

/// One player's alerts about another, at most this often: enough to say they're
/// trying to reach you, however many messages they send.
const FRIEND_ALERT_GAP_NS: u64 = 5 * 60 * 1_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FriendAlert {
    Message,
    Request,
}

/// When each (recipient, sender, kind) was last alerted. Kept in memory: at worst
/// an upgrade lets one extra alert through.
#[derive(Default)]
pub struct Throttle(BTreeMap<(Principal, Principal, FriendAlert), u64>);

impl Throttle {
    pub fn allow(&mut self, to: Principal, from: Principal, kind: FriendAlert, now: u64) -> bool {
        let last = self.0.entry((to, from, kind)).or_insert(0);
        if *last != 0 && now.saturating_sub(*last) < FRIEND_ALERT_GAP_NS {
            return false;
        }
        *last = now;
        true
    }

    pub fn prune(&mut self, now: u64) {
        self.0.retain(|_, last| now.saturating_sub(*last) < FRIEND_ALERT_GAP_NS);
    }
}

thread_local! {
    static THROTTLE: RefCell<Throttle> = RefCell::new(Throttle::default());
}

pub fn prune_throttle(now: u64) {
    THROTTLE.with_borrow_mut(|t| t.prune(now));
}

/// Names the sender, and leaves the words on their way to the conversation:
/// a phone's lock screen is no place for a private message.
pub fn friend_notification(kind: FriendAlert, from: Principal, name: &str) -> Notification {
    match kind {
        FriendAlert::Message => Notification {
            title: format!("{name} sent you a message"),
            body: "Tap to read it.".into(),
            url: Some(format!("{SITE}/?open=friends&chat={from}")),
            key: Some(format!("message-{from}")),
            funnel: Some(MESSAGE_FUNNEL.into()),
            ..Default::default()
        },
        FriendAlert::Request => Notification {
            title: format!("{name} wants to be friends"),
            body: "Tap to accept or decline.".into(),
            url: Some(format!("{SITE}/?open=friends")),
            key: Some(format!("request-{from}")),
            funnel: Some(REQUEST_FUNNEL.into()),
            ..Default::default()
        },
    }
}

pub fn daily_notification() -> Notification {
    Notification {
        title: "Your daily chips are ready".into(),
        body: "Tap to claim your free play chips.".into(),
        url: Some(format!("{SITE}/?open=lobby")),
        key: Some("daily-chips".into()),
        funnel: Some(DAILY_FUNNEL.into()),
        ..Default::default()
    }
}

pub fn send_friend_alert(kind: FriendAlert, to: Principal, from: Principal, name: &str, now: u64) {
    if THROTTLE.with_borrow_mut(|t| t.allow(to, from, kind, now)) {
        notifications::send(to, friend_notification(kind, from, name));
    }
}

pub fn send_daily_alerts(players: Vec<Principal>) {
    for player in players {
        notifications::send(player, daily_notification());
    }
}

/// What a player sees. It expires at the start, when it stops being true.
pub fn start_notification(alert: &StartAlert) -> Notification {
    Notification {
        title: "Your tournament is about to start".into(),
        body: format!("{} starts in a few minutes. Tap to take your seat.", alert.name),
        url: Some(format!("{SITE}/?tournament={}", alert.id)),
        key: Some(format!("tournament-{}", alert.id)),
        expires_at: Some(alert.start_at),
        funnel: Some(START_FUNNEL.into()),
        ..Default::default()
    }
}

pub fn send_start_alerts(alerts: Vec<StartAlert>) {
    for alert in alerts {
        let notification = start_notification(&alert);
        for player in alert.players {
            notifications::send(player, notification.clone());
        }
    }
}

/// Adds up the library's hourly figures for `/metrics`.
pub fn fill_metrics(m: &mut Metrics) {
    let figures = notifications::metrics();
    for hour in &figures.hours {
        m.alerts_queued += hour.counts.queued;
        m.alerts_accepted += hour.counts.accepted;
        m.alerts_received += hour.counts.received;
        m.alerts_opened += hour.counts.opened;
        m.alerts_dropped += hour.counts.dropped;
    }
    m.alerts_backlog = figures.backlog;
    m.alerts_misconfigured_since = figures.misconfigured.map_or(0, |m| m.since / 1_000_000_000);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn who(n: u8) -> Principal {
        Principal::from_slice(&[n; 10])
    }

    #[test]
    fn a_message_alert_opens_the_conversation_and_keeps_the_words_off_the_lock_screen() {
        let n = friend_notification(FriendAlert::Message, who(1), "alice");
        assert!(n.title.contains("alice"));
        assert_eq!(n.url, Some(format!("{SITE}/?open=friends&chat={}", who(1))));
        assert_eq!(n.key, Some(format!("message-{}", who(1))), "a newer one from alice replaces it");
    }

    #[test]
    fn a_request_alert_opens_the_friends_page() {
        let n = friend_notification(FriendAlert::Request, who(1), "alice");
        assert!(n.title.contains("alice"));
        assert_eq!(n.url, Some(format!("{SITE}/?open=friends")));
    }

    #[test]
    fn the_daily_alert_opens_the_lobby_where_the_button_is() {
        assert_eq!(daily_notification().url, Some(format!("{SITE}/?open=lobby")));
    }

    #[test]
    fn one_sender_can_alert_one_player_about_one_thing_only_so_often() {
        let mut t = Throttle::default();
        let start = 1_000 * FRIEND_ALERT_GAP_NS;
        assert!(t.allow(who(1), who(2), FriendAlert::Message, start));
        assert!(!t.allow(who(1), who(2), FriendAlert::Message, start + 1), "same again");
        assert!(t.allow(who(1), who(3), FriendAlert::Message, start), "another sender");
        assert!(t.allow(who(4), who(2), FriendAlert::Message, start), "another recipient");
        assert!(t.allow(who(1), who(2), FriendAlert::Request, start), "another kind");
        assert!(t.allow(who(1), who(2), FriendAlert::Message, start + FRIEND_ALERT_GAP_NS), "after the gap");
    }

    #[test]
    fn the_throttle_forgets_what_can_no_longer_stop_an_alert() {
        let mut t = Throttle::default();
        let start = 1_000 * FRIEND_ALERT_GAP_NS;
        t.allow(who(1), who(2), FriendAlert::Message, start);
        t.prune(start + FRIEND_ALERT_GAP_NS - 1);
        assert_eq!(t.0.len(), 1);
        t.prune(start + FRIEND_ALERT_GAP_NS);
        assert!(t.0.is_empty());
    }

    fn alert() -> StartAlert {
        StartAlert { id: 7, name: "Hourly Freeroll".into(), start_at: 1_000, players: vec![Principal::anonymous()] }
    }

    #[test]
    fn a_start_alert_names_the_event_and_is_gone_once_it_starts() {
        let n = start_notification(&alert());
        assert!(n.body.contains("Hourly Freeroll"));
        assert_eq!(n.url.as_deref(), Some("https://byggy-dyaaa-aaaad-aha7q-cai.icp0.io/?tournament=7"));
        assert_eq!(n.key.as_deref(), Some("tournament-7"));
        assert_eq!(n.expires_at, Some(1_000));
    }

    /// Only the origin alerts are sent for always passes Internet Identity's check
    /// (see `SITE`), so an alert has to open that same address.
    #[test]
    fn alerts_open_the_origin_they_are_sent_for() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../icp.yaml");
        let config = std::fs::read_to_string(path).unwrap();
        assert!(config.contains(&format!("notification_origin: \"{SITE}\"")), "{config}");
    }
}

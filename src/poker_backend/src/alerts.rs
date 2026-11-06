//! Alerts on players' devices, delivered by Internet Identity to anyone who
//! allowed them there. A send can't fail the game: anyone who hasn't allowed
//! alerts is simply answered "no channel" by Internet Identity.

use crate::metrics::Metrics;
use crate::tournament::StartAlert;
use identity_notifications::{self as notifications, Notification};

/// Where an alert opens: the canister address, not www.bulletspoker.com. Internet
/// Identity opens a custom domain only if it can read the domain from
/// `.well-known/ii-alternative-origins`, and it reads that from the canister's
/// `ic0.app` address, which the gateway refuses for this canister. It then sends the
/// player to that same refused address, an error page. A link on the origin alerts
/// are sent for needs no list and opens as written, and the frontend then forwards
/// the player to www.bulletspoker.com (`alertForward` in `links.ts`).
pub const SITE: &str = "https://byggy-dyaaa-aaaad-aha7q-cai.icp0.io";

/// Counts this kind of alert separately in the library's delivery figures.
const START_FUNNEL: &str = "tournament_start";

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
    use candid::Principal;

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

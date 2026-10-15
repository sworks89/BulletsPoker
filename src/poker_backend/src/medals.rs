//! Medals (achievements): definitions and the checks that award them.

use crate::cards::{best_hand, rank};
use crate::state::{Channel, MessageKind, State};
use crate::table::{Currency, HandResult};
use candid::{CandidType, Deserialize, Principal};

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct MedalDef {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: String,
}

const MEDAL_XP: u64 = 25;

/// (id, name, icon, description)
const MEDALS: &[(&str, &str, &str, &str)] = &[
    ("first_hand", "Shuffle Up", "🃏", "Play your first hand"),
    ("first_pot", "First Blood", "🩸", "Win your first pot"),
    ("hands_100", "Regular", "🪑", "Play 100 hands"),
    ("hands_1000", "Grinder", "⚙️", "Play 1,000 hands"),
    ("full_house", "Full House", "🏠", "Win a pot with a full house"),
    ("quads", "Quads!", "🍀", "Win a pot with four of a kind"),
    ("straight_flush", "Straight Flush", "🌈", "Win a pot with a straight flush"),
    ("royal_flush", "Royal Flush", "👑", "Win a pot with a royal flush"),
    ("bad_beat", "Bad Beat", "💔", "Lose a showdown holding a full house or better"),
    ("jackpot", "Jackpot!", "💥", "Be the loser or winner of a bad-beat jackpot hand"),
    ("chip_leader", "Chip Leader", "🎲", "Win a play-chip pot of 25,000 chips or more"),
    ("whale", "Whale", "🐋", "Win an ICP pot of 1 ICP or more"),
    ("generous", "Generous", "🎁", "Send 10 gifts"),
    ("food_fight", "Food Fight", "🍅", "Throw 25 things at other players"),
    ("popular", "Popular", "🤝", "Have 10 friends"),
    ("dedicated", "Dedicated", "📅", "Claim the daily bonus 7 days in a row"),
    ("host", "Host", "🏡", "Create your own table"),
    ("in_the_money", "In the Money", "💰", "Finish a tournament in the prizes"),
    ("sng_champ", "Sit & Go Champ", "🥇", "Win a sit-and-go"),
    ("champion", "Tournament Champion", "🏆", "Win a scheduled tournament"),
];

pub fn medal_defs() -> Vec<MedalDef> {
    MEDALS
        .iter()
        .map(|(id, name, icon, description)| MedalDef {
            id: id.to_string(),
            name: name.to_string(),
            icon: icon.to_string(),
            description: description.to_string(),
        })
        .collect()
}

fn medal(id: &str) -> Option<&'static (&'static str, &'static str, &'static str, &'static str)> {
    MEDALS.iter().find(|m| m.0 == id)
}

impl State {
    /// Awards a medal once. Announces it in `channel` when given.
    pub fn award(&mut self, p: &Principal, id: &str, channel: Option<Channel>, now: u64) {
        let Some((_, name, icon, _)) = medal(id) else { return };
        let Some(u) = self.users.get_mut(p) else { return };
        if u.medals.contains_key(id) {
            return;
        }
        u.medals.insert(id.to_string(), now);
        u.xp += MEDAL_XP;
        let username = u.username.clone();
        if let Some(ch) = channel {
            self.post(ch, None, String::new(), format!("🏅 {username} earned the {icon} {name} medal!"), MessageKind::System, now);
        }
    }

    /// Medals that depend on the outcome of a hand.
    pub fn award_hand_medals(&mut self, table_id: u64, currency: Option<Currency>, result: &HandResult, now: u64) {
        let ch = Some(Channel::Table(table_id));
        for pr in &result.players {
            let played = self.users.get(&pr.player).map_or(0, |u| u.stats.hands_played);
            self.award(&pr.player, "first_hand", ch, now);
            if played >= 100 {
                self.award(&pr.player, "hands_100", ch, now);
            }
            if played >= 1000 {
                self.award(&pr.player, "hands_1000", ch, now);
            }
            if pr.won > 0 {
                self.award(&pr.player, "first_pot", ch, now);
                match currency {
                    Some(Currency::Chips) if pr.won >= 25_000 => self.award(&pr.player, "chip_leader", ch, now),
                    Some(Currency::Icp) if pr.won >= crate::state::E8S => self.award(&pr.player, "whale", ch, now),
                    _ => {}
                }
            }
        }
        if result.board.len() < 5 {
            return;
        }
        // Showdown hands (before auto-muck hides them from other players).
        for sh in &result.shown {
            let mut cards = sh.cards.clone();
            cards.extend_from_slice(&result.board);
            let (score, best) = best_hand(&cards);
            let category = score >> 20;
            let won = result.winners.iter().any(|w| w.player == sh.player);
            if won {
                if category >= 6 {
                    self.award(&sh.player, "full_house", ch, now);
                }
                if category >= 7 {
                    self.award(&sh.player, "quads", ch, now);
                }
                if category == 8 {
                    self.award(&sh.player, "straight_flush", ch, now);
                    if best.iter().any(|&c| rank(c) == 12) && best.iter().any(|&c| rank(c) == 8) {
                        self.award(&sh.player, "royal_flush", ch, now);
                    }
                }
            } else if category >= 6 {
                self.award(&sh.player, "bad_beat", ch, now);
            }
        }
    }
}

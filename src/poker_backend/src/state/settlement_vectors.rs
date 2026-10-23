//! Settlement vectors for the browser verifier (`src/poker_frontend/src/fair/settle.ts`).
//!
//! Every case is a real hand played through `State` (deal, actions, settlement,
//! `record_hand`). The file holds the stored hand record (all hole cards and the
//! seed) and what the engine paid: per-seat winnings, the rake and the jackpot
//! drop. "Rigged" cases overwrite the hole cards and the deck after the deal to
//! force a split, side pots or an odd chip; their cards do not follow from the
//! seed, so only the settlement is checked against them. Unrigged cases are
//! whole hands the verifier must report as VERIFIED.
//!
//! Regenerate with
//! `UPDATE_SETTLEMENT_VECTORS=1 cargo test -p poker_backend settlement_vectors_file_matches`.

use super::tests::{p, setup};
use super::*;
use crate::table::{test_canister, Action};

const TABLE: u64 = 7; // Low Rollers: 0.05 / 0.1 ICP, 9 seats
const BB: u64 = E8S / 10;

struct Fees {
    rake_bps: u16,
    rake_cap: u64,
    jackpot_bps: u16,
    jackpot_cap: u64,
}

fn card(s: &str) -> u8 {
    let b = s.as_bytes();
    let r = b"23456789TJQKA".iter().position(|&x| x == b[0]).unwrap() as u8;
    let su = b"cdhs".iter().position(|&x| x == b[1]).unwrap() as u8;
    r * 4 + su
}

fn seed(tag: u8) -> [u8; 32] {
    std::array::from_fn(|i| tag ^ (i as u8).wrapping_mul(29).wrapping_add(3))
}

/// p1, p2, p3 at seats 0, 2, 4 with these buy-ins; the first hand's button is seat 0.
fn table_with(fees: &Fees, buys: [u64; 3]) -> State {
    let mut s = setup();
    {
        let c = &mut s.tables.get_mut(&TABLE).unwrap().config;
        c.rake_bps = fees.rake_bps;
        c.rake_cap = fees.rake_cap;
        c.jackpot_bps = fees.jackpot_bps;
        c.jackpot_cap = fees.jackpot_cap;
    }
    for (k, (seat, buy)) in [0u8, 2, 4].into_iter().zip(buys).enumerate() {
        s.join_table(p(k as u8 + 1), TABLE, seat, buy, None, 0).unwrap();
    }
    assert_eq!(s.tick(1), vec![TABLE]);
    s
}

/// Hole cards for seats 0, 2, 4 and a five-card board, dealt by the engine's
/// own `deal_board` (one burn before the flop, the turn and the river).
fn rig(s: &mut State, holes: [&str; 3], board: &str) {
    let t = s.tables.get_mut(&TABLE).unwrap();
    for (seat, h) in [0usize, 2, 4].into_iter().zip(holes) {
        t.seats[seat].as_mut().unwrap().hole = h.split_whitespace().map(card).collect();
    }
    let b: Vec<u8> = board.split_whitespace().map(card).collect();
    let used: Vec<u8> = holes.iter().flat_map(|h| h.split_whitespace().map(card)).chain(b.iter().copied()).collect();
    let mut burns = (0..52u8).filter(|c| !used.contains(c));
    let (b1, b2, b3) = (burns.next().unwrap(), burns.next().unwrap(), burns.next().unwrap());
    // Popped from the end: burn, flop x3, burn, turn, burn, river.
    t.deck = vec![b[4], b3, b[3], b2, b[2], b[1], b[0], b1];
}

/// Applies `script` to whoever is to act, then checks (or calls) the hand down.
fn play(s: &mut State, script: &[Action]) {
    let mut now = 2;
    let mut script = script.iter();
    while s.tables[&TABLE].phase.is_betting() {
        let t = &s.tables[&TABLE];
        let actor = t.seats[t.to_act.unwrap()].as_ref().unwrap().player;
        match script.next() {
            Some(a) => s.act(actor, TABLE, a.clone(), now).unwrap(),
            None => {
                if s.act(actor, TABLE, Action::Check, now).is_err() {
                    s.act(actor, TABLE, Action::Call, now).unwrap();
                }
            }
        }
        now += 1;
    }
}

struct Case {
    name: &'static str,
    rigged: bool,
    fees: Fees,
    s: State,
}

fn cases() -> Vec<Case> {
    use Action::*;
    let mut out = vec![];
    let std_buy = [5 * E8S, 5 * E8S, 5 * E8S];
    // Button seat 0 (p1), small blind seat 2 (p2), big blind seat 4 (p3).
    // Pre-flop order: p1, p2, p3. After the flop: p2, p3, p1.

    // Split pot: p1 and p3 hold the same straight, p2 aces.
    let fees = Fees { rake_bps: 500, rake_cap: 3 * BB, jackpot_bps: 100, jackpot_cap: BB };
    let mut s = table_with(&fees, std_buy);
    s.deal_with_seed(TABLE, 0, seed(1), test_canister(), 1).unwrap();
    rig(&mut s, ["Tc Jd", "As Ad", "Th Js"], "2c 7d 8h 9s Ks");
    play(&mut s, &[Call, Call, Check]);
    out.push(Case { name: "split_pot", rigged: true, fees, s });

    // Three-way all-in: p1 short (wins the main pot), p2 middle (wins the
    // side pot), p3 covers both. Rake and drop rates that do not divide evenly.
    let fees = Fees { rake_bps: 333, rake_cap: 10 * E8S, jackpot_bps: 77, jackpot_cap: 10 * E8S };
    let mut s = table_with(&fees, [230_000_000, 5 * E8S, 10 * E8S]);
    s.deal_with_seed(TABLE, 0, seed(2), test_canister(), 1).unwrap();
    rig(&mut s, ["As Ah", "Ks Kh", "Qs Qh"], "2c 7d 9c 4d Jc");
    play(&mut s, &[AllIn, AllIn, Call]);
    out.push(Case { name: "three_way_all_in_side_pots", rigged: true, fees, s });

    // Everyone folds to the big blind before the flop: no rake, no drop.
    let fees = Fees { rake_bps: 500, rake_cap: 3 * BB, jackpot_bps: 100, jackpot_cap: BB };
    let mut s = table_with(&fees, std_buy);
    s.deal_with_seed(TABLE, 0, seed(3), test_canister(), 1).unwrap();
    play(&mut s, &[Fold, Fold]);
    out.push(Case { name: "everyone_folds_preflop", rigged: false, fees, s });

    // A big pot: the rake and the drop both hit their caps.
    let fees = Fees { rake_bps: 1000, rake_cap: 2_000_000, jackpot_bps: 300, jackpot_cap: 500_000 };
    let mut s = table_with(&fees, std_buy);
    s.deal_with_seed(TABLE, 0, seed(4), test_canister(), 1).unwrap();
    play(&mut s, &[Raise(5 * BB), Call, Call]);
    out.push(Case { name: "rake_cap", rigged: false, fees, s });

    // Odd chip: p1 (the button, seat 0) and p3 (seat 4) split an odd pot.
    // The odd chip goes to the first winner left of the button: seat 4.
    let fees = Fees { rake_bps: 0, rake_cap: 0, jackpot_bps: 0, jackpot_cap: 0 };
    let mut s = table_with(&fees, std_buy);
    s.deal_with_seed(TABLE, 0, seed(5), test_canister(), 1).unwrap();
    rig(&mut s, ["Tc Jd", "As Ad", "Th Js"], "2c 7d 8h 9s Ks");
    play(&mut s, &[Raise(2 * BB + 1), Call, Call]);
    out.push(Case { name: "odd_chip_split", rigged: true, fees, s });

    // A bet on the flop that nobody calls: raked, uncontested, bet returned.
    let fees = Fees { rake_bps: 500, rake_cap: 3 * BB, jackpot_bps: 100, jackpot_cap: BB };
    let mut s = table_with(&fees, std_buy);
    s.deal_with_seed(TABLE, 0, seed(6), test_canister(), 1).unwrap();
    play(&mut s, &[Call, Call, Check, Raise(BB), Fold, Fold]);
    out.push(Case { name: "uncontested_after_flop", rigged: false, fees, s });

    // A three-way showdown at the table's own (default) rates.
    let mut s = setup();
    for (k, seat) in [(1u8, 0u8), (2, 2), (3, 4)] {
        s.join_table(p(k), TABLE, seat, 5 * E8S, None, 0).unwrap();
    }
    assert_eq!(s.tick(1), vec![TABLE]);
    let c = s.tables[&TABLE].config.clone();
    let fees = Fees { rake_bps: c.rake_bps, rake_cap: c.rake_cap, jackpot_bps: c.jackpot_bps, jackpot_cap: c.jackpot_cap };
    s.deal_with_seed(TABLE, 0, seed(7), test_canister(), 1).unwrap();
    play(&mut s, &[]);
    out.push(Case { name: "showdown_default_rates", rigged: false, fees, s });

    out
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn js(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn nums(v: &[u8]) -> String {
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
}

/// The stored record as JSON. u64 values are decimal strings.
fn record_json(r: &HandRecord, ind: &str) -> String {
    let mut o = String::new();
    o.push_str("{\n");
    let f = |k: &str, v: String| format!("{ind}  \"{k}\": {v},\n");
    o.push_str(&f("id", js(&r.id.to_string())));
    o.push_str(&f("table_id", js(&r.table_id.to_string())));
    o.push_str(&f("hand_no", js(&r.hand_no.to_string())));
    o.push_str(&f("small_blind", js(&r.small_blind.to_string())));
    o.push_str(&f("big_blind", js(&r.big_blind.to_string())));
    o.push_str(&f("button", r.button.to_string()));
    o.push_str(&f("board", format!("[{}]", nums(&r.board))));
    o.push_str(&f("pot", js(&r.pot.to_string())));
    o.push_str(&f("rake", js(&r.rake.to_string())));
    o.push_str(&format!("{ind}  \"players\": [\n"));
    for (k, pl) in r.players.iter().enumerate() {
        o.push_str(&format!(
            "{ind}    {{ \"seat\": {}, \"player\": {}, \"username\": {}, \"cards\": [{}], \"shown\": {}, \"committed\": {}, \"won\": {} }}{}\n",
            pl.seat,
            js(&pl.player.to_text()),
            js(&pl.username),
            nums(&pl.cards),
            pl.shown,
            js(&pl.committed.to_string()),
            js(&pl.won.to_string()),
            if k + 1 == r.players.len() { "" } else { "," }
        ));
    }
    o.push_str(&format!("{ind}  ],\n"));
    o.push_str(&format!("{ind}  \"log\": [\n"));
    for (k, e) in r.log.iter().enumerate() {
        let kind = match &e.kind {
            None => "null".to_string(),
            Some(LogKind::ShuffleCommitted { commitment }) => {
                format!("{{ \"ShuffleCommitted\": {{ \"commitment\": {} }} }}", js(&hex(commitment)))
            }
            Some(LogKind::ShuffleRevealed) => js("ShuffleRevealed"),
            Some(LogKind::HandVerificationAvailable) => js("HandVerificationAvailable"),
        };
        o.push_str(&format!(
            "{ind}    {{ \"phase\": {}, \"seat\": {}, \"text\": {}, \"amount\": {}, \"kind\": {} }}{}\n",
            js(&format!("{:?}", e.phase)),
            e.seat.map_or("null".to_string(), |x| x.to_string()),
            js(&e.text),
            js(&e.amount.to_string()),
            kind,
            if k + 1 == r.log.len() { "" } else { "," }
        ));
    }
    o.push_str(&format!("{ind}  ],\n"));
    match &r.fair {
        None => o.push_str(&format!("{ind}  \"fair\": null\n")),
        Some(fe) => {
            o.push_str(&format!("{ind}  \"fair\": {{\n"));
            let g = |k: &str, v: String| format!("{ind}    \"{k}\": {v},\n");
            o.push_str(&g("version", fe.version.to_string()));
            o.push_str(&g("scheme", js(&fe.scheme)));
            o.push_str(&g("canister", js(&fe.canister.to_text())));
            o.push_str(&g("table_id", js(&fe.table_id.to_string())));
            o.push_str(&g("hand_no", js(&fe.hand_no.to_string())));
            o.push_str(&g("commitment", js(&hex(&fe.commitment))));
            o.push_str(&g("revealed_seed", fe.revealed_seed.as_ref().map_or("null".to_string(), |x| js(&hex(x)))));
            o.push_str(&g("params", js(&hex(&fe.params))));
            o.push_str(&g("outcome", js(&fe.outcome)));
            o.push_str(&g("dealt_order", format!("[{}]", nums(&fe.dealt_order))));
            o.push_str(&g("burns", fe.burns.to_string()));
            o.push_str(&g("settled_at", js(&fe.settled_at.to_string())));
            o.push_str(&format!("{ind}    \"reveal_at\": {}\n", js(&fe.reveal_at.to_string())));
            o.push_str(&format!("{ind}  }}\n"));
        }
    }
    o.push_str(&format!("{ind}}}"));
    o
}

fn vectors_json() -> String {
    let mut o = String::new();
    o.push_str("{\n  \"protocol\": \"BP-FAIR-v1\",\n");
    o.push_str(&format!("  \"canister\": {},\n", js(&test_canister().to_text())));
    o.push_str("  \"cases\": [\n");
    let all = cases();
    for (n, c) in all.iter().enumerate() {
        let t = &c.s.tables[&TABLE];
        let r = t.last_result.as_ref().expect("the hand settled");
        let id = c.s.hand_history(p(1), 1)[0].id;
        let rec = c.s.stores.hands.get(&id).expect("recorded");
        assert_eq!(rec.hand_no, r.hand_no);
        // Engine-side sanity: the settlement conserves chips.
        let won: u64 = r.players.iter().map(|x| x.won).sum();
        assert_eq!(won + r.rake + r.jackpot_drop, r.pot, "{}", c.name);
        o.push_str("    {\n");
        o.push_str(&format!("      \"name\": {},\n", js(c.name)));
        o.push_str(&format!("      \"rigged\": {},\n", c.rigged));
        o.push_str(&format!(
            "      \"fees\": {{ \"rake_bps\": {}, \"rake_cap\": {}, \"jackpot_bps\": {}, \"jackpot_cap\": {} }},\n",
            c.fees.rake_bps,
            js(&c.fees.rake_cap.to_string()),
            c.fees.jackpot_bps,
            js(&c.fees.jackpot_cap.to_string())
        ));
        let won_by_seat: Vec<String> = rec
            .players
            .iter()
            .map(|pl| {
                let w = r.players.iter().find(|x| x.player == pl.player).map_or(0, |x| x.won);
                format!("{{ \"seat\": {}, \"won\": {} }}", pl.seat, js(&w.to_string()))
            })
            .collect();
        o.push_str(&format!(
            "      \"expected\": {{ \"pot\": {}, \"rake\": {}, \"jackpot_drop\": {}, \"won\": [{}] }},\n",
            js(&r.pot.to_string()),
            js(&r.rake.to_string()),
            js(&r.jackpot_drop.to_string()),
            won_by_seat.join(", ")
        ));
        o.push_str(&format!("      \"record\": {}\n", record_json(&rec, "      ")));
        o.push_str(if n + 1 == all.len() { "    }\n" } else { "    },\n" });
    }
    o.push_str("  ]\n}\n");
    o
}

#[test]
fn cases_cover_what_they_claim() {
    let all = cases();
    let get = |name: &str| {
        let c = all.iter().find(|c| c.name == name).unwrap();
        c.s.tables[&TABLE].last_result.clone().unwrap()
    };
    let r = get("split_pot");
    assert_eq!(r.winners.len(), 2);
    let r = get("three_way_all_in_side_pots");
    assert_eq!(r.winners.len(), 2, "main and side pot go to different players");
    assert!(r.rake > 0 && r.jackpot_drop > 0);
    let r = get("everyone_folds_preflop");
    assert!(r.board.is_empty());
    assert_eq!((r.rake, r.jackpot_drop), (0, 0));
    let r = get("rake_cap");
    assert_eq!((r.rake, r.jackpot_drop), (2_000_000, 500_000));
    let r = get("odd_chip_split");
    assert_eq!(r.pot % 2, 1);
    let w4 = r.winners.iter().find(|w| w.seat == 4).unwrap().amount;
    let w0 = r.winners.iter().find(|w| w.seat == 0).unwrap().amount;
    assert_eq!(w4, w0 + 1, "the odd chip goes left of the button, not to the lowest seat");
    let r = get("uncontested_after_flop");
    assert_eq!(r.board.len(), 3);
    assert!(r.rake > 0);
    let r = get("showdown_default_rates");
    assert_eq!(r.board.len(), 5);
}

#[test]
fn settlement_vectors_file_matches() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../poker_frontend/src/fair/settlement-vectors.json");
    let want = vectors_json();
    if std::env::var("UPDATE_SETTLEMENT_VECTORS").as_deref() == Ok("1") {
        std::fs::write(&path, &want).unwrap();
    }
    let got = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!("{} is missing; run `UPDATE_SETTLEMENT_VECTORS=1 cargo test settlement_vectors_file_matches`", path.display())
    });
    assert!(
        got.replace("\r\n", "\n") == want,
        "settlement-vectors.json is stale; run `UPDATE_SETTLEMENT_VECTORS=1 cargo test settlement_vectors_file_matches`"
    );
}

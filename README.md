# ♠ Bullets Poker

A social Texas Hold'em poker room that runs entirely on the [Internet Computer](https://internetcomputer.org), in the spirit of TripleJack, with **ICP as the currency**. Players use ICP to buy in at tables and to buy avatars, card backs and table gifts.

## Features

**Poker**
- No-limit Texas Hold'em cash tables (heads-up up to 10-max) with blinds, a moving button, min-raise rules, all-ins, side pots, split pots and odd-chip handling
- Action clock with a time bank: when your clock runs out, up to 30 seconds of extra time kicks in automatically (you're only charged what you use, and it refills 2 seconds every hand). A player who runs out of both is checked or folded and sat out ("I'm back" to return)
- A "reconnecting" banner when your connection drops, and an instant catch-up when it returns
- Hole cards stay private: the backend only returns your own cards, plus hands shown at showdown
- A fresh deck every hand, shuffled with on-chain randomness (`raw_rand`)
- Configurable rake (default 2.5%, capped at 3 big blinds, and none if there's no flop)

**Play chips (free-to-play)**
- A second currency for free tables: 20,000 chips on signup, plus a daily bonus that grows with a 7-day streak
- Separate rake-free chip tables; chips are never backed by or exchangeable for ICP
- Cosmetics, gifts and throwables can be bought with chips as well as ICP (premium items stay ICP only)

**Multi-tabling**
- Play up to 4 tables at once: a strip of open tables follows you on every page, flashing with a countdown when it's your turn
- Tabs mode (one table at a time, optional auto-switch to the table that needs you) or tile mode (tables side by side with chat for the focused one)
- All open tables are refreshed with a single `get_tables` query per second; open tables and layout are remembered per browser

**Tournaments**
- Sit-and-go's that start as soon as they fill (chips and ICP, heads-up to 9-max), re-opening automatically
- Scheduled multi-table tournaments (an hourly play-chip freeroll with a guaranteed prize pool, plus a nightly ICP event) with rising blinds, table balancing and merging down to a final table, and payouts that scale with the field
- Late registration for the first levels of scheduled events (late players are seated at the emptiest table, or a new one opens)
- Rebuy events: during the rebuy period, busted players get 30 seconds to rebuy before they're knocked out, and anyone at or below the starting stack can rebuy between hands. Rebuys cost the buy-in only (the house fee is charged once, on entry)
- Add-ons: one extra stack per player, sold during the last rebuy level and the level after it, with the whole price going into the prize pool
- Pools and payouts grow with every entry, and the pool always pays out exactly
- Absent players keep getting dealt in and blind off, so every event finishes; tournament chips can't be cashed out, and prizes go straight to your wallet

**Host your own table**
- Players from level 2 can open a table (chips or ICP) with their choice of stakes, seats and clock, optionally password protected
- Hosts of ICP tables earn 20% of the rake their table generates; empty tables close after 30 minutes

**Bad-beat jackpot** (cash tables only)
- Every cash pot that sees a flop drops 1% (after rake, capped at one big blind) into a jackpot: one in ICP, one in play chips (seeded with 100,000 chips). Tournament tables don't feed it and can't win it
- Hits when four of a kind or better loses at showdown, with both of the loser's hole cards playing and at least 4 players dealt in
- Pays 90% of the jackpot: half to the player with the losing hand, a quarter to the winner, and the rest split by everyone else dealt in; 10% seeds the next one
- Lobby banner with rules and recent hits, jackpot amount at every cash table, and a celebration overlay for everyone at the table; the ICP jackpot is part of the canister's accounted liabilities, and admins can top it up from the treasury

**Player notes**
- Private notes on any player: free text plus a color label (maniac, loose, fish, nit…). Seats show the color as a ring around the avatar so you can read a table at a glance
- Edit from the player card at the table; search and manage all your notes on the Friends & Notes page

**Player safety**
- Block anyone from their player card: their chat disappears for you, and they can't message, befriend, gift or throw things at you. Manage blocks on the Friends & Notes page
- Report players (abusive chat, cheating, collusion, offensive names, spam) with their recent chat attached as evidence; up to 10 reports a day
- Moderators work a report queue and can warn, mute in chat, or suspend (timed or permanent). Suspended players are stood up from cash tables with their chips returned and can always withdraw
- Usernames with offensive words are refused, and chat masks them

**Fair seats**
- Players sitting out at a cash table for 10 minutes are stood up automatically, chips back to their wallet
- At most 4 cash seats at once per player (tournament tables don't count)

**Medals**
- 19 medals for milestones (first pot, 1,000 hands), big hands (quads, royal flush, bad beats), social play (gifts, throwables, friends) and tournament results, shown on profiles and in player cards

**Hand history & settings**
- Your last 50 hands with a step-by-step replay (board, pot and action log). Only your own cards and hands shown at showdown are revealed
- Settings saved to your profile: four-color deck, auto-muck losing hands, auto top-up, sounds, show/hide throwables

**Social**
- Lobby chat and per-table chat with quick emotes, plus rate limiting and a word filter
- Friends: search by username, friend requests, online status, "join their table", direct messages
- Table gifts (beer, roses, champagne, trophies…) shown on the recipient's seat
- Throwables (tomatoes, pies, fish, bombs…) animated flying from your seat to another player's
- Profiles with levels/XP, stats (hands, win rate, biggest pot, net winnings), status message
- Leaderboard

**ICP economy**
- Sign in with Internet Identity
- Deposit either with **ICRC-2** (approve and pull from your II wallet in one click) or by **sending ICP from any wallet or exchange** to your personal deposit address, then claiming it
- Withdraw to any principal
- Store: premium avatars, card backs and gifts priced in ICP
- House treasury collects rake and store sales; controllers can withdraw it

## Architecture

```
src/
  poker_backend/            Rust canister (ic-cdk 0.20)
    src/cards.rs            deck, SHA-256 seeded Fisher–Yates shuffle, 7-card evaluator
    src/table.rs            Hold'em state machine (pure logic, heavily tested)
    src/state.rs            users, wallet balances, store, gifts, chat, friends, tables, player rooms
    src/tournament.rs       sit-and-go's and scheduled tournaments: blinds, payouts, balancing
    src/medals.rs           medal definitions and awarding
    src/jackpot.rs          bad-beat jackpot: qualifying hands and payouts
    src/ledger.rs           ICP ledger calls (ICRC-1 transfer, ICRC-2 transfer_from), deposit accounts
    src/wallet.rs           recorded ledger operations, pause switches, solvency checks
    src/moderation.rs       blocking, reports, mutes, bans, word filter, audit log
    src/ratelimit.rs        per-caller request budgets
    src/metrics.rs          /metrics (Prometheus) and /health
    src/lib.rs              canister endpoints, timers, upgrade persistence
    poker_backend.did       Candid interface (generated from the Rust code)
  poker_frontend/           React + Vite + @icp-sdk/core / @icp-sdk/auth
    src/components/         Lobby, Table, Wallet, Store, Friends, Profile/Leaderboard, Chat
    src/declarations/       generated Candid bindings
scripts/                    local deploy and test-ICP funding helpers
```

**How money moves:** 1 chip = 1 e8s of ICP. Your *poker balance* lives in the backend canister. Sitting down moves your buy-in from your balance onto the table, and standing up moves your stack back. The canister's main ICP ledger account always holds at least the sum of all balances, all chips on tables and the treasury. `admin_house_stats` reports that total so you can reconcile it against the ledger.

**Game loop:** a 1-second canister timer enforces action clocks and deals the next hand about 5 seconds after the previous one ends. Players' clients poll `get_table` (a fast query) every second.

## Development

Requirements: Rust (with `wasm32-unknown-unknown`), Node 22, and [dfx](https://internetcomputer.org/docs/building-apps/getting-started/install).

```bash
# Backend tests: hand evaluator, betting/side pots, 500 randomized hands, accounting
cargo test

# After changing the backend API, regenerate the .did and the frontend bindings
UPDATE_CANDID=1 cargo test

# Run everything locally (local ICP ledger with ICRC-2, Internet Identity, backend, frontend)
dfx start --background --clean
./scripts/deploy-local.sh

# Give your Internet Identity principal (shown on the Wallet page) some local ICP
./scripts/fund.sh <your-principal> 50

# Hot-reloading frontend against the local replica
cd src/poker_frontend && npm run dev
```

## Deploying to mainnet

```bash
dfx deploy --network ic poker_backend   # defaults to the real ICP ledger
dfx deploy --network ic poker_frontend
```

Only controllers can call the `admin_*` methods (create tables, add recurring tournaments, add store items, view house stats, withdraw the treasury). Six tables, from micro to high stakes, and the default store catalogue are created at install time.

## Operations

**Storage and upgrades.** Users, usernames, notes, direct messages, hand histories and the ledger-operation log live directly in stable memory (`src/store.rs`), so upgrades never serialize them. Live game state (tables, tournaments, chats) is bounded and saved as a versioned snapshot in `pre_upgrade`. Rule for future changes: new fields on stored records must be `Option<T>`, or bump `SNAPSHOT_VERSION` and migrate in `State::restore`.

**Money movements.** Every deposit, sweep and withdrawal is recorded before it is sent, and sent with `created_at_time` plus a memo carrying the operation id, so the ledger deduplicates retries. Outcomes are *completed*, *failed* (any debit is refunded) or *unknown* (money stays reserved). Unknown operations are re-sent every minute; the ledger either executes them or reports the original as a duplicate, so nothing is paid twice. A retry that fails never refunds an unknown operation, since the ledger can reject a retry (busy, upgrading) while the first attempt went through. Once the ledger's 24-hour window has closed (plus a 10-minute margin), an admin checks the ledger and settles it with `admin_resolve_op`; earlier attempts are refused. Release builds have overflow checks on, so any arithmetic bug traps and rolls back instead of wrapping. Tournament entry fees stay with the event (and stay refundable) until it starts. Players see all of this in the wallet's transaction list.

**Emergency stop.** `admin_set_pause` independently pauses deposits, withdrawals, dealing new hands, and new seats/tables, with a reason shown to players in a banner. Hands in progress always finish and players can always stand up.

**Solvency.** Every 10 minutes (and on demand via `admin_check_solvency`) the canister compares its ICP ledger balance with everything it owes: wallet balances, chips on ICP tables, tournament pools, the jackpot and the treasury. A shortfall pauses withdrawals automatically, but only if a second check confirms it, so transfers settling mid-check can't trigger a false alarm. `get_status` exposes the latest report publicly. `admin_audit_wallet_total` re-adds every balance to verify the running total.

**Upgrade runbook.** 1) `admin_set_pause` withdrawals and deposits. 2) Check `admin_unresolved_ops` is empty (or wait for the retry loop). 3) `dfx canister stop`, upgrade, `dfx canister start`. Ledger calls still in flight at upgrade time are marked unknown and retried automatically. 4) Unpause.

**Abuse protection.** `inspect_message` drops ingress calls before they cost cycles: anonymous or unregistered callers (except `register`), arguments over 16 KB, `admin_*` from non-controllers, `mod_*` from non-moderators, and callers over budget. Each player gets a burst of 20 update calls, refilling at 5 per second; every endpoint checks again, since `inspect_message` isn't guaranteed to run. Sign-ups are capped per hour (`admin_set_signup_limit`, default 1,000), hosted tables are capped at 200 overall, and friend lists, friend requests and block lists are bounded.

**Admin page.** Controllers and moderators get a 🛡️ button in the top bar (the backend decides via `get_my_roles`; every action is re-checked server-side). Admins see the live overview (players, tables, money, solvency, cycles and memory), the emergency switches, the report queue, player lookup with mute/suspend/moderator controls, treasury withdrawals, jackpot top-ups, unconfirmed ledger operations (retry or settle), settings (moderators, sign-up limit, word filter) and the audit log. Moderators see only the report queue and player lookup.

**Moderation.** `admin_set_moderator` appoints moderators. They use `mod_open_reports`, `mod_resolve_report` (dismiss, warn, mute or suspend), `mod_mute`, `mod_ban` and `mod_unban`. Moderators can't act on themselves, other moderators or admins, and a suspended moderator loses their powers. A suspension also keeps a player out of tournament play (no sitting back in, acting, rebuys or add-ons). `admin_set_banned_words` replaces the word filter. Every admin and moderator action lands in the audit log (`admin_audit_log`).

**Monitoring.** `https://<backend-id>.raw.icp0.io/metrics` serves Prometheus text: players (registered, active in the last 5 minutes, seated), tables, hands dealt, cycles, heap and stable memory, treasury, liabilities, jackpots, unresolved and failed ledger ops, the last solvency check, pause switches, reports, bans and rate-limited calls. `/health` returns `ok`. Responses are uncertified and contain nothing private, so use them for dashboards and alerts only. Controllers can also call `admin_get_metrics`. Suggested alerts: `bp_solvency_ok == 0`, `bp_unresolved_ledger_ops > 0` for 30 minutes, `bp_cycles` below 2T, and no increase in `bp_hands_dealt_total` while `bp_seated_players > 1`.

**Frontend headers.** `.ic-assets.json5` sets a strict Content-Security-Policy (own scripts only, API calls only to IC boundary nodes), Permissions-Policy, HSTS and anti-framing headers. If you add an external host (analytics, fonts), add it to the CSP.

**Testing.** `cargo test` runs the unit tests (game engine, state, wallet logic, an upgrade round-trip). `./scripts/integration-test.sh` runs the canister against the real ICP ledger on a PocketIC replica: deposits both ways, withdrawals, exact solvency, pause switches, an upgrade in the middle of a hand, the ingress filter and rate limits, moderation and the metrics endpoint. CI runs both.

## Before taking real money: read this

- **Legal.** Real-money poker is regulated or prohibited in many jurisdictions. Get legal advice, and expect to need licensing, geo-blocking, age verification/KYC and responsible-gambling tooling before launching for real ICP. A play-chips mode is an easy addition if you want a free-to-play launch first.
- **Card secrecy.** Hole cards are returned by query calls, which a single replica answers. A malicious node could see them. The planned fix is to encrypt each player's hole cards with [vetKeys](https://internetcomputer.org/docs/building-apps/network-features/vetkeys/introduction) so only the player's browser can decrypt them.
- **Collusion and bots.** Players can report collusion, but there is no automated collusion or bot detection yet.
- **Scale.** All state is in one canister and serialized on upgrade. That is fine to start with. For growth, move to `ic-stable-structures` and shard tables across canisters.
- **Audit.** Have the wallet and ledger code professionally audited before holding meaningful funds.

## Roadmap ideas

vetKeys-encrypted hole cards · Omaha and Crazy Pineapple · bounty tournaments · animated avatars as images/NFTs · automated collusion detection · multisig control of the canister (e.g. Orbit).

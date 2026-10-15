# ♠ Bullets Poker

A social Texas Hold'em poker room that runs entirely on the [Internet Computer](https://internetcomputer.org), in the spirit of TripleJack, with **ICP as the currency**. Players use ICP to buy in at tables and to buy avatars, card backs and table gifts.

## Features

**Poker**
- No-limit Texas Hold'em cash tables (heads-up up to 10-max) with blinds, a moving button, min-raise rules, all-ins, side pots, split pots and odd-chip handling
- Action clock: a player who times out is checked or folded and sat out ("I'm back" to return)
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
- Absent players keep getting dealt in and blind off, so every event finishes; tournament chips can't be cashed out, and prizes go straight to your wallet

**Host your own table**
- Any player can open a table (chips or ICP) with their choice of stakes, seats and clock, optionally password protected
- Hosts of ICP tables earn 20% of the rake their table generates; empty tables close after 30 minutes

**Medals**
- 19 medals for milestones (first pot, 1,000 hands), big hands (quads, royal flush, bad beats), social play (gifts, throwables, friends) and tournament results, shown on profiles and in player cards

**Hand history & settings**
- Your last 50 hands with a step-by-step replay (board, pot and action log). Only your own cards and hands shown at showdown are revealed
- Settings saved to your profile: four-color deck, auto-muck losing hands, auto top-up, sounds, show/hide throwables

**Social**
- Lobby chat and per-table chat with quick emotes, plus rate limiting
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
    src/ledger.rs           ICP ledger calls (ICRC-1 transfer, ICRC-2 transfer_from), deposit accounts
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

## Before taking real money: read this

- **Legal.** Real-money poker is regulated or prohibited in many jurisdictions. Get legal advice, and expect to need licensing, geo-blocking, age verification/KYC and responsible-gambling tooling before launching for real ICP. A play-chips mode is an easy addition if you want a free-to-play launch first.
- **Card secrecy.** Hole cards are returned by query calls, which a single replica answers. A malicious node could see them. The planned fix is to encrypt each player's hole cards with [vetKeys](https://internetcomputer.org/docs/building-apps/network-features/vetkeys/introduction) so only the player's browser can decrypt them.
- **Collusion and bots.** There is no anti-collusion or bot detection yet.
- **Scale.** All state is in one canister and serialized on upgrade. That is fine to start with. For growth, move to `ic-stable-structures` and shard tables across canisters.
- **Audit.** Have the wallet and ledger code professionally audited before holding meaningful funds.

## Roadmap ideas

vetKeys-encrypted hole cards · Omaha and Crazy Pineapple · bad-beat jackpot · rebuys and late registration · animated avatars as images/NFTs · moderators and mute/report · auto-stand for players sitting out too long.

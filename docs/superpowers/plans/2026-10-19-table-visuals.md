# Table Visuals Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the emoji avatars, gifts and throwables with outlined-cartoon SVG art. Throws land on the avatar and leave a mark, gifts arrive and idle, and avatars react to winning and to bad beats.

**Architecture:**
- An art registry keyed by the item's emoji maps each default item to an inline SVG React component. Anything unknown falls back to the emoji.
- One `<Avatar>` component renders avatars everywhere and takes an `expression` that CSS uses to swap face groups.
- Table effects (throw marks, gift arrivals, hand reactions) are per-seat state in `Table.tsx`, driven by pure, unit-tested functions.
- The only backend change adds two read-only fields to `SeatView`, so the client knows who sent a gift and when.

**Tech Stack:** Rust canister (`src/poker_backend`), React 19 + TypeScript + Vite (`src/poker_frontend`), plain CSS (`styles.css`), and vitest (new, dev only).

**Spec:** `docs/superpowers/specs/2026-10-19-table-visuals-design.md`. Read it first. It holds the per-item visuals, timings, reaction rules and the calls already made.

## Global Constraints

- **Art style:** outlined cartoon, matching the "B — Outlined cartoon" Cool Cat in the brainstorm mockup.
  - Ink outlines are `#1b1b1b`, about 3px at a 120×120 viewBox.
  - Gradient shading.
  - Round coloured background for avatars.
  - The mockup file is `.superpowers/brainstorm/765-1792416991/content/tomato-hit-v3.html`. It is gitignored but present locally; copy its SVG and keyframes as a starting point.
- **Dependencies:** no new runtime dependencies. `vitest` is the only new dev dependency.
- **CSP:** no external images or fonts and no inline `<script>`. Art is inline SVG in TSX; animation is CSS keyframes in `styles.css`.
- **Motion:**
  - Animate only `transform` and `opacity`, with no per-frame JS loops.
  - Under `@media (prefers-reduced-motion: reduce)`, idle animations stop, and flights and bursts become a ≤200ms opacity fade. Marks and end states still show.
- **Layering:** effects never cover hole cards, the nameplate, the timer, the action tag or bet chips. Marks are clipped to the avatar circle.
- **Fallback:** an emoji with no art renders exactly as today (the emoji text).
- **SVG ids:** gradient and clip ids are unique per rendered instance. Use React `useId()` with non-alphanumerics stripped.
- **Settings:** throws, marks and hit reactions obey `settings.show_throwables` and are skipped when the table is not `visible`, as today. Gift arrivals and hand reactions always play. The throw sound (`sounds.splat`) stays.
- **Unchanged:** backend gift duration (10 min, newer replaces older), prices and the catalogue.

## Review Focus

1. An admin-added item with an unknown emoji (say 🦖) shows that emoji in the seat, store, picker and flight, with no crash and no blank. Tested in Task 3.
2. A repeat gift of the same item to the same player plays the arrival again, because it's keyed on `gift_sent_at`, not on the emoji. Tested in Task 6.
3. Side pots and mucks:
   - A shown player who wins a side pot gets no bad-beat reaction.
   - A mucked loser gets none.
   - A hand that ended before the river gets none.

   Tested in Task 2.
4. Two players with the same avatar on one page don't share gradient ids, so neither renders unshaded. Tested in Task 3.
5. In the compact multi-table layout and the `avatar sm` lists, marks and reactions stay inside the avatar and don't overflow onto cards or chips. Checked visually in Task 8.

---

### Task 1: Backend — gift sender and sent time in SeatView

**Files:**
- Modify: `src/poker_backend/src/state.rs` (`SeatView` ~line 445, `full_table_view` ~line 2030, `table_view` restricted map ~line 1987, test `store_and_gifts` ~line 2168)
- Regenerate: `src/poker_backend/poker_backend.did` and `src/poker_frontend/src/declarations/poker_backend/*`

**Interfaces:**
- Produces two new `SeatView` fields after `gift`:
  - `gift_from: Option<String>`: the sender's username, from `ActiveGift.from`.
  - `gift_sent_at: Option<u64>`: `expires_at - GIFT_DURATION_NS`.

  Both are `None` whenever `gift` is `None`, including expired gifts and the restricted view.
- In TypeScript they arrive as `gift_from: [] | [string]` and `gift_sent_at: [] | [bigint]`.
- No storage change, so upgrades stay compatible.

- [ ] **Step 1: Extend `store_and_gifts` with failing assertions.** After the first `send_gift`, the seat view has `gift_from == Some("player1")` and `gift_sent_at == Some(0)`. After a second `send_gift` of the same beer at `now = 5 * SEC`, `gift_sent_at == Some(5 * SEC)` and `gift` is still `"🍺"`. After expiry, all three are `None`.
- [ ] **Step 2:** Run `cargo test -p poker_backend store_and_gifts`. Expected: a compile failure on the missing fields.
- [ ] **Step 3:** Add the fields, fill them in `full_table_view` from the same filtered `ActiveGift`, and set both to `None` in the restricted map in `table_view`.
- [ ] **Step 4:** Run `UPDATE_CANDID=1 cargo test`, then `cargo test`. Expected: all pass, and the `.did`, `.did.d.ts` and `.did.js` show the two new fields.
- [ ] **Step 5:** Run `npm --prefix src/poker_frontend run typecheck`. Expected: pass, since the new fields are additive.
- [ ] **Step 6:** Commit with message `Send the gift's sender and time with each seat`.

---

### Task 2: Hand evaluator and hand reactions (pure, tested), plus test runner

**Files:**
- Modify: `src/poker_frontend/package.json`. Add devDependency `"vitest": "5.0.5"` (its peer range includes vite 8), script `"test": "vitest run"`, and run `npm install` to update the lockfile.
- Create: `src/poker_frontend/vitest.config.ts`. It contains only `@vitejs/plugin-react` and `test: { environment: 'node' }`. It must not import `vite.config.ts`, whose dev-server hook shells out to `icp`.
- Modify: `.github/workflows/ci.yml`. Add a `- run: npm test` step after `npm run build` in the frontend job.
- Create: `src/poker_frontend/src/fx/poker.ts`
- Create: `src/poker_frontend/src/fx/poker.test.ts`

**Interfaces:**
- Produces from `fx/poker.ts`:
  - `export function handScore(cards: number[]): number`. It takes 5–7 cards and returns the best 5-card hand's score. Higher is better and equal means a tie. It uses the card encoding from `cards.rs`: `rank = c >> 2` (0 = deuce .. 12 = ace) and `suit = c & 3`. The categories rank from high card up to straight flush, and the wheel A-2-3-4-5 is the lowest straight.
  - `export type Reaction = 'badBeat' | 'win' | 'bigWin'`
  - `export interface ReactionInput { result: { board: number[]; shown: { seat: number; cards: number[] }[]; winners: { seat: number; amount: bigint; hand: string }[] }; bigBlind: bigint; allInSeats: Set<number> }`
  - `export function handReactions(input: ReactionInput): Map<number, Reaction>`, keyed by seat.

**Rules for `handReactions`** (from the spec):
- **Uncontested:** if every winner's `hand === 'Uncontested'`, each winner gets `'win'` and nothing else applies.
- **Shown winners:** `W` is the winners whose seat appears in `shown`.
- **Bad beat:** a shown seat that isn't among the winners is `'badBeat'` when all of these hold:
  - `board.length === 5`,
  - `W` is non-empty,
  - `handScore(hole + board[0..4])` is strictly greater than the same score for every member of `W`.
- **Big win:** a winner is `'bigWin'` if any of these hold, otherwise `'win'`:
  - `amount >= 40n * bigBlind`,
  - some shown seat is in `allInSeats`,
  - the winner is in `W` and some seat is `'badBeat'`.

- [ ] **Step 1: Write the `handScore` tests.**
  - Royal flush beats four of a kind.
  - A full house beats a flush.
  - The wheel loses to a six-high straight.
  - With two pair against two pair, the kicker decides.
  - 7 cards pick the best 5: the seven `[A♠ K♠ Q♠ J♠ T♠ 2♦ 3♣]` equal the score of `[A♠ K♠ Q♠ J♠ T♠]`.
  - Identical ranks in different suits tie.

  Build the cards with a helper `c('As')`.
- [ ] **Step 2: Write the `handReactions` tests.**
  - **Classic bad beat:** the board is `Kh 7d 2c 9s` plus `Ah` on the river. Seat 1 holds `K♠ K♦` and loses to seat 2's `A♣ A♦`; seat 2 is the only winner. Expect seat 1 → `'badBeat'` and seat 2 → `'bigWin'`.
  - **Side-pot winner** (Review Focus 3): same hand, but seat 1 also appears in `winners` with a side-pot amount. Expect seat 1 to be absent or a win reaction, not `'badBeat'`.
  - **Mucked loser:** seat 1 is not in `shown`. Expect no `'badBeat'`.
  - **Board of 4 or fewer cards:** expect no `'badBeat'`.
  - **Tie on the turn:** expect no `'badBeat'`.
  - **Uncontested:** `hand: 'Uncontested'` with an amount of 1000 BB. Expect `'win'`, not `'bigWin'`.
  - **Big pot:** `amount = 40n * bb` gives `'bigWin'`, and `39n * bb` with no all-in gives `'win'`.
  - **All-in showdown:** a shown seat in `allInSeats` makes the winner `'bigWin'`.
- [ ] **Step 3:** Run `npm --prefix src/poker_frontend test`. Expected: FAIL, because the module is missing.
- [ ] **Step 4:** Implement `fx/poker.ts`. For `handScore`, check every 5-card subset, score each as `category * 13^5 + kickers` in base 13, and keep the maximum.
- [ ] **Step 5:** Run `npm --prefix src/poker_frontend test` and `npm --prefix src/poker_frontend run build`. Expected: both PASS.
- [ ] **Step 6:** Commit with message `Add a hand evaluator and the hand reaction rules`.

---

### Task 3: Art registry, `<Avatar>`, the 10 avatars, and replacing every avatar emoji

**Files:**
- Create: `src/poker_frontend/src/art/registry.ts`
- Create: `src/poker_frontend/src/art/Avatar.tsx`
- Create: `src/poker_frontend/src/art/avatars.tsx`, holding the 10 avatar SVG components
- Create: `src/poker_frontend/src/art/art.test.tsx`
- Modify: `styles.css` (the `.avatar` rules ~line 95, plus new expression, idle and reduced-motion rules)
- Modify: every avatar render site:
  - `App.tsx:91`
  - `Profile.tsx:23,88-95,182`
  - `Friends.tsx:55,71,90`
  - `Lobby.tsx:111`
  - `Notes.tsx:124`
  - `Tournaments.tsx:225`
  - `Table.tsx:370-374` (seat) and `Table.tsx:628` (PlayerCard)

**Interfaces:**
- **`art/registry.ts`:**
  - `export type ArtKind = 'avatar' | 'gift' | 'throwable'`
  - `export type Expression = 'normal' | 'hit' | 'sad' | 'happy'`
  - `export interface ArtProps { uid: string }`. Every art component draws in `viewBox="0 0 120 120"` and prefixes all its ids with `uid`.
  - `export interface ArtEntry { kind: ArtKind; name: string; Svg: (p: ArtProps) => ReactElement }`
  - `export function registerArt(icon: string, entry: ArtEntry): void`
  - `export function artFor(icon: string): ArtEntry | undefined`
- **`art/Avatar.tsx`:** `export function Avatar(p: { icon: string; size?: 'sm' | 'md' | 'lg'; expression?: Expression; className?: string; title?: string; children?: ReactNode })`.
  - It renders `<span className={`avatar ${size} ${className}`} data-expr={expression} title={title}>`. Inside are the art's `<Svg uid>` (or the emoji when `artFor` misses) followed by `children`.
  - `md` adds no class, because the seat size is the existing default `.avatar`.
- **Avatar SVG contract** (CSS relies on these class names):
  - The face groups are `.face-normal`, `.face-hit`, `.face-sad` and `.face-happy`. Only the group matching `data-expr` is visible, and a missing or `normal` value shows `.face-normal`.
  - `.eyes` is inside `.face-normal` and gets the blink keyframe (`scaleY` toward 0.1 for about 120ms every 4–6s). Each instance gets a randomised `animation-delay` via an inline style set from the uid hash.
  - `.acc` is an optional accessory group (shades, hat, crown). It tilts under `[data-expr=hit]`.
  - The whole svg gets a slow breathing `scale(1→1.02)` loop. It is not applied to `.sm`.
- Emoji keys, per `default_items()` in `state.rs:581`: 🙂 Rookie, 😎 Cool Cat, 🤖 Robot, 🤠 Cowboy, 👽 Alien, 🥷 Ninja, 🦈 Shark, 🐉 Dragon, 🦄 Unicorn, 👑 Crown. Crown is a royal character wearing a crown.

- [ ] **Step 1: Write the tests in `art.test.tsx`**, using `renderToStaticMarkup` from `react-dom/server`.
  - Every one of the 10 avatar emoji has `artFor(e)?.kind === 'avatar'`.
  - `<Avatar icon="🦖" size="sm" />` markup contains `🦖` and no `<svg` (Review Focus 1).
  - Rendering two `<Avatar icon="😎" />` in one tree yields no duplicate `id="` values (Review Focus 4).
  - `<Avatar icon="😎" expression="hit" />` markup contains `data-expr="hit"`.
- [ ] **Step 2:** Run `npm --prefix src/poker_frontend test`. Expected: FAIL.
- [ ] **Step 3:** Implement the registry, `Avatar`, the 10 avatars and the CSS: expression visibility, blink, breathing, `.acc` tilt and the reduced-motion overrides. Size the svg to fill the circle and drop `font-size`, which is now only for the fallback.
- [ ] **Step 4:** Replace every avatar render site listed above with `<Avatar>`.
  - Keep the existing extra classes (`note-*`, `pick active`).
  - On the seat, pass the gift badge and note mark as `children`.
  - `Profile.tsx`'s picker keeps its click handler on the wrapper element.
- [ ] **Step 5:** Run `npm --prefix src/poker_frontend test` and `npm --prefix src/poker_frontend run build`. Expected: PASS. Then run `grep -rn "avatar sm\">{\|avatar lg\">{" src/poker_frontend/src`. Expected: no matches.
- [ ] **Step 6:** Commit with message `Draw the avatars as outlined cartoon art`.

---

### Task 4: Gift and throwable art, plus the store and picker

**Files:**
- Create: `src/poker_frontend/src/art/items.tsx`, holding the 8 gift and 7 throwable SVG components, registered with `registerArt`
- Create: `src/poker_frontend/src/art/ItemArt.tsx`
- Modify: `art/art.test.tsx`, `Store.tsx:62`, and the picker in `Table.tsx:616`

**Interfaces:**
- **Consumes:** `registerArt`, `artFor`, `ArtProps` from Task 3.
- **Produces:** `export function ItemArt(p: { icon: string; className?: string })`. It renders `<span className={`item-art ${className}`}>` around the svg, or the emoji as a fallback. It is sized by the parent's `width`/`height` or `font-size`.
- Gifts: 🍺 Beer, ☕ Coffee, 🌹 Rose, 🍸 Cocktail, 🍕 Pizza, 🚬 Cigar, 🍾 Champagne, 🏆 Trophy. Each SVG gets an `.idle` group for Task 6 to animate: foam, steam, petals, gleam, steam, smoke, bubbles and shine respectively.
- Throwables: 🍅 Tomato, 🥧 Pie, ❄️ Snowball, 🐟 Fish, 💋 Kiss, 💸 Money, 💣 Bomb.

- [ ] **Step 1:** Add tests. Every gift emoji maps to `kind === 'gift'` and every throwable emoji to `kind === 'throwable'`. `<ItemArt icon="🦖" />` renders the emoji. The ❄️ key matches the catalogue string exactly, including U+FE0F.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement the art and `ItemArt`. In `Store.tsx` use `<ItemArt>` for non-CardBack items; for avatars use `<Avatar size="lg">`. In the Table picker, use `<ItemArt>` in `.gift-btn .icon`.
- [ ] **Step 4:** Run `test` and `build`. Expected: PASS.
- [ ] **Step 5:** Commit with message `Draw gifts and throwables as outlined cartoon art`.

---

### Task 5: Throws that hit — flight, mark and flinch

**Files:**
- Create: `src/poker_frontend/src/fx/SeatFx.tsx`, holding the mark overlays and per-seat effect state
- Create: `src/poker_frontend/src/fx/marks.tsx`, one mark component per throwable
- Modify: `Table.tsx` (the throw effect at lines 115-131, the `.flying` render at 400-408, the seat avatar), `styles.css` (replace the `.flying`/`fly`/`splat` rules at 214-220)
- Test: `src/poker_frontend/src/fx/fx.test.ts`

**Interfaces:**
- **Consumes:** `ItemArt` (Task 4), `Avatar` with `expression` (Task 3).
- **Produces in `fx/SeatFx.tsx`:**
  - `export type MarkKind = 'tomato' | 'pie' | 'snow' | 'fish' | 'kiss' | 'money' | 'bomb'`
  - `export const THROW_FX: Record<string, { mark: MarkKind; expression: Expression }>`, keyed by emoji. 💸 maps to `'happy'`; all the others map to `'hit'`.
  - `export interface SeatEffect { expression?: Expression; mark?: MarkKind; overlay?: ReactNode; key: number }`
  - `export function useSeatFx(): { fx: Map<number, SeatEffect>; play(seat: number, effect: Omit<SeatEffect, 'key'>, ms: number): void }`.
    - `play` replaces any running effect on that seat, so a repeat hit restarts it.
    - It clears the effect after `ms` and adds the `cleaning` class for the last 700ms.
    - It clears its timers on unmount.
  - `export function Mark(p: { kind: MarkKind; uid: string })`, an absolutely positioned SVG inside the avatar, clipped to the circle.
- **Timeline** (spec):
  - The flight takes about 700ms on an arc with spin, using `ItemArt`.
  - On impact the flying element is removed, and `play(toSeat, { mark, expression }, 4700)` runs.
  - Particles burst, the avatar shakes, and the mark wipes during the final 700ms.
  - With an unknown throwable emoji there is no mark: the existing emoji flight and fade is kept, with a `'hit'` flinch.

- [ ] **Step 1:** Test that `THROW_FX` has an entry for all 7 throwable emoji, and that 💸 maps to `'happy'`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement the marks per the spec's table: tomato splat and drips, pie cream sliding, snow frost plus a shiver, fish slap with sideways knock and stars, kiss lipstick plus blush and hearts, money bills fluttering, bomb blast then soot and frazzle. Implement `useSeatFx`, wire the throw effect (keeping the `show_throwables`/`visible` guards and `sounds.splat`), and pass `expression` and the `<Mark>` child to the seat `<Avatar>`.
- [ ] **Step 4:** Run `test` and `build`. Expected: PASS.
- [ ] **Step 5:** Commit with message `Make throws land on the avatar`.

---

### Task 6: Gift arrival and idle

**Files:**
- Create: `src/poker_frontend/src/fx/gifts.ts`
- Modify: `fx/fx.test.ts`, `Table.tsx` (the seat gift badge at line 372, plus a new effect next to the throw effect), `styles.css` (`.avatar .gift` at line 99, plus gift idle and arrival keyframes)

**Interfaces:**
- **Consumes:** `SeatView.gift`, `gift_from` and `gift_sent_at` (Task 1); `ItemArt` (Task 4).
- **Produces in `fx/gifts.ts`:** `export function giftArrivals(prev: Map<number, bigint> | null, seats: { seat: number; username: string; gift: [] | [string]; gift_from: [] | [string]; gift_sent_at: [] | [bigint] }[]): { next: Map<number, bigint>; arrivals: { seat: number; icon: string; fromSeat: number | null }[] }`
  - With `prev === null` (first load), `arrivals` is empty.
  - An arrival happens when a seat's `gift_sent_at` differs from `prev`.
  - `fromSeat` is the seat whose `username === gift_from`, or `null` when the sender isn't seated.

- [ ] **Step 1:** Write the tests:
  - First load gives no arrivals.
  - A new gift from a seated sender gives `fromSeat` set to that seat.
  - A sender who isn't seated gives `null`.
  - **Repeat gift** (Review Focus 2): the same icon with a new `gift_sent_at` gives one arrival.
  - An unchanged `gift_sent_at` gives no arrival.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement `giftArrivals` and wire it into Table.
  - **Arrival:** when `fromSeat !== null`, an `ItemArt` flies from that seat to the receiver over about 700ms. Otherwise it drops from above the receiver. Then it pops with a sparkle in the badge spot.
  - **Badge:** the seat shows `<ItemArt className="gift idle" icon={gift} />`, with per-gift idle keyframes targeting the `.idle` groups from Task 4. It fades out when `gift` becomes empty.
  - **Reduced motion:** the badge just fades in.
- [ ] **Step 4:** Run `test` and `build`. Expected: PASS.
- [ ] **Step 5:** Commit with message `Animate gifts arriving and idling at the seat`.

---

### Task 7: Hand reactions at HandOver

**Files:**
- Modify: `Table.tsx` (next to the win-sound effect at lines 89-112, the seat render, the `.seat` class), `fx/SeatFx.tsx` (overlays), `styles.css`

**Interfaces:**
- **Consumes:** `handReactions` and `Reaction` (Task 2); `useSeatFx().play` (Task 5); `view.config.big_blind`, `view.seats[i][0].all_in`, `view.jackpot_hit`.
- **Produces in `SeatFx.tsx`:** `export function RainCloud()`, `export function ChipBurst(p: { big: boolean })` and `export function BadBeatTag()`.

- [ ] **Step 1:** Wire it up. When `phase === 'HandOver'` and `last_result[0].hand_no` hasn't reacted yet (tracked in a ref, skipped on first load, like throws), compute `handReactions` with `board`, `shown`, `winners`, `bigBlind = view.config.big_blind`, and `allInSeats` taken from the seats' `all_in`. Then call `play` for each seat with `ms = 4500`:
  - **`badBeat`:** expression `'sad'`, a `RainCloud` overlay above the avatar with a drizzle and a tear, and a head-droop class. Show a `BadBeatTag` near the nameplate unless `view.jackpot_hit[0]?.hand_no === result.hand_no`.
  - **`win`:** expression `'happy'`, a bounce, and `ChipBurst big={false}`.
  - **`bigWin`:** expression `'happy'`, a bigger bounce, `ChipBurst big` with confetti, and a gold glow on the seat ring via a `big-win` class on `.seat`.
  - If the `JackpotCelebration` modal covers the seats, these still run underneath. Confirm visually in Task 8 that they're visible after closing it, or around it.
- [ ] **Step 2:** Run `npm --prefix src/poker_frontend test` and `build`. Expected: PASS.
- [ ] **Step 3:** Commit with message `Add winner and bad beat reactions`.

---

### Task 8: Local end-to-end visual check

**Files:**
- Create (scratchpad, not committed): a node probe script modelled on the existing scratchpad `show-probe.mjs`.
  - It registers bots, seats them at a chip table, and opens a signed-out watcher browser with Playwright (`channel: 'chrome'`).
  - It throws each of the 7 throwables, sends a gift twice (same item, from a seated bot), plays a hand to showdown, and screenshots each moment.

- [ ] **Step 1:** In WSL Ubuntu-24.04, run `./scripts/deploy-local.sh`. The network must already be running in a held foreground session; see the memory note `local-deploy-wsl`. Expected: the frontend URL is printed.
- [ ] **Step 2:** Run the probe. Expected: a screenshot for each throwable mid-mark, the gift arrival twice, and the winner or bad-beat reaction. Look at every screenshot and check:
  - Every item shows art, not emoji.
  - Marks stay inside the avatar circle.
  - Nothing covers cards, nameplates or chips.
- [ ] **Step 3:** Repeat one throw screenshot at the compact multi-table size (open 2+ tables) and at a phone width of 390px (Review Focus 5).
- [ ] **Step 4:** Repeat once with Chrome's reduced-motion emulation (`page.emulateMedia({ reducedMotion: 'reduce' })`). Expected: no flight or idle motion, and marks and reactions still appear.
- [ ] **Step 5:** Fix anything found and commit with message `Polish table visuals after the local check`.

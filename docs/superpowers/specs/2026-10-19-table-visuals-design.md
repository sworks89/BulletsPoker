# Table visuals: illustrated avatars, gifts and throwables

Date: 2026-10-19
Status: draft, awaiting approval

## Why

The table feels lifeless. Avatars, gifts and throwables are plain emoji: a thrown
tomato spins across the felt, grows and fades, and the player it hits never
reacts. Gifts show up as a small emoji badge. The owner wants the table to feel
like TripleJack: illustrated characters, and items that land on people and get
a reaction.

## Goals

1. Replace every emoji avatar, gift and throwable in the default catalogue with
   vector illustrations in one **outlined cartoon** style. The owner picked this
   style from a mockup. It uses thick dark ink outlines (about 3px at a 120px
   avatar), gradient shading, and a little costume or character detail. The
   reference is the "B — Outlined cartoon" Cool Cat in the brainstorm mockup
   (`.superpowers/brainstorm/*/content/tomato-hit-v3.html`).
2. **Throwables hit the avatar.** Each throw arcs from the thrower's seat to the
   target and lands on the target's avatar. It leaves a mark on the face (the
   owner asked for splatter that affects the avatar). The avatar flinches, and
   the mark clears after a few seconds.
3. **Gifts arrive and stay.** A gift flies from the sender to the receiver and
   lands beside the receiver's avatar with a pop. It stays there with a small
   idle animation for the existing 10 minutes, or until a newer gift replaces it.
4. **Avatars react to the hand.**
   - **Bad beat:** a showdown loser who was ahead before the river gets a sad
     reaction.
   - **Winner:** every pot winner gets a cheer, with a bigger celebration for
     big wins.
5. **Avatars idle.** They blink every few seconds and breathe gently, so a quiet
   table doesn't look frozen.

## Non-goals

- No pricing or gameplay changes. The store, buying, equipping, the gift
  duration (10 minutes, newer replaces older) and the throw cooldown all stay as
  they are.
- The only backend change is the read-only gift sender info described under
  Gifts. The owner approved it.
- No new catalogue items. The same 10 avatars, 8 gifts and 7 throwables are
  redrawn.
- Card backs keep their current look.
- No new sounds. The existing `sound.ts` cues may be triggered where they already
  fit.

## Anti-goals (failure even if it "works")

- An admin-added store item with an emoji the art set doesn't know must not
  break or disappear. It falls back to rendering its emoji exactly as today.
- Animations must not hide game information while it matters. That covers hole
  cards, the action timer, stack, bet and the action tag. A splat covers the
  avatar face only, never the cards or the nameplate.
- Animations must not make the table janky. Use CSS transforms and opacity only,
  with no layout thrash, and no per-frame JavaScript loops.
- A mix of styles. Anything that still shows the old emoji for a default item is
  a bug.

## Constraints

- **Frontend:** `src/poker_frontend` (React 19, Vite, TypeScript, plain CSS in
  `styles.css`), plus the small backend addition under Gifts. No new runtime
  dependencies. Art is inline SVG in React
  components, and animation is CSS keyframes, as the mockup does it.
- **Art lookup by emoji.** The backend only sends the item's `icon` string
  (`SeatView.avatar`, `SeatView.gift`, `ThrowEvent.icon`, `StoreItem.icon`), so
  the art registry is keyed by that emoji. Unknown keys fall back to the emoji.
- **Strict CSP** (`public/_headers`): no external images or fonts, and no inline
  `<script>`. Inline SVG and the existing stylesheet are fine.
- **Respect `prefers-reduced-motion`.** Under it, idle animations stop. Throws,
  gifts and reactions use a short fade instead of motion; the mark and the end
  state still show.
- Avatars render at several sizes: seat (about 56–64px), `avatar lg` (profile,
  player popup, store) and `avatar sm` (lists). The art must read at the
  smallest size, so detail scales down or simply blurs out acceptably.
- **Keep SVG ids unique** per instance (gradients in particular), because the same
  avatar can appear many times on one page.

## Behaviour

### Where the art appears
Everywhere a default avatar, gift or throwable currently renders as emoji:
- The table seat, and the player popup (`Table.tsx`).
- The gift/throw picker in the player popup.
- The flying throw.
- The store grid (`Store.tsx`).
- The profile and avatar picker (`Profile.tsx`).
- The header (`App.tsx`).
- Lists in `Friends.tsx`, `Lobby.tsx`, `Notes.tsx` and `Tournaments.tsx`.

Medals, and the emoji inside chat messages, are out of scope.

### Avatars (10)
Rookie 🙂, Cool Cat 😎, Robot 🤖, Cowboy 🤠, Alien 👽, Ninja 🥷, Shark 🦈,
Dragon 🐉, Unicorn 🦄, Crown 👑.

Each is a head-and-shoulders character on a round, coloured background, with the
character's personality matching its store description. Crown is a royal
character wearing a crown, not the crown alone.

Every avatar has these expression states, swapped in by CSS class:
- **normal:** blinks every few seconds, randomised per seat so a table doesn't
  blink in unison, and has a slight breathing scale.
- **hit:** eyes squeezed shut, mouth open, plus a jolt or shake. Accessories such
  as sunglasses or a hat can be knocked askew.
- **sad:** droopy eyes and a frown, for the bad beat.
- **happy:** a big grin with closed or sparkly eyes, for winning.

### Throwables (7)
Common flow:
1. The item flies in an arc from the thrower's seat to the target's avatar,
   spinning; the current flight takes about 0.7 s.
2. On impact the item disappears, its mark appears on the face, particles
   burst, and the avatar switches to **hit** and shakes.
3. The mark stays about 4 s, then wipes or fades off, and the avatar returns to
   **normal**.

Per-item marks (the builder may refine these):

| Item | Mark / effect |
|---|---|
| 🍅 Tomato | Red splat with seeds and running drips (the mockup) |
| 🥧 Pie | Cream and crust covering the face, sliding down slowly |
| ❄️ Snowball | White powder burst and frost on the face; the avatar shivers |
| 🐟 Fish | A slap: the avatar is knocked sideways, with a wet streak and a star or two |
| 💋 Kiss | A lipstick mark on the cheek, blush, and a couple of floating hearts |
| 💸 Money | Bills burst and flutter down over the avatar, which looks pleased (it switches to **happy**, not **hit**) |
| 💣 Bomb | A cartoon blast cloud, then the face is left sooty with frazzled edges; the avatar is stunned |

Throws already present on first load are skipped, as now. Two throws landing on
the same target close together restart the effect rather than stacking.

### Gifts (8)
- **Backend addition (approved by the owner).** Today `SeatView.gift` is only
  the emoji, so the client can't tell who sent a gift or notice a repeat gift
  with the same emoji. The change:
  - Add to `SeatView` the sender's principal or `None`, and the time the
    current gift was sent (`expires_at - GIFT_DURATION_NS`), or carry
    `ActiveGift` as a record. Storing the sender's principal in `ActiveGift`
    next to the existing name is fine.
  - Read-only: nothing about buying, pricing or duration changes.
  - Update the `.did`, regenerate the bindings (`UPDATE_CANDID=1 cargo test`),
    and keep stable-memory upgrades compatible: a new field must be optional
    or defaulted.
- **Arrival:**
  - If the sender is seated at this table, the gift flies from their seat to
    the receiver.
  - Otherwise it drops in from above the receiver.
  - It lands beside the avatar with a pop and a short sparkle.
- **Detecting arrival:** a change in the gift's sent time marks a new gift,
  including a repeat of the same item. A gift present on first load is shown
  without the arrival.
- **While shown,** each gift has a subtle idle animation:
  - 🍺 Beer: foam.
  - ☕ Coffee: steam.
  - 🌹 Rose: sway.
  - 🍸 Cocktail: a gleam.
  - 🍕 Pizza: steam.
  - 🚬 Cigar: smoke wisps.
  - 🍾 Champagne: bubbles.
  - 🏆 Trophy: a periodic shine.
- **Duration and replacement:** unchanged, decided by the backend. When `gift`
  goes empty, the gift fades out.

### Hand reactions
Both reactions play during the HandOver pause, about 5 s, while the result is
on screen. Everyone at the table sees them.

**Bad beat**
- **Qualifies when** the hand reached a showdown with all 5 board cards, and a
  shown losing hand was strictly better than every winner's hand on the first
  4 board cards (flop plus turn). Both hands use their own hole cards. A tie on
  the turn doesn't count.
- **Mucked hands** have no cards, so they get no reaction.
- **Computed in the frontend** from `HandResult.board`, `HandResult.shown` and
  `HandResult.winners`. That needs a small TypeScript hand evaluator: the best
  5 of 6 cards, with an ordering consistent with the backend's `cards.rs`.
- **Visual:**
  - A small rain cloud drifts over the avatar and drizzles.
  - The avatar switches to **sad**, its head droops, and a tear runs down.
  - A cracked "Bad beat" tag flashes near the nameplate.

**Winner**
- **Every winner:** the avatar switches to **happy** and bounces, with a short
  burst of chips from the avatar.
- **Big win:** a bigger celebration (confetti, a gold glow on the seat ring, a
  longer bounce) for a winner who meets any of these:
  - The pot is at least 40 big blinds (the builder's threshold call).
  - The hand was won at an all-in showdown, meaning at least one shown player
    was all-in.
  - The player won the hand that was someone else's bad beat.
- **Uncontested wins** (everyone folded) get the small cheer only.

**Calls made while writing this (not the owner's; flagged for review)**
- **Who counts in a bad beat:**
  - The loser is a shown player who won nothing in any pot. A player who
    takes a side pot doesn't get the sad reaction.
  - The comparison is against every shown winner who took a pot. The loser
    must have been strictly ahead of all of them on the turn.
  - "Won someone else's bad beat" means any of those winners.
- **Big win threshold:** that winner's own `Winner.amount` is at least
  40 × the table's big blind at HandOver, using the current level in
  tournaments.
- **Bad-beat jackpot:**
  - When `view.jackpot_hit` is showing its celebration, the cracked "Bad beat"
    tag is skipped, because the jackpot overlay already says it.
  - The avatar reactions (sad, happy) still play, and the jackpot's winner gets
    the big celebration.
  - If the overlay covers the seats, the reactions play after it closes or
    remain visible around it. The builder chooses whichever looks right.
- **Settings:**
  - Splats, hit reactions and flying throws obey the existing "show
    throwables" setting, as throws do today.
  - Gift arrivals and hand reactions always play. Reduced motion still applies.

## Left to the builder

- Exact drawings, palettes, timings and easing, within the outlined style and
  the timings above.
- How to structure the art registry and the effect components. Expected:
  - A map from emoji to `{ kind, name, Svg }`.
  - One `<Avatar icon size expression />` component used everywhere.
  - Effect overlays positioned by the existing `seatPos`.
- Whether the HandOver detection lives in `Table.tsx` or a hook.
- Testing:
  - The hand evaluator and the bad-beat rule have pure functions that need unit
    tests. The frontend has no test runner today, so adding `vitest` as a dev
    dependency is acceptable for this.
  - Visual checks are done by running the app locally (`./scripts/deploy-local.sh`)
    and screenshotting a table with bots, as was done for the show-cards feature.

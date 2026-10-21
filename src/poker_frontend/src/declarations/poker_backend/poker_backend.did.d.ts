import type { Principal } from '@icp-sdk/core/principal';
import type { ActorMethod } from '@icp-sdk/core/agent';
import type { IDL } from '@icp-sdk/core/candid';

/**
 * [Account](https://github.com/dfinity/ICRC-1/blob/main/standards/ICRC-3/README.md#value)
 * representation of ledgers supporting the ICRC-1 standard.
 */
export interface Account {
  'owner' : Principal,
  'subaccount' : [] | [Uint8Array | number[]],
}
export type Action = { 'Call' : null } |
  { 'Fold' : null } |
  {
    /**
     * Bet or raise so that the player's total bet on this street is the given amount.
     */
    'Raise' : bigint
  } |
  { 'AllIn' : null } |
  { 'Check' : null };
/**
 * Settings an admin can change from the admin page.
 */
export interface AdminConfig {
  /**
   * Moderators with their usernames (empty if they have no profile).
   */
  'moderators' : Array<[Principal, string]>,
  'banned_words' : Array<string>,
  'signups_per_hour' : number,
  'signup_pow_bits' : number,
}
export interface AuditEntry {
  'action' : string,
  'actor' : Principal,
  'time' : bigint,
}
/**
 * Where a player stands with auto-sit at a table.
 */
export type AutoSitStatus = {
    'Queued' : { 'place' : number, 'buy_in' : bigint, 'waiting' : number }
  } |
  {
    /**
     * Seated automatically at this table (reported once).
     */
    'Seated' : { 'table' : bigint }
  } |
  {
    /**
     * Taken out of the line because seating failed (reported once).
     */
    'Dropped' : { 'reason' : string }
  } |
  { 'NotQueued' : null };
/**
 * The viewer's own place in a table's auto-sit line.
 */
export interface AutoSitView { 'place' : number, 'buy_in' : bigint }
export interface Ban {
  /**
   * 0 = permanent.
   */
  'until' : bigint,
  'reason' : string,
}
/**
 * A row of a board as shown.
 */
export interface BoardRow {
  'username' : string,
  /**
   * Hands played in the period.
   */
  'hands' : number,
  /**
   * Points, or net ICP won (e8s).
   */
  'value' : bigint,
  'player' : Principal,
  /**
   * Position on the live list (1-based), the paid place for past winners, or
   * 0 for a player outside the list.
   */
  'rank' : number,
  /**
   * The prize if the period ended now; for past winners, what was paid.
   */
  'prize' : bigint,
  /**
   * Can be paid: in the period, with a value above zero, and (winnings
   * boards) enough hands.
   */
  'qualified' : boolean,
  'avatar' : string,
}
/**
 * One leaderboard as shown.
 */
export interface BoardView {
  /**
   * The caller's row (rank 0 when outside the list).
   */
  'me' : [] | [BoardRow],
  /**
   * When the period ends (ns).
   */
  'ends_at' : bigint,
  /**
   * The board's prize pool (e8s).
   */
  'pool' : bigint,
  /**
   * The top 10.
   */
  'rows' : Array<BoardRow>,
  /**
   * The period: "2026-W43" or "2026-11".
   */
  'label' : string,
  /**
   * The paid winners of the last closed period.
   */
  'last_winners' : Array<BoardRow>,
  /**
   * "weekly_points", "weekly_winnings", "monthly_points" or "monthly_winnings".
   */
  'board' : string,
}
export type Channel = { 'Lobby' : null } |
  { 'Table' : bigint };
export interface ChatMessage {
  'id' : bigint,
  'username' : string,
  'from' : [] | [Principal],
  'kind' : MessageKind,
  'text' : string,
  'time' : bigint,
}
/**
 * What a table plays for: real ICP (1 chip = 1 e8s) or free play chips.
 */
export type Currency = { 'Icp' : null } |
  { 'Chips' : null };
export interface DepositInfo {
  'fee' : bigint,
  /**
   * Legacy account identifier for exchanges / older wallets.
   */
  'account_id' : string,
  'ledger' : Principal,
  /**
   * ICRC-1 account to send ICP to (this canister + your deposit subaccount).
   */
  'account' : Account,
}
export interface EarnedMedal { 'id' : string, 'earned_at' : bigint }
export interface FriendInfo {
  'id' : Principal,
  'username' : string,
  'tables' : BigUint64Array | bigint[],
  'avatar' : string,
  'online' : boolean,
}
/**
 * One friend in the referrer's list.
 */
export interface FriendRow {
  /**
   * Earned the referrer something in the last 7 days.
   */
  'active' : boolean,
  'username' : string,
  /**
   * When the 12-month earning window for this friend ends (ns).
   */
  'ends_at' : bigint,
  /**
   * Cuts earned from this friend (e8s). Earned, not paid: a cut forfeited
   * because the referrer was suspended at payout still counts here.
   */
  'earned' : bigint,
  'joined_at' : bigint,
}
export interface HandPlayer {
  'won' : bigint,
  'username' : string,
  'player' : Principal,
  /**
   * Hole cards. Stored for everyone; views only reveal your own and shown hands.
   */
  'cards' : Uint8Array | number[],
  'seat' : number,
  'committed' : bigint,
  'shown' : boolean,
}
export interface HandRecord {
  'id' : bigint,
  'log' : Array<LogEntry>,
  'pot' : bigint,
  'small_blind' : bigint,
  'hand_no' : bigint,
  'rake' : bigint,
  'time' : bigint,
  'table_id' : bigint,
  'players' : Array<HandPlayer>,
  'currency' : Currency,
  'big_blind' : bigint,
  'board' : Uint8Array | number[],
  'button' : number,
  'table_name' : string,
}
export interface HandResult {
  'pot' : bigint,
  'hand_no' : bigint,
  'rake' : bigint,
  'shown' : Array<ShownHand>,
  'players' : Array<PlayerResult>,
  'board' : Uint8Array | number[],
  'winners' : Array<Winner>,
  /**
   * Taken from the pot for the bad-beat jackpot.
   */
  'jackpot_drop' : bigint,
}
export interface HandSummary {
  'id' : bigint,
  /**
   * What the viewer won minus what they put in.
   */
  'net' : bigint,
  'pot' : bigint,
  'my_cards' : Uint8Array | number[],
  'time' : bigint,
  'currency' : Currency,
  'board' : Uint8Array | number[],
  'table_name' : string,
}
export interface HouseStats {
  /**
   * What the canister's main ledger account must hold to cover everything.
   */
  'liabilities_total' : bigint,
  'chips_on_tables' : bigint,
  /**
   * ICP bad-beat jackpot.
   */
  'jackpot' : bigint,
  'wallet_balances' : bigint,
  /**
   * Fish and leaderboard prize pools, owed to players.
   */
  'rewards_pools' : bigint,
  /**
   * Referral cuts owed to referrers, paid at the weekly close.
   */
  'referral_owed' : bigint,
  'users' : bigint,
  'treasury' : bigint,
}
export interface HttpRequest {
  'url' : string,
  'method' : string,
  'body' : Uint8Array | number[],
  'headers' : Array<[string, string]>,
}
export interface HttpResponse {
  'body' : Uint8Array | number[],
  'headers' : Array<[string, string]>,
  'status_code' : number,
}
export interface InitArgs {
  'ledger' : [] | [Principal],
  'ledger_fee' : [] | [bigint],
}
export type ItemKind = { 'CardBack' : null } |
  { 'Throwable' : null } |
  { 'Avatar' : null } |
  { 'Gift' : null };
export interface JackpotHit {
  'winner_cards' : Uint8Array | number[],
  'total' : bigint,
  'loser_hand' : string,
  'loser_name' : string,
  'hand_no' : bigint,
  'time' : bigint,
  'table_id' : bigint,
  'winner' : Principal,
  'loser' : Principal,
  'currency' : Currency,
  'board' : Uint8Array | number[],
  'loser_cards' : Uint8Array | number[],
  'table_name' : string,
  'payouts' : Array<JackpotPayout>,
  'winner_hand' : string,
  'winner_name' : string,
}
export interface JackpotInfo {
  'icp' : bigint,
  'min_players' : number,
  'chips' : bigint,
  'qualifying_hand' : string,
  'winner_bps' : bigint,
  'loser_bps' : bigint,
  'payout_bps' : bigint,
  'drop_bps' : number,
  'recent' : Array<JackpotHit>,
}
export interface JackpotPayout {
  'username' : string,
  'player' : Principal,
  /**
   * "loser", "winner" or "table".
   */
  'share' : string,
  'amount' : bigint,
}
export interface LeaderboardEntry {
  'id' : Principal,
  'username' : string,
  'level' : number,
  'stats' : Stats,
  'avatar' : string,
}
export interface LedgerOp {
  'id' : bigint,
  'to' : Account,
  'fee' : bigint,
  'status' : OpStatus,
  'updated_at' : bigint,
  'from' : [] | [Account],
  'kind' : OpKind,
  /**
   * The player (or the controller for treasury withdrawals).
   */
  'user' : Principal,
  'from_subaccount' : [] | [Uint8Array | number[]],
  'created_at_time' : bigint,
  /**
   * Amount moved on the ledger, excluding the fee.
   */
  'amount' : bigint,
}
/**
 * One line of a hand's action log, used for hand histories and replays.
 */
export interface LogEntry {
  'seat' : [] | [number],
  'text' : string,
  'phase' : Phase,
  /**
   * The player's total bet on this street after the action, or the amount won/returned.
   */
  'amount' : bigint,
}
export interface MedalDef {
  'id' : string,
  'icon' : string,
  'name' : string,
  'description' : string,
}
export type MessageKind = { 'System' : null } |
  { 'Gift' : null } |
  { 'Text' : null } |
  { 'Throw' : null };
export interface Metrics {
  'running_tournaments' : bigint,
  'last_solvency_surplus_e8s' : [] | [bigint],
  'treasury_e8s' : bigint,
  'jackpot_chips' : bigint,
  'bans' : bigint,
  'last_solvency_at' : [] | [bigint],
  'last_solvency_ok' : [] | [boolean],
  'idle_stand_ups' : bigint,
  'reports_filed' : bigint,
  'open_reports' : bigint,
  'time' : bigint,
  'seated_players' : bigint,
  'ledger_ops_unknown' : bigint,
  'paused_deposits' : boolean,
  'paused_joins' : boolean,
  'tables' : bigint,
  'cycles' : bigint,
  'liabilities_e8s' : bigint,
  'paused_withdrawals' : boolean,
  'users' : bigint,
  'ledger_ops_failed' : bigint,
  /**
   * Players who made an update call in the last 5 minutes.
   */
  'active_players_5m' : bigint,
  'unresolved_ledger_ops' : bigint,
  'hands_dealt' : bigint,
  'wallet_total_e8s' : bigint,
  'paused_dealing' : boolean,
  'stable_bytes' : bigint,
  /**
   * Tables with at least two players seated.
   */
  'active_tables' : bigint,
  'jackpot_icp_e8s' : bigint,
  'rate_limited_calls' : bigint,
  'heap_bytes' : bigint,
}
export type ModAction = {
    /**
     * Suspension for this many hours (0 = permanent).
     */
    'Ban' : number
  } |
  { 'Warn' : null } |
  { 'Dismiss' : null } |
  {
    /**
     * Chat mute for this many hours.
     */
    'MuteChat' : number
  };
/**
 * What moderators see when they look a player up.
 */
export interface ModPlayerView {
  'id' : Principal,
  'ban' : [] | [Ban],
  'status' : string,
  'chat_muted_until' : bigint,
  'username' : string,
  'balance' : bigint,
  'chips' : bigint,
  'is_moderator' : boolean,
  'created_at' : bigint,
  'level' : number,
  /**
   * Tables they're seated at.
   */
  'tables' : BigUint64Array | bigint[],
  /**
   * Reports against them, newest first (open and resolved, up to 20).
   */
  'reports' : Array<Report>,
  'last_seen' : bigint,
  'net_winnings' : bigint,
  'hands_played' : bigint,
}
/**
 * Color label for a player note (a quick read on a player's style).
 */
export type NoteColor = { 'Red' : null } |
  { 'Yellow' : null } |
  { 'Blue' : null } |
  { 'Green' : null } |
  { 'Orange' : null } |
  { 'Purple' : null };
export interface NoteView {
  'updated_at' : bigint,
  'username' : string,
  'player' : Principal,
  'color' : [] | [NoteColor],
  'text' : string,
  'avatar' : string,
}
export type OpKind = {
    /**
     * Player withdrawal from their poker balance.
     */
    'Withdraw' : null
  } |
  {
    /**
     * Sweep of a player's deposit subaccount into the main account.
     */
    'Sweep' : null
  } |
  {
    /**
     * ICRC-2 transfer_from the player's wallet into the canister.
     */
    'Deposit' : null
  } |
  {
    /**
     * Controller withdrawal from the house treasury.
     */
    'Treasury' : null
  };
export type OpStatus = { 'Failed' : { 'reason' : string } } |
  { 'Unknown' : { 'attempts' : number, 'reason' : string } } |
  { 'Completed' : { 'block' : bigint } } |
  { 'Pending' : null };
export interface PauseFlags {
  /**
   * No new seats, rooms or tournament entries.
   */
  'joins' : boolean,
  'withdrawals' : boolean,
  'deposits' : boolean,
  /**
   * No new hands are dealt (hands in progress finish).
   */
  'dealing' : boolean,
  'reason' : [] | [string],
}
export type Phase = { 'Flop' : null } |
  { 'Turn' : null } |
  { 'River' : null } |
  { 'HandOver' : null } |
  { 'Waiting' : null } |
  { 'PreFlop' : null };
/**
 * A private note one player keeps about another. Only its author can see it.
 */
export interface PlayerNote {
  'updated_at' : bigint,
  'color' : [] | [NoteColor],
  'text' : string,
}
export interface PlayerResult {
  'won' : bigint,
  'player' : Principal,
  'committed' : bigint,
}
export interface Profile {
  'id' : Principal,
  'xp' : bigint,
  'ban' : [] | [Ban],
  'status' : string,
  'friend_requests' : Array<Principal>,
  'chat_muted_until' : bigint,
  'username' : string,
  'balance' : bigint,
  'blocked' : Array<Principal>,
  'chips' : bigint,
  'inventory' : Uint32Array | number[],
  /**
   * When the next daily chip bonus can be claimed (ns).
   */
  'next_daily_at' : bigint,
  'created_at' : bigint,
  'level' : number,
  /**
   * Size of the next daily bonus.
   */
  'next_daily_amount' : bigint,
  'stats' : Stats,
  'settings' : Settings,
  'card_back' : number,
  'friends' : Array<Principal>,
  'daily_streak' : number,
  'avatar' : number,
  'medals' : Array<EarnedMedal>,
}
export interface PublicProfile {
  'id' : Principal,
  'status' : string,
  'username' : string,
  'level' : number,
  'tables' : BigUint64Array | bigint[],
  'stats' : Stats,
  'avatar' : string,
  'medals' : Array<string>,
  'online' : boolean,
}
/**
 * What the referrals page shows. The zero view (`Default`) is for an
 * anonymous caller or a player with no profile.
 */
export interface ReferralView {
  /**
   * Friends who earned something in the last 7 days. Counted over the
   * newest `ACTIVE_SCAN_MAX` (2,000) friends only, so with more friends than
   * that it can undercount.
   */
  'active_7d' : number,
  /**
   * Username of the caller's referrer.
   */
  'my_referrer' : [] | [string],
  /**
   * Within the first 24 hours and no referrer yet.
   */
  'can_enter_code' : boolean,
  /**
   * The caller's username, which is their referral code.
   */
  'code' : string,
  /**
   * At most `FRIENDS_PAGE` friends, newest first, from `offset`.
   */
  'rows' : Array<FriendRow>,
  /**
   * Cuts waiting for the next Monday payout (e8s).
   */
  'owed_this_week' : bigint,
  'paid_total' : bigint,
  'friends' : number,
  /**
   * The next Monday 00:00 UTC (ns), when `owed_this_week` is paid.
   */
  'pays_at' : bigint,
}
export interface Report {
  'id' : bigint,
  'status' : ReportStatus,
  'created_at' : bigint,
  'target' : Principal,
  /**
   * The target's recent messages in that channel, captured when reported.
   */
  'evidence' : Array<ChatMessage>,
  'reporter_name' : string,
  'details' : string,
  'channel' : [] | [Channel],
  'reporter' : Principal,
  'target_name' : string,
  'reason' : ReportReason,
}
/**
 * A page of the open-report queue, oldest first.
 */
export interface ReportPage {
  /**
   * All open reports, not just this page.
   */
  'open_total' : bigint,
  'reports' : Array<Report>,
}
export type ReportReason = { 'Spam' : null } |
  { 'Cheating' : null } |
  { 'Collusion' : null } |
  { 'Other' : null } |
  { 'OffensiveName' : null } |
  { 'AbusiveChat' : null };
export type ReportStatus = { 'Open' : null } |
  {
    'Resolved' : {
      'at' : bigint,
      'by' : Principal,
      'action' : ModAction,
      'note' : string,
    }
  };
export type Result = { 'Ok' : null } |
  { 'Err' : string };
export type Result_1 = { 'Ok' : number } |
  { 'Err' : string };
export type Result_10 = { 'Ok' : Array<LedgerOp> } |
  { 'Err' : string };
export type Result_11 = { 'Ok' : AutoSitStatus } |
  { 'Err' : string };
export type Result_12 = { 'Ok' : ModPlayerView } |
  { 'Err' : string };
export type Result_13 = { 'Ok' : ReportPage } |
  { 'Err' : string };
export type Result_14 = { 'Ok' : Array<Report> } |
  { 'Err' : string };
export type Result_15 = { 'Ok' : Profile } |
  { 'Err' : string };
export type Result_16 = { 'Ok' : SpinRecord } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : Array<AuditEntry> } |
  { 'Err' : string };
export type Result_3 = { 'Ok' : [bigint, bigint] } |
  { 'Err' : string };
export type Result_4 = { 'Ok' : SolvencyReport } |
  { 'Err' : string };
export type Result_5 = { 'Ok' : bigint } |
  { 'Err' : string };
export type Result_6 = { 'Ok' : AdminConfig } |
  { 'Err' : string };
export type Result_7 = { 'Ok' : Metrics } |
  { 'Err' : string };
export type Result_8 = { 'Ok' : HouseStats } |
  { 'Err' : string };
export type Result_9 = { 'Ok' : LedgerOp } |
  { 'Err' : string };
/**
 * What the rewards screen shows for a player.
 */
export interface RewardsView {
  'tier_name' : string,
  'meter' : bigint,
  'tiers' : Array<TierInfo>,
  'step' : bigint,
  'tier' : number,
  'next_tier_at' : [] | [bigint],
  /**
   * After decay.
   */
  'tier_points' : bigint,
  'history' : Array<SpinRecord>,
  'lifetime_points' : bigint,
  'days_since_earned' : bigint,
  /**
   * Days of grace left before decay starts (0 = decaying now); none without points.
   */
  'decay_starts_in_days' : [] | [bigint],
  'fish_pool' : bigint,
  'spins_banked' : number,
}
/**
 * What the signed-in caller may administer (drives the admin page).
 */
export interface Roles { 'admin' : boolean, 'moderator' : boolean }
export interface RoomArgs {
  'action_timeout_secs' : bigint,
  'password' : [] | [string],
  'name' : string,
  'max_seats' : number,
  'currency' : Currency,
  'big_blind' : bigint,
}
export interface SeatView {
  'bet' : bigint,
  'in_hand' : boolean,
  /**
   * Seconds left in this player's time bank.
   */
  'time_bank_secs' : number,
  'username' : string,
  'player' : Principal,
  /**
   * Who sent the current gift (their username).
   */
  'gift_from' : [] | [string],
  'all_in' : boolean,
  'cards' : Uint8Array | number[],
  'gift' : [] | [string],
  /**
   * The viewer's own note about this player, if any.
   */
  'note' : [] | [PlayerNote],
  'has_cards' : boolean,
  'seat' : number,
  'last_action' : [] | [string],
  /**
   * When the current gift was sent; a repeat of the same gift gets a new time.
   */
  'gift_sent_at' : [] | [bigint],
  'level' : number,
  'stack' : bigint,
  'card_back' : string,
  'sitting_out' : boolean,
  'folded' : boolean,
  'avatar' : string,
}
/**
 * Public status: what's paused and the latest solvency check.
 */
export interface ServiceStatus {
  'solvency' : [] | [SolvencyReport],
  'pause' : PauseFlags,
}
export interface Settings {
  /**
   * Refill your stack to the table maximum after each hand when it drops below half.
   */
  'auto_top_up' : boolean,
  /**
   * Don't show losing hands at showdown.
   */
  'auto_muck' : boolean,
  'four_color_deck' : boolean,
  'sound' : boolean,
  'show_throwables' : boolean,
}
export interface ShownHand {
  'player' : Principal,
  'cards' : Uint8Array | number[],
  'hand' : string,
  'seat' : number,
}
export interface SignupChallenge {
  /**
   * Required leading zero bits of sha256("bp-signup" ‖ principal ‖ nonce as u64 LE).
   */
  'difficulty' : number,
}
export interface SolvencyReport {
  'ok' : boolean,
  'liabilities' : bigint,
  'ledger_balance' : bigint,
  /**
   * Ledger balance minus liabilities (negative means insolvent).
   */
  'surplus' : bigint,
  'unresolved_ops' : bigint,
  'checked_at' : bigint,
}
/**
 * One spin of the wheel, as shown in the player's history.
 */
export interface SpinRecord {
  'paid' : bigint,
  'tier' : string,
  'time' : bigint,
  'multiplier_x10' : bigint,
  'stake' : bigint,
  /**
   * Index into the wheel's segments (for the animation).
   */
  'segment' : number,
  'capped' : boolean,
  /**
   * Before the pool cap.
   */
  'prize' : bigint,
}
export interface Standing {
  'table' : [] | [bigint],
  'username' : string,
  'player' : Principal,
  'chips' : bigint,
  'place' : [] | [number],
  'prize' : bigint,
  'avatar' : string,
}
export interface Stats {
  /**
   * Largest ICP pot won, in e8s.
   */
  'biggest_pot_won' : bigint,
  'hands_won' : bigint,
  /**
   * Net play chips won or lost at chip tables.
   */
  'chip_net' : bigint,
  /**
   * Net ICP won or lost at ICP tables, in e8s.
   */
  'net_winnings' : bigint,
  'hands_played' : bigint,
}
export interface StoreItem {
  'id' : number,
  /**
   * Emoji or image URL rendered by the frontend.
   */
  'icon' : string,
  'kind' : ItemKind,
  'name' : string,
  'description' : string,
  'available' : boolean,
  /**
   * Price in play chips. Zero means it can't be bought with chips.
   */
  'chip_price' : bigint,
  /**
   * Price in e8s. Zero means free.
   */
  'price' : bigint,
}
export interface TableConfig {
  'small_blind' : bigint,
  'action_timeout_secs' : bigint,
  'min_buy_in' : bigint,
  'name' : string,
  /**
   * Bad-beat jackpot drop in basis points of the pot after rake (flop only). 0 = no jackpot.
   */
  'jackpot_bps' : number,
  'jackpot_cap' : bigint,
  /**
   * Rake in basis points of the pot (250 = 2.5%). Only taken when a flop is dealt.
   */
  'rake_bps' : number,
  'rake_cap' : bigint,
  'max_seats' : number,
  'currency' : Currency,
  'big_blind' : bigint,
  'max_buy_in' : bigint,
}
export interface TableSummary {
  'id' : bigint,
  'small_blind' : bigint,
  'min_buy_in' : bigint,
  /**
   * The game this table belongs to: the original table's id (its own id for originals and rooms).
   */
  'game' : bigint,
  'name' : string,
  /**
   * No more copies of this game can open right now (always true for rooms).
   */
  'game_at_cap' : boolean,
  'locked' : boolean,
  'max_seats' : number,
  'avg_pot' : bigint,
  'players' : number,
  'currency' : Currency,
  /**
   * 1 for the original, 2.. for copies opened when the game filled up.
   */
  'number' : number,
  'big_blind' : bigint,
  'phase' : Phase,
  /**
   * Username of the player who created this room, for player-created tables.
   */
  'room_owner' : [] | [string],
  'max_buy_in' : bigint,
}
export interface TableView {
  'id' : bigint,
  'now' : bigint,
  'pot' : bigint,
  'my_seat' : [] | [number],
  'to_call' : bigint,
  /**
   * The viewer's place in this table's auto-sit line, if they're in it.
   */
  'my_auto_sit' : [] | [AutoSitView],
  /**
   * The game this table belongs to: the original table's id (its own id for originals and rooms).
   */
  'game' : bigint,
  'hand_no' : bigint,
  /**
   * Bad-beat jackpot for this table's currency (0 at tournament tables).
   */
  'jackpot' : bigint,
  /**
   * A jackpot hit in the hand on screen, for the celebration.
   */
  'jackpot_hit' : [] | [JackpotHit],
  /**
   * Joining needs a password.
   */
  'locked' : boolean,
  'last_result' : [] | [HandResult],
  'tournament' : [] | [TournamentBadge],
  /**
   * The player to act is on their time bank (`action_deadline` includes it).
   */
  'time_bank_active' : boolean,
  'min_raise_to' : bigint,
  'seats' : Array<[] | [SeatView]>,
  'action_deadline' : bigint,
  'to_act' : [] | [number],
  'current_bet' : bigint,
  /**
   * Items thrown in the last few seconds, for clients to animate.
   */
  'throws' : Array<ThrowEvent>,
  /**
   * 1 for the original, 2.. for copies opened when the game filled up.
   */
  'number' : number,
  /**
   * A private room the viewer hasn't unlocked: game details are hidden.
   */
  'restricted' : boolean,
  'board' : Uint8Array | number[],
  'phase' : Phase,
  'config' : TableConfig,
  /**
   * The player who created this room, if it is a player-created table.
   */
  'room_owner' : [] | [Principal],
  'button' : number,
  /**
   * How many watchers are waiting to be seated here automatically.
   */
  'auto_sit_waiting' : number,
  /**
   * Smallest buy-in the viewer may sit down with (they left recently with
   * this many chips); 0 means the table's normal minimum.
   */
  'my_min_buy_in' : bigint,
  'max_raise_to' : bigint,
}
export interface ThrowEvent {
  'id' : bigint,
  'from_seat' : number,
  'to_seat' : number,
  'icon' : string,
  'time' : bigint,
}
export interface TierInfo {
  'rate_bps' : bigint,
  'from' : bigint,
  'name' : string,
}
export interface TournamentBadge {
  'id' : bigint,
  /**
   * The viewer may rebuy right now.
   */
  'can_rebuy' : boolean,
  'small_blind' : bigint,
  'rebuy_cost' : bigint,
  /**
   * The viewer may take the add-on right now.
   */
  'can_addon' : boolean,
  'addon_chips' : bigint,
  'name' : string,
  'level' : number,
  'addon_cost' : bigint,
  'finished' : boolean,
  'currency' : Currency,
  'big_blind' : bigint,
  'entrants' : number,
  'players_left' : number,
  'rebuys_left' : number,
  'my_place' : [] | [number],
  'my_prize' : bigint,
  /**
   * Busted viewer's deadline to rebuy (0 if not waiting).
   */
  'rebuy_deadline' : bigint,
  'next_level_at' : bigint,
}
export interface TournamentDetail {
  'small_blind' : bigint,
  'standings' : Array<Standing>,
  'level' : number,
  'tables' : BigUint64Array | bigint[],
  'summary' : TournamentSummary,
  'big_blind' : bigint,
  'next_level_at' : bigint,
  'payouts' : BigUint64Array | bigint[],
}
export type TournamentKind = {
    /**
     * Starts at a set time if enough players registered.
     */
    'Scheduled' : null
  } |
  {
    /**
     * Starts as soon as it fills up.
     */
    'SitAndGo' : null
  };
export type TournamentStatus = { 'Finished' : null } |
  { 'Running' : null } |
  { 'Cancelled' : null } |
  { 'Registering' : null };
export interface TournamentSummary {
  'id' : bigint,
  'fee' : bigint,
  'status' : TournamentStatus,
  'max_rebuys' : number,
  /**
   * The table the viewer is playing at, if any.
   */
  'my_table' : [] | [bigint],
  'min_players' : number,
  'rebuy_levels' : number,
  'start_at' : bigint,
  'addon_chips' : bigint,
  'kind' : TournamentKind,
  'name' : string,
  'level_secs' : bigint,
  'max_players' : number,
  'level' : number,
  /**
   * Total buy-ins: registrations plus rebuys.
   */
  'entries' : number,
  'addon_cost' : bigint,
  'currency' : Currency,
  'prize_pool' : bigint,
  'players_left' : number,
  'registered_me' : boolean,
  'starting_stack' : bigint,
  'buy_in' : bigint,
  'late_reg_levels' : number,
  'registered' : number,
  /**
   * Registration is open right now (before the start, or during late registration).
   */
  'can_register' : boolean,
}
export interface TournamentTemplate {
  /**
   * Goes to the house (ICP) or leaves circulation (chips).
   */
  'fee' : bigint,
  'max_rebuys' : number,
  'min_players' : number,
  /**
   * Rebuys are allowed while the level (0-based) is below this. 0 = freezeout.
   */
  'rebuy_levels' : number,
  'addon_chips' : bigint,
  'kind' : TournamentKind,
  'name' : string,
  'level_secs' : bigint,
  /**
   * Scheduled events: time between runs.
   */
  'interval_secs' : bigint,
  'max_players' : number,
  /**
   * Minimum prize pool, only for play-chip events (the house mints chips).
   */
  'guarantee' : bigint,
  /**
   * One-time add-on at the end of the rebuy period: price (into the pool, no fee)
   * and the chips it buys. 0 = no add-on.
   */
  'addon_cost' : bigint,
  'currency' : Currency,
  'starting_stack' : bigint,
  /**
   * Goes into the prize pool.
   */
  'buy_in' : bigint,
  'table_size' : number,
  /**
   * Registration stays open while the level (0-based) is below this. 0 = no late registration.
   */
  'late_reg_levels' : number,
}
/**
 * What the caller sees of their welcome bonus.
 */
export interface WelcomeView {
  /**
   * Granted, not fully released, not expired.
   */
  'active' : boolean,
  'expired' : boolean,
  /**
   * Still locked: zero once expired or fully released.
   */
  'locked' : bigint,
  'released' : bigint,
  /**
   * No bonus yet and still entitled to one on the first deposit.
   */
  'eligible' : boolean,
  'amount' : bigint,
  'expires_at' : bigint,
}
export interface Winner {
  'player' : Principal,
  'hand' : string,
  'seat' : number,
  'amount' : bigint,
}
export interface _SERVICE {
  'act' : ActorMethod<[bigint, Action], Result>,
  /**
   * Takes the one-time add-on at the end of a tournament's rebuy period.
   */
  'addon_tournament' : ActorMethod<[bigint], Result>,
  'admin_add_item' : ActorMethod<[StoreItem], Result_1>,
  /**
   * Adds a recurring tournament (a sit-and-go or a scheduled event).
   */
  'admin_add_tournament' : ActorMethod<[TournamentTemplate], Result_1>,
  /**
   * Privileged actions (admins and moderators), newest first.
   */
  'admin_audit_log' : ActorMethod<[number], Result_2>,
  /**
   * Recomputes the sum of ICP balances by scanning every user, to verify the running total.
   */
  'admin_audit_wallet_total' : ActorMethod<[], Result_3>,
  /**
   * Compares the canister's ICP balance with everything it owes. A shortfall
   * pauses withdrawals automatically. Also runs every 10 minutes on its own.
   */
  'admin_check_solvency' : ActorMethod<[], Result_4>,
  'admin_create_table' : ActorMethod<[TableConfig], Result_5>,
  'admin_get_config' : ActorMethod<[], Result_6>,
  'admin_get_metrics' : ActorMethod<[], Result_7>,
  'admin_house_stats' : ActorMethod<[], Result_8>,
  /**
   * Settles an unknown op after checking the ledger by hand (beyond the 24h retry window).
   */
  'admin_resolve_op' : ActorMethod<[bigint, boolean, [] | [bigint]], Result_9>,
  /**
   * Re-sends an unknown op right away (it's deduplicated by the ledger).
   */
  'admin_retry_op' : ActorMethod<[bigint], Result_9>,
  /**
   * Moves ICP from the house treasury into the bad-beat jackpot.
   */
  'admin_seed_jackpot' : ActorMethod<[bigint], Result>,
  /**
   * Replaces the chat/username word filter (whole words, case-insensitive).
   */
  'admin_set_banned_words' : ActorMethod<[Array<string>], Result>,
  'admin_set_item_available' : ActorMethod<[number, boolean], Result>,
  'admin_set_moderator' : ActorMethod<[Principal, boolean], Result>,
  /**
   * Emergency switches: pause deposits, withdrawals, dealing and/or new joins.
   */
  'admin_set_pause' : ActorMethod<[PauseFlags], Result>,
  /**
   * Hourly sign-up target: past it, sign-ups need more proof-of-work (0 closes sign-ups).
   */
  'admin_set_signup_limit' : ActorMethod<[number], Result>,
  /**
   * Base sign-up proof-of-work, in leading zero bits (default 12, max 22).
   */
  'admin_set_signup_pow' : ActorMethod<[number], Result>,
  /**
   * Ledger operations whose outcome is unknown and need attention.
   */
  'admin_unresolved_ops' : ActorMethod<[], Result_10>,
  /**
   * Withdraws house revenue (rake and store sales) from the treasury.
   */
  'admin_withdraw_treasury' : ActorMethod<[Account, bigint], Result_5>,
  /**
   * Waits for a seat at a table: you're seated automatically, with this buy-in,
   * when one opens. `None` leaves the line. Keep calling `auto_sit_ping` while
   * you watch, or you're dropped from the line.
   */
  'auto_sit' : ActorMethod<[bigint, [] | [bigint]], Result_11>,
  /**
   * Keeps you in a table's auto-sit line and reports whether you were seated or dropped.
   */
  'auto_sit_ping' : ActorMethod<[bigint], Result_11>,
  /**
   * Blocks (or unblocks) a player: hides their chat and stops DMs, friend
   * requests, gifts and throws between you.
   */
  'block_player' : ActorMethod<[Principal, boolean], Result>,
  'buy_item' : ActorMethod<[number, Currency], Result>,
  /**
   * Claims the daily free play-chip bonus (bigger on consecutive days).
   */
  'claim_daily_chips' : ActorMethod<[], Result_5>,
  /**
   * Deposit via plain transfer: send ICP to your deposit account (see
   * `get_deposit_info`) from any wallet or exchange, then call this. Returns the amount credited.
   */
  'claim_deposit' : ActorMethod<[], Result_5>,
  'close_room' : ActorMethod<[bigint], Result>,
  /**
   * Creates your own table. Hosts of ICP tables earn 20% of the rake it generates.
   */
  'create_room' : ActorMethod<[RoomArgs], Result_5>,
  /**
   * Deposit via ICRC-2: the caller first approves this canister for
   * `amount + fee` on the ICP ledger, then calls this. Returns the new balance.
   */
  'deposit' : ActorMethod<[bigint], Result_5>,
  'equip' : ActorMethod<[number], Result>,
  'find_user' : ActorMethod<[string], [] | [PublicProfile]>,
  'get_chat' : ActorMethod<[Channel, bigint], Array<ChatMessage>>,
  'get_deposit_info' : ActorMethod<[], DepositInfo>,
  'get_dms' : ActorMethod<[Principal, bigint], Array<ChatMessage>>,
  'get_friends' : ActorMethod<[], Array<FriendInfo>>,
  /**
   * Full replay of a hand you played in.
   */
  'get_hand' : ActorMethod<[bigint], [] | [HandRecord]>,
  /**
   * Your most recent hands, newest first.
   */
  'get_hand_history' : ActorMethod<[number], Array<HandSummary>>,
  /**
   * Current bad-beat jackpots, the rules and recent hits.
   */
  'get_jackpots' : ActorMethod<[], JackpotInfo>,
  'get_leaderboard' : ActorMethod<[], Array<LeaderboardEntry>>,
  /**
   * The four weekly and monthly leaderboards, with the caller's own row.
   */
  'get_leaderboards' : ActorMethod<[], Array<BoardView>>,
  /**
   * Your recent deposits and withdrawals, newest first.
   */
  'get_my_ledger_ops' : ActorMethod<[], Array<LedgerOp>>,
  'get_my_profile' : ActorMethod<[], [] | [Profile]>,
  'get_my_roles' : ActorMethod<[], Roles>,
  /**
   * All your notes, most recently updated first.
   */
  'get_notes' : ActorMethod<[], Array<NoteView>>,
  'get_profile' : ActorMethod<[Principal], [] | [PublicProfile]>,
  /**
   * The caller's referral earnings and friends, `offset` friends in (newest first).
   */
  'get_referrals' : ActorMethod<[number], ReferralView>,
  /**
   * The caller's Fish Buffet standing, spins and history.
   */
  'get_rewards' : ActorMethod<[], RewardsView>,
  /**
   * How much sign-up work a new account needs right now (see `register`).
   */
  'get_signup_challenge' : ActorMethod<[], SignupChallenge>,
  'get_status' : ActorMethod<[], ServiceStatus>,
  'get_table' : ActorMethod<[bigint], [] | [TableView]>,
  /**
   * `password` is only needed for password-protected player rooms.
   * Views of several tables at once (for multi-tabling); unknown ids are skipped.
   */
  'get_tables' : ActorMethod<[BigUint64Array | bigint[]], Array<TableView>>,
  'get_tournament' : ActorMethod<[bigint], [] | [TournamentDetail]>,
  /**
   * The caller's welcome bonus: the offer, or its progress.
   */
  'get_welcome' : ActorMethod<[], WelcomeView>,
  /**
   * `GET /metrics` (Prometheus text, without the house's money figures) and
   * `GET /health` on the canister's raw domain. Responses are not certified, so
   * they're for monitoring only.
   */
  'http_request' : ActorMethod<[HttpRequest], HttpResponse>,
  /**
   * Joins a game: a free seat at its fullest table, or a new copy of it when every
   * table is full. Returns the id of the table you were seated at.
   */
  'join_game' : ActorMethod<[bigint, bigint], Result_5>,
  /**
   * `password` is only needed for password-protected player rooms.
   */
  'join_table' : ActorMethod<[bigint, number, bigint, [] | [string]], Result>,
  'leave_table' : ActorMethod<[bigint], Result_5>,
  'list_medals' : ActorMethod<[], Array<MedalDef>>,
  'list_store' : ActorMethod<[], Array<StoreItem>>,
  'list_tables' : ActorMethod<[], Array<TableSummary>>,
  'list_tournaments' : ActorMethod<[], Array<TournamentSummary>>,
  /**
   * Suspends a player for `hours` (0 = permanent). They keep access to withdrawals.
   */
  'mod_ban' : ActorMethod<[Principal, number, string], Result>,
  'mod_get_player' : ActorMethod<[Principal], Result_12>,
  /**
   * Mutes a player in every chat for `hours` (0 lifts the mute).
   */
  'mod_mute' : ActorMethod<[Principal, number], Result>,
  /**
   * The open-report queue, oldest first: up to `limit` (max 50) after report `after`.
   */
  'mod_open_reports' : ActorMethod<[[] | [bigint], number], Result_13>,
  /**
   * The latest reports, open or resolved.
   */
  'mod_recent_reports' : ActorMethod<[number], Result_14>,
  'mod_resolve_report' : ActorMethod<[bigint, ModAction, string], Result>,
  'mod_unban' : ActorMethod<[Principal], Result>,
  /**
   * Buys a fresh starting stack during a tournament's rebuy period (buy-in only, no fee).
   */
  'rebuy_tournament' : ActorMethod<[bigint], Result>,
  /**
   * Creates your profile. `pow` is a nonce solving `get_signup_challenge`.
   */
  'register' : ActorMethod<[string, [] | [bigint]], Result_15>,
  'register_tournament' : ActorMethod<[bigint], Result>,
  'remove_friend' : ActorMethod<[Principal], Result>,
  /**
   * Reports a player to the moderators. With a channel, their recent messages
   * there are attached as evidence. Returns the report id.
   */
  'report_player' : ActorMethod<
    [Principal, ReportReason, string, [] | [Channel]],
    Result_5
  >,
  'respond_friend_request' : ActorMethod<[Principal, boolean], Result>,
  'send_chat' : ActorMethod<[Channel, string], Result>,
  'send_dm' : ActorMethod<[Principal, string], Result>,
  'send_friend_request' : ActorMethod<[Principal], Result>,
  'send_gift' : ActorMethod<[bigint, Principal, number, Currency], Result>,
  /**
   * Saves a private note about a player. Empty text and no color deletes it.
   */
  'set_note' : ActorMethod<[Principal, string, [] | [NoteColor]], Result>,
  /**
   * Attaches a referrer (another player's username) within your first 24 hours.
   */
  'set_referrer' : ActorMethod<[string], Result>,
  'set_status' : ActorMethod<[string], Result>,
  /**
   * Shows your cards after winning a hand uncontested, until the next hand is dealt.
   */
  'show_cards' : ActorMethod<[bigint], Result>,
  'sit_out' : ActorMethod<[bigint, boolean], Result>,
  /**
   * Spins the oldest banked Fish Buffet spin for ICP cashback.
   */
  'spin_wheel' : ActorMethod<[], Result_16>,
  'throw_item' : ActorMethod<[bigint, Principal, number, Currency], Result>,
  'top_up' : ActorMethod<[bigint, bigint], Result>,
  /**
   * Enters a private room's password so you can watch it and chat there.
   */
  'unlock_room' : ActorMethod<[bigint, string], Result>,
  'unregister_tournament' : ActorMethod<[bigint], Result>,
  'update_settings' : ActorMethod<[Settings], Result>,
  'whoami' : ActorMethod<[], Principal>,
  /**
   * Sends `amount` e8s to `to`. The ledger fee is charged on top. Returns the block index.
   */
  'withdraw' : ActorMethod<[Account, bigint], Result_5>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
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
export interface AuditEntry {
  'action' : string,
  'actor' : Principal,
  'time' : bigint,
}
export interface Ban {
  /**
   * 0 = permanent.
   */
  'until' : bigint,
  'reason' : string,
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
export type Result_11 = { 'Ok' : Array<Report> } |
  { 'Err' : string };
export type Result_12 = { 'Ok' : Profile } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : Array<AuditEntry> } |
  { 'Err' : string };
export type Result_3 = { 'Ok' : [bigint, bigint] } |
  { 'Err' : string };
export type Result_4 = { 'Ok' : SolvencyReport } |
  { 'Err' : string };
export type Result_5 = { 'Ok' : bigint } |
  { 'Err' : string };
export type Result_6 = { 'Ok' : Metrics } |
  { 'Err' : string };
export type Result_7 = { 'Ok' : HouseStats } |
  { 'Err' : string };
export type Result_8 = { 'Ok' : Array<Principal> } |
  { 'Err' : string };
export type Result_9 = { 'Ok' : LedgerOp } |
  { 'Err' : string };
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
  'username' : string,
  'player' : Principal,
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
  'name' : string,
  'locked' : boolean,
  'max_seats' : number,
  'avg_pot' : bigint,
  'players' : number,
  'currency' : Currency,
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
  'min_raise_to' : bigint,
  'seats' : Array<[] | [SeatView]>,
  'action_deadline' : bigint,
  'to_act' : [] | [number],
  'current_bet' : bigint,
  /**
   * Items thrown in the last few seconds, for clients to animate.
   */
  'throws' : Array<ThrowEvent>,
  'board' : Uint8Array | number[],
  'phase' : Phase,
  'config' : TableConfig,
  /**
   * The player who created this room, if it is a player-created table.
   */
  'room_owner' : [] | [Principal],
  'button' : number,
  'max_raise_to' : bigint,
}
export interface ThrowEvent {
  'id' : bigint,
  'from_seat' : number,
  'to_seat' : number,
  'icon' : string,
  'time' : bigint,
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
  'admin_get_metrics' : ActorMethod<[], Result_6>,
  'admin_house_stats' : ActorMethod<[], Result_7>,
  'admin_list_moderators' : ActorMethod<[], Result_8>,
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
   * Most new accounts per hour (sign-up throttle against bulk account creation).
   */
  'admin_set_signup_limit' : ActorMethod<[number], Result>,
  /**
   * Ledger operations whose outcome is unknown and need attention.
   */
  'admin_unresolved_ops' : ActorMethod<[], Result_10>,
  /**
   * Withdraws house revenue (rake and store sales) from the treasury.
   */
  'admin_withdraw_treasury' : ActorMethod<[Account, bigint], Result_5>,
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
   * Your recent deposits and withdrawals, newest first.
   */
  'get_my_ledger_ops' : ActorMethod<[], Array<LedgerOp>>,
  'get_my_profile' : ActorMethod<[], [] | [Profile]>,
  /**
   * All your notes, most recently updated first.
   */
  'get_notes' : ActorMethod<[], Array<NoteView>>,
  'get_profile' : ActorMethod<[Principal], [] | [PublicProfile]>,
  'get_status' : ActorMethod<[], ServiceStatus>,
  'get_table' : ActorMethod<[bigint], [] | [TableView]>,
  /**
   * `password` is only needed for password-protected player rooms.
   * Views of several tables at once (for multi-tabling); unknown ids are skipped.
   */
  'get_tables' : ActorMethod<[BigUint64Array | bigint[]], Array<TableView>>,
  'get_tournament' : ActorMethod<[bigint], [] | [TournamentDetail]>,
  /**
   * `GET /metrics` (Prometheus text) and `GET /health` on the canister's raw
   * domain. Responses are not certified, so they're for monitoring only.
   */
  'http_request' : ActorMethod<[HttpRequest], HttpResponse>,
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
  /**
   * Mutes a player in every chat for `hours` (0 lifts the mute).
   */
  'mod_mute' : ActorMethod<[Principal, number], Result>,
  'mod_open_reports' : ActorMethod<[], Result_11>,
  'mod_resolve_report' : ActorMethod<[bigint, ModAction, string], Result>,
  'mod_unban' : ActorMethod<[Principal], Result>,
  /**
   * Buys a fresh starting stack during a tournament's rebuy period (buy-in only, no fee).
   */
  'rebuy_tournament' : ActorMethod<[bigint], Result>,
  'register' : ActorMethod<[string], Result_12>,
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
  'set_status' : ActorMethod<[string], Result>,
  'sit_out' : ActorMethod<[bigint, boolean], Result>,
  'throw_item' : ActorMethod<[bigint, Principal, number, Currency], Result>,
  'top_up' : ActorMethod<[bigint, bigint], Result>,
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
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
  'wallet_balances' : bigint,
  'users' : bigint,
  'treasury' : bigint,
}
export interface InitArgs {
  'ledger' : [] | [Principal],
  'ledger_fee' : [] | [bigint],
}
export type ItemKind = { 'CardBack' : null } |
  { 'Throwable' : null } |
  { 'Avatar' : null } |
  { 'Gift' : null };
export interface LeaderboardEntry {
  'id' : Principal,
  'username' : string,
  'level' : number,
  'stats' : Stats,
  'avatar' : string,
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
export type MessageKind = { 'System' : null } |
  { 'Gift' : null } |
  { 'Text' : null } |
  { 'Throw' : null };
export type Phase = { 'Flop' : null } |
  { 'Turn' : null } |
  { 'River' : null } |
  { 'HandOver' : null } |
  { 'Waiting' : null } |
  { 'PreFlop' : null };
export interface PlayerResult {
  'won' : bigint,
  'player' : Principal,
  'committed' : bigint,
}
export interface Profile {
  'id' : Principal,
  'xp' : bigint,
  'status' : string,
  'friend_requests' : Array<Principal>,
  'username' : string,
  'balance' : bigint,
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
}
export interface PublicProfile {
  'id' : Principal,
  'status' : string,
  'username' : string,
  'level' : number,
  'tables' : BigUint64Array | bigint[],
  'stats' : Stats,
  'avatar' : string,
  'online' : boolean,
}
export type Result = { 'Ok' : null } |
  { 'Err' : string };
export type Result_1 = { 'Ok' : number } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : bigint } |
  { 'Err' : string };
export type Result_3 = { 'Ok' : HouseStats } |
  { 'Err' : string };
export type Result_4 = { 'Ok' : Profile } |
  { 'Err' : string };
export interface SeatView {
  'bet' : bigint,
  'in_hand' : boolean,
  'username' : string,
  'player' : Principal,
  'all_in' : boolean,
  'cards' : Uint8Array | number[],
  'gift' : [] | [string],
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
  'max_seats' : number,
  'avg_pot' : bigint,
  'players' : number,
  'currency' : Currency,
  'big_blind' : bigint,
  'phase' : Phase,
  'max_buy_in' : bigint,
}
export interface TableView {
  'id' : bigint,
  'now' : bigint,
  'pot' : bigint,
  'my_seat' : [] | [number],
  'to_call' : bigint,
  'hand_no' : bigint,
  'last_result' : [] | [HandResult],
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
export interface Winner {
  'player' : Principal,
  'hand' : string,
  'seat' : number,
  'amount' : bigint,
}
export interface _SERVICE {
  'act' : ActorMethod<[bigint, Action], Result>,
  'admin_add_item' : ActorMethod<[StoreItem], Result_1>,
  'admin_create_table' : ActorMethod<[TableConfig], Result_2>,
  'admin_house_stats' : ActorMethod<[], Result_3>,
  'admin_set_item_available' : ActorMethod<[number, boolean], Result>,
  /**
   * Withdraws house revenue (rake and store sales) from the treasury.
   */
  'admin_withdraw_treasury' : ActorMethod<[Account, bigint], Result_2>,
  'buy_item' : ActorMethod<[number, Currency], Result>,
  /**
   * Claims the daily free play-chip bonus (bigger on consecutive days).
   */
  'claim_daily_chips' : ActorMethod<[], Result_2>,
  /**
   * Deposit via plain transfer: send ICP to your deposit account (see
   * `get_deposit_info`) from any wallet or exchange, then call this.
   */
  'claim_deposit' : ActorMethod<[], Result_2>,
  /**
   * Deposit via ICRC-2: the caller first approves this canister for
   * `amount + fee` on the ICP ledger, then calls this.
   */
  'deposit' : ActorMethod<[bigint], Result_2>,
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
  'get_leaderboard' : ActorMethod<[], Array<LeaderboardEntry>>,
  'get_my_profile' : ActorMethod<[], [] | [Profile]>,
  'get_profile' : ActorMethod<[Principal], [] | [PublicProfile]>,
  'get_table' : ActorMethod<[bigint], [] | [TableView]>,
  'join_table' : ActorMethod<[bigint, number, bigint], Result>,
  'leave_table' : ActorMethod<[bigint], Result_2>,
  'list_store' : ActorMethod<[], Array<StoreItem>>,
  'list_tables' : ActorMethod<[], Array<TableSummary>>,
  'ping' : ActorMethod<[], undefined>,
  'register' : ActorMethod<[string], Result_4>,
  'remove_friend' : ActorMethod<[Principal], Result>,
  'respond_friend_request' : ActorMethod<[Principal, boolean], Result>,
  'send_chat' : ActorMethod<[Channel, string], Result>,
  'send_dm' : ActorMethod<[Principal, string], Result>,
  'send_friend_request' : ActorMethod<[Principal], Result>,
  'send_gift' : ActorMethod<[bigint, Principal, number, Currency], Result>,
  'set_status' : ActorMethod<[string], Result>,
  'sit_out' : ActorMethod<[bigint, boolean], Result>,
  'throw_item' : ActorMethod<[bigint, Principal, number, Currency], Result>,
  'top_up' : ActorMethod<[bigint, bigint], Result>,
  'update_settings' : ActorMethod<[Settings], Result>,
  'whoami' : ActorMethod<[], Principal>,
  /**
   * Sends `amount` e8s to `to`. The ledger fee is charged on top.
   */
  'withdraw' : ActorMethod<[Account, bigint], Result_2>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
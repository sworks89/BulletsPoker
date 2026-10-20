export const idlFactory = ({ IDL }) => {
  const InitArgs = IDL.Record({
    'ledger' : IDL.Opt(IDL.Principal),
    'ledger_fee' : IDL.Opt(IDL.Nat64),
  });
  const Action = IDL.Variant({
    'Call' : IDL.Null,
    'Fold' : IDL.Null,
    'Raise' : IDL.Nat64,
    'AllIn' : IDL.Null,
    'Check' : IDL.Null,
  });
  const Result = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : IDL.Text });
  const ItemKind = IDL.Variant({
    'CardBack' : IDL.Null,
    'Throwable' : IDL.Null,
    'Avatar' : IDL.Null,
    'Gift' : IDL.Null,
  });
  const StoreItem = IDL.Record({
    'id' : IDL.Nat32,
    'icon' : IDL.Text,
    'kind' : ItemKind,
    'name' : IDL.Text,
    'description' : IDL.Text,
    'available' : IDL.Bool,
    'chip_price' : IDL.Nat64,
    'price' : IDL.Nat64,
  });
  const Result_1 = IDL.Variant({ 'Ok' : IDL.Nat32, 'Err' : IDL.Text });
  const TournamentKind = IDL.Variant({
    'Scheduled' : IDL.Null,
    'SitAndGo' : IDL.Null,
  });
  const Currency = IDL.Variant({ 'Icp' : IDL.Null, 'Chips' : IDL.Null });
  const TournamentTemplate = IDL.Record({
    'fee' : IDL.Nat64,
    'max_rebuys' : IDL.Nat32,
    'min_players' : IDL.Nat32,
    'rebuy_levels' : IDL.Nat32,
    'addon_chips' : IDL.Nat64,
    'kind' : TournamentKind,
    'name' : IDL.Text,
    'level_secs' : IDL.Nat64,
    'interval_secs' : IDL.Nat64,
    'max_players' : IDL.Nat32,
    'guarantee' : IDL.Nat64,
    'addon_cost' : IDL.Nat64,
    'currency' : Currency,
    'starting_stack' : IDL.Nat64,
    'buy_in' : IDL.Nat64,
    'table_size' : IDL.Nat8,
    'late_reg_levels' : IDL.Nat32,
  });
  const AuditEntry = IDL.Record({
    'action' : IDL.Text,
    'actor' : IDL.Principal,
    'time' : IDL.Nat64,
  });
  const Result_2 = IDL.Variant({
    'Ok' : IDL.Vec(AuditEntry),
    'Err' : IDL.Text,
  });
  const Result_3 = IDL.Variant({
    'Ok' : IDL.Tuple(IDL.Nat64, IDL.Nat64),
    'Err' : IDL.Text,
  });
  const SolvencyReport = IDL.Record({
    'ok' : IDL.Bool,
    'liabilities' : IDL.Nat64,
    'ledger_balance' : IDL.Nat64,
    'surplus' : IDL.Int,
    'unresolved_ops' : IDL.Nat64,
    'checked_at' : IDL.Nat64,
  });
  const Result_4 = IDL.Variant({ 'Ok' : SolvencyReport, 'Err' : IDL.Text });
  const TableConfig = IDL.Record({
    'small_blind' : IDL.Nat64,
    'action_timeout_secs' : IDL.Nat64,
    'min_buy_in' : IDL.Nat64,
    'name' : IDL.Text,
    'jackpot_bps' : IDL.Nat16,
    'jackpot_cap' : IDL.Nat64,
    'rake_bps' : IDL.Nat16,
    'rake_cap' : IDL.Nat64,
    'max_seats' : IDL.Nat8,
    'currency' : Currency,
    'big_blind' : IDL.Nat64,
    'max_buy_in' : IDL.Nat64,
  });
  const Result_5 = IDL.Variant({ 'Ok' : IDL.Nat64, 'Err' : IDL.Text });
  const AdminConfig = IDL.Record({
    'moderators' : IDL.Vec(IDL.Tuple(IDL.Principal, IDL.Text)),
    'banned_words' : IDL.Vec(IDL.Text),
    'signups_per_hour' : IDL.Nat32,
    'signup_pow_bits' : IDL.Nat8,
  });
  const Result_6 = IDL.Variant({ 'Ok' : AdminConfig, 'Err' : IDL.Text });
  const Metrics = IDL.Record({
    'running_tournaments' : IDL.Nat64,
    'last_solvency_surplus_e8s' : IDL.Opt(IDL.Int64),
    'treasury_e8s' : IDL.Nat64,
    'jackpot_chips' : IDL.Nat64,
    'bans' : IDL.Nat64,
    'last_solvency_at' : IDL.Opt(IDL.Nat64),
    'last_solvency_ok' : IDL.Opt(IDL.Bool),
    'idle_stand_ups' : IDL.Nat64,
    'reports_filed' : IDL.Nat64,
    'open_reports' : IDL.Nat64,
    'time' : IDL.Nat64,
    'seated_players' : IDL.Nat64,
    'ledger_ops_unknown' : IDL.Nat64,
    'paused_deposits' : IDL.Bool,
    'paused_joins' : IDL.Bool,
    'tables' : IDL.Nat64,
    'cycles' : IDL.Nat64,
    'liabilities_e8s' : IDL.Nat64,
    'paused_withdrawals' : IDL.Bool,
    'users' : IDL.Nat64,
    'ledger_ops_failed' : IDL.Nat64,
    'active_players_5m' : IDL.Nat64,
    'unresolved_ledger_ops' : IDL.Nat64,
    'hands_dealt' : IDL.Nat64,
    'wallet_total_e8s' : IDL.Nat64,
    'paused_dealing' : IDL.Bool,
    'stable_bytes' : IDL.Nat64,
    'active_tables' : IDL.Nat64,
    'jackpot_icp_e8s' : IDL.Nat64,
    'rate_limited_calls' : IDL.Nat64,
    'heap_bytes' : IDL.Nat64,
  });
  const Result_7 = IDL.Variant({ 'Ok' : Metrics, 'Err' : IDL.Text });
  const HouseStats = IDL.Record({
    'liabilities_total' : IDL.Nat64,
    'chips_on_tables' : IDL.Nat64,
    'jackpot' : IDL.Nat64,
    'wallet_balances' : IDL.Nat64,
    'rewards_pools' : IDL.Nat64,
    'users' : IDL.Nat64,
    'treasury' : IDL.Nat64,
  });
  const Result_8 = IDL.Variant({ 'Ok' : HouseStats, 'Err' : IDL.Text });
  const Account = IDL.Record({
    'owner' : IDL.Principal,
    'subaccount' : IDL.Opt(IDL.Vec(IDL.Nat8)),
  });
  const OpStatus = IDL.Variant({
    'Failed' : IDL.Record({ 'reason' : IDL.Text }),
    'Unknown' : IDL.Record({ 'attempts' : IDL.Nat32, 'reason' : IDL.Text }),
    'Completed' : IDL.Record({ 'block' : IDL.Nat64 }),
    'Pending' : IDL.Null,
  });
  const OpKind = IDL.Variant({
    'Withdraw' : IDL.Null,
    'Sweep' : IDL.Null,
    'Deposit' : IDL.Null,
    'Treasury' : IDL.Null,
  });
  const LedgerOp = IDL.Record({
    'id' : IDL.Nat64,
    'to' : Account,
    'fee' : IDL.Nat64,
    'status' : OpStatus,
    'updated_at' : IDL.Nat64,
    'from' : IDL.Opt(Account),
    'kind' : OpKind,
    'user' : IDL.Principal,
    'from_subaccount' : IDL.Opt(IDL.Vec(IDL.Nat8)),
    'created_at_time' : IDL.Nat64,
    'amount' : IDL.Nat64,
  });
  const Result_9 = IDL.Variant({ 'Ok' : LedgerOp, 'Err' : IDL.Text });
  const PauseFlags = IDL.Record({
    'joins' : IDL.Bool,
    'withdrawals' : IDL.Bool,
    'deposits' : IDL.Bool,
    'dealing' : IDL.Bool,
    'reason' : IDL.Opt(IDL.Text),
  });
  const Result_10 = IDL.Variant({ 'Ok' : IDL.Vec(LedgerOp), 'Err' : IDL.Text });
  const AutoSitStatus = IDL.Variant({
    'Queued' : IDL.Record({
      'place' : IDL.Nat32,
      'buy_in' : IDL.Nat64,
      'waiting' : IDL.Nat32,
    }),
    'Seated' : IDL.Record({ 'table' : IDL.Nat64 }),
    'Dropped' : IDL.Record({ 'reason' : IDL.Text }),
    'NotQueued' : IDL.Null,
  });
  const Result_11 = IDL.Variant({ 'Ok' : AutoSitStatus, 'Err' : IDL.Text });
  const RoomArgs = IDL.Record({
    'action_timeout_secs' : IDL.Nat64,
    'password' : IDL.Opt(IDL.Text),
    'name' : IDL.Text,
    'max_seats' : IDL.Nat8,
    'currency' : Currency,
    'big_blind' : IDL.Nat64,
  });
  const Stats = IDL.Record({
    'biggest_pot_won' : IDL.Nat64,
    'hands_won' : IDL.Nat64,
    'chip_net' : IDL.Int64,
    'net_winnings' : IDL.Int64,
    'hands_played' : IDL.Nat64,
  });
  const PublicProfile = IDL.Record({
    'id' : IDL.Principal,
    'status' : IDL.Text,
    'username' : IDL.Text,
    'level' : IDL.Nat32,
    'tables' : IDL.Vec(IDL.Nat64),
    'stats' : Stats,
    'avatar' : IDL.Text,
    'medals' : IDL.Vec(IDL.Text),
    'online' : IDL.Bool,
  });
  const Channel = IDL.Variant({ 'Lobby' : IDL.Null, 'Table' : IDL.Nat64 });
  const MessageKind = IDL.Variant({
    'System' : IDL.Null,
    'Gift' : IDL.Null,
    'Text' : IDL.Null,
    'Throw' : IDL.Null,
  });
  const ChatMessage = IDL.Record({
    'id' : IDL.Nat64,
    'username' : IDL.Text,
    'from' : IDL.Opt(IDL.Principal),
    'kind' : MessageKind,
    'text' : IDL.Text,
    'time' : IDL.Nat64,
  });
  const DepositInfo = IDL.Record({
    'fee' : IDL.Nat64,
    'account_id' : IDL.Text,
    'ledger' : IDL.Principal,
    'account' : Account,
  });
  const FriendInfo = IDL.Record({
    'id' : IDL.Principal,
    'username' : IDL.Text,
    'tables' : IDL.Vec(IDL.Nat64),
    'avatar' : IDL.Text,
    'online' : IDL.Bool,
  });
  const Phase = IDL.Variant({
    'Flop' : IDL.Null,
    'Turn' : IDL.Null,
    'River' : IDL.Null,
    'HandOver' : IDL.Null,
    'Waiting' : IDL.Null,
    'PreFlop' : IDL.Null,
  });
  const LogEntry = IDL.Record({
    'seat' : IDL.Opt(IDL.Nat8),
    'text' : IDL.Text,
    'phase' : Phase,
    'amount' : IDL.Nat64,
  });
  const HandPlayer = IDL.Record({
    'won' : IDL.Nat64,
    'username' : IDL.Text,
    'player' : IDL.Principal,
    'cards' : IDL.Vec(IDL.Nat8),
    'seat' : IDL.Nat8,
    'committed' : IDL.Nat64,
    'shown' : IDL.Bool,
  });
  const HandRecord = IDL.Record({
    'id' : IDL.Nat64,
    'log' : IDL.Vec(LogEntry),
    'pot' : IDL.Nat64,
    'small_blind' : IDL.Nat64,
    'hand_no' : IDL.Nat64,
    'rake' : IDL.Nat64,
    'time' : IDL.Nat64,
    'table_id' : IDL.Nat64,
    'players' : IDL.Vec(HandPlayer),
    'currency' : Currency,
    'big_blind' : IDL.Nat64,
    'board' : IDL.Vec(IDL.Nat8),
    'button' : IDL.Nat8,
    'table_name' : IDL.Text,
  });
  const HandSummary = IDL.Record({
    'id' : IDL.Nat64,
    'net' : IDL.Int64,
    'pot' : IDL.Nat64,
    'my_cards' : IDL.Vec(IDL.Nat8),
    'time' : IDL.Nat64,
    'currency' : Currency,
    'board' : IDL.Vec(IDL.Nat8),
    'table_name' : IDL.Text,
  });
  const JackpotPayout = IDL.Record({
    'username' : IDL.Text,
    'player' : IDL.Principal,
    'share' : IDL.Text,
    'amount' : IDL.Nat64,
  });
  const JackpotHit = IDL.Record({
    'winner_cards' : IDL.Vec(IDL.Nat8),
    'total' : IDL.Nat64,
    'loser_hand' : IDL.Text,
    'loser_name' : IDL.Text,
    'hand_no' : IDL.Nat64,
    'time' : IDL.Nat64,
    'table_id' : IDL.Nat64,
    'winner' : IDL.Principal,
    'loser' : IDL.Principal,
    'currency' : Currency,
    'board' : IDL.Vec(IDL.Nat8),
    'loser_cards' : IDL.Vec(IDL.Nat8),
    'table_name' : IDL.Text,
    'payouts' : IDL.Vec(JackpotPayout),
    'winner_hand' : IDL.Text,
    'winner_name' : IDL.Text,
  });
  const JackpotInfo = IDL.Record({
    'icp' : IDL.Nat64,
    'min_players' : IDL.Nat8,
    'chips' : IDL.Nat64,
    'qualifying_hand' : IDL.Text,
    'winner_bps' : IDL.Nat64,
    'loser_bps' : IDL.Nat64,
    'payout_bps' : IDL.Nat64,
    'drop_bps' : IDL.Nat16,
    'recent' : IDL.Vec(JackpotHit),
  });
  const LeaderboardEntry = IDL.Record({
    'id' : IDL.Principal,
    'username' : IDL.Text,
    'level' : IDL.Nat32,
    'stats' : Stats,
    'avatar' : IDL.Text,
  });
  const Ban = IDL.Record({ 'until' : IDL.Nat64, 'reason' : IDL.Text });
  const Settings = IDL.Record({
    'auto_top_up' : IDL.Bool,
    'auto_muck' : IDL.Bool,
    'four_color_deck' : IDL.Bool,
    'sound' : IDL.Bool,
    'show_throwables' : IDL.Bool,
  });
  const EarnedMedal = IDL.Record({ 'id' : IDL.Text, 'earned_at' : IDL.Nat64 });
  const Profile = IDL.Record({
    'id' : IDL.Principal,
    'xp' : IDL.Nat64,
    'ban' : IDL.Opt(Ban),
    'status' : IDL.Text,
    'friend_requests' : IDL.Vec(IDL.Principal),
    'chat_muted_until' : IDL.Nat64,
    'username' : IDL.Text,
    'balance' : IDL.Nat64,
    'blocked' : IDL.Vec(IDL.Principal),
    'chips' : IDL.Nat64,
    'inventory' : IDL.Vec(IDL.Nat32),
    'next_daily_at' : IDL.Nat64,
    'created_at' : IDL.Nat64,
    'level' : IDL.Nat32,
    'next_daily_amount' : IDL.Nat64,
    'stats' : Stats,
    'settings' : Settings,
    'card_back' : IDL.Nat32,
    'friends' : IDL.Vec(IDL.Principal),
    'daily_streak' : IDL.Nat32,
    'avatar' : IDL.Nat32,
    'medals' : IDL.Vec(EarnedMedal),
  });
  const Roles = IDL.Record({ 'admin' : IDL.Bool, 'moderator' : IDL.Bool });
  const NoteColor = IDL.Variant({
    'Red' : IDL.Null,
    'Yellow' : IDL.Null,
    'Blue' : IDL.Null,
    'Green' : IDL.Null,
    'Orange' : IDL.Null,
    'Purple' : IDL.Null,
  });
  const NoteView = IDL.Record({
    'updated_at' : IDL.Nat64,
    'username' : IDL.Text,
    'player' : IDL.Principal,
    'color' : IDL.Opt(NoteColor),
    'text' : IDL.Text,
    'avatar' : IDL.Text,
  });
  const SignupChallenge = IDL.Record({ 'difficulty' : IDL.Nat8 });
  const ServiceStatus = IDL.Record({
    'solvency' : IDL.Opt(SolvencyReport),
    'pause' : PauseFlags,
  });
  const AutoSitView = IDL.Record({ 'place' : IDL.Nat32, 'buy_in' : IDL.Nat64 });
  const ShownHand = IDL.Record({
    'player' : IDL.Principal,
    'cards' : IDL.Vec(IDL.Nat8),
    'hand' : IDL.Text,
    'seat' : IDL.Nat8,
  });
  const PlayerResult = IDL.Record({
    'won' : IDL.Nat64,
    'player' : IDL.Principal,
    'committed' : IDL.Nat64,
  });
  const Winner = IDL.Record({
    'player' : IDL.Principal,
    'hand' : IDL.Text,
    'seat' : IDL.Nat8,
    'amount' : IDL.Nat64,
  });
  const HandResult = IDL.Record({
    'pot' : IDL.Nat64,
    'hand_no' : IDL.Nat64,
    'rake' : IDL.Nat64,
    'shown' : IDL.Vec(ShownHand),
    'players' : IDL.Vec(PlayerResult),
    'board' : IDL.Vec(IDL.Nat8),
    'winners' : IDL.Vec(Winner),
    'jackpot_drop' : IDL.Nat64,
  });
  const TournamentBadge = IDL.Record({
    'id' : IDL.Nat64,
    'can_rebuy' : IDL.Bool,
    'small_blind' : IDL.Nat64,
    'rebuy_cost' : IDL.Nat64,
    'can_addon' : IDL.Bool,
    'addon_chips' : IDL.Nat64,
    'name' : IDL.Text,
    'level' : IDL.Nat32,
    'addon_cost' : IDL.Nat64,
    'finished' : IDL.Bool,
    'currency' : Currency,
    'big_blind' : IDL.Nat64,
    'entrants' : IDL.Nat32,
    'players_left' : IDL.Nat32,
    'rebuys_left' : IDL.Nat32,
    'my_place' : IDL.Opt(IDL.Nat32),
    'my_prize' : IDL.Nat64,
    'rebuy_deadline' : IDL.Nat64,
    'next_level_at' : IDL.Nat64,
  });
  const PlayerNote = IDL.Record({
    'updated_at' : IDL.Nat64,
    'color' : IDL.Opt(NoteColor),
    'text' : IDL.Text,
  });
  const SeatView = IDL.Record({
    'bet' : IDL.Nat64,
    'in_hand' : IDL.Bool,
    'time_bank_secs' : IDL.Nat32,
    'username' : IDL.Text,
    'player' : IDL.Principal,
    'gift_from' : IDL.Opt(IDL.Text),
    'all_in' : IDL.Bool,
    'cards' : IDL.Vec(IDL.Nat8),
    'gift' : IDL.Opt(IDL.Text),
    'note' : IDL.Opt(PlayerNote),
    'has_cards' : IDL.Bool,
    'seat' : IDL.Nat8,
    'last_action' : IDL.Opt(IDL.Text),
    'gift_sent_at' : IDL.Opt(IDL.Nat64),
    'level' : IDL.Nat32,
    'stack' : IDL.Nat64,
    'card_back' : IDL.Text,
    'sitting_out' : IDL.Bool,
    'folded' : IDL.Bool,
    'avatar' : IDL.Text,
  });
  const ThrowEvent = IDL.Record({
    'id' : IDL.Nat64,
    'from_seat' : IDL.Nat8,
    'to_seat' : IDL.Nat8,
    'icon' : IDL.Text,
    'time' : IDL.Nat64,
  });
  const TableView = IDL.Record({
    'id' : IDL.Nat64,
    'now' : IDL.Nat64,
    'pot' : IDL.Nat64,
    'my_seat' : IDL.Opt(IDL.Nat8),
    'to_call' : IDL.Nat64,
    'my_auto_sit' : IDL.Opt(AutoSitView),
    'game' : IDL.Nat64,
    'hand_no' : IDL.Nat64,
    'jackpot' : IDL.Nat64,
    'jackpot_hit' : IDL.Opt(JackpotHit),
    'locked' : IDL.Bool,
    'last_result' : IDL.Opt(HandResult),
    'tournament' : IDL.Opt(TournamentBadge),
    'time_bank_active' : IDL.Bool,
    'min_raise_to' : IDL.Nat64,
    'seats' : IDL.Vec(IDL.Opt(SeatView)),
    'action_deadline' : IDL.Nat64,
    'to_act' : IDL.Opt(IDL.Nat8),
    'current_bet' : IDL.Nat64,
    'throws' : IDL.Vec(ThrowEvent),
    'number' : IDL.Nat8,
    'restricted' : IDL.Bool,
    'board' : IDL.Vec(IDL.Nat8),
    'phase' : Phase,
    'config' : TableConfig,
    'room_owner' : IDL.Opt(IDL.Principal),
    'button' : IDL.Nat8,
    'auto_sit_waiting' : IDL.Nat32,
    'my_min_buy_in' : IDL.Nat64,
    'max_raise_to' : IDL.Nat64,
  });
  const Standing = IDL.Record({
    'table' : IDL.Opt(IDL.Nat64),
    'username' : IDL.Text,
    'player' : IDL.Principal,
    'chips' : IDL.Nat64,
    'place' : IDL.Opt(IDL.Nat32),
    'prize' : IDL.Nat64,
    'avatar' : IDL.Text,
  });
  const TournamentStatus = IDL.Variant({
    'Finished' : IDL.Null,
    'Running' : IDL.Null,
    'Cancelled' : IDL.Null,
    'Registering' : IDL.Null,
  });
  const TournamentSummary = IDL.Record({
    'id' : IDL.Nat64,
    'fee' : IDL.Nat64,
    'status' : TournamentStatus,
    'max_rebuys' : IDL.Nat32,
    'my_table' : IDL.Opt(IDL.Nat64),
    'min_players' : IDL.Nat32,
    'rebuy_levels' : IDL.Nat32,
    'start_at' : IDL.Nat64,
    'addon_chips' : IDL.Nat64,
    'kind' : TournamentKind,
    'name' : IDL.Text,
    'level_secs' : IDL.Nat64,
    'max_players' : IDL.Nat32,
    'level' : IDL.Nat32,
    'entries' : IDL.Nat32,
    'addon_cost' : IDL.Nat64,
    'currency' : Currency,
    'prize_pool' : IDL.Nat64,
    'players_left' : IDL.Nat32,
    'registered_me' : IDL.Bool,
    'starting_stack' : IDL.Nat64,
    'buy_in' : IDL.Nat64,
    'late_reg_levels' : IDL.Nat32,
    'registered' : IDL.Nat32,
    'can_register' : IDL.Bool,
  });
  const TournamentDetail = IDL.Record({
    'small_blind' : IDL.Nat64,
    'standings' : IDL.Vec(Standing),
    'level' : IDL.Nat32,
    'tables' : IDL.Vec(IDL.Nat64),
    'summary' : TournamentSummary,
    'big_blind' : IDL.Nat64,
    'next_level_at' : IDL.Nat64,
    'payouts' : IDL.Vec(IDL.Nat64),
  });
  const HttpRequest = IDL.Record({
    'url' : IDL.Text,
    'method' : IDL.Text,
    'body' : IDL.Vec(IDL.Nat8),
    'headers' : IDL.Vec(IDL.Tuple(IDL.Text, IDL.Text)),
  });
  const HttpResponse = IDL.Record({
    'body' : IDL.Vec(IDL.Nat8),
    'headers' : IDL.Vec(IDL.Tuple(IDL.Text, IDL.Text)),
    'status_code' : IDL.Nat16,
  });
  const MedalDef = IDL.Record({
    'id' : IDL.Text,
    'icon' : IDL.Text,
    'name' : IDL.Text,
    'description' : IDL.Text,
  });
  const TableSummary = IDL.Record({
    'id' : IDL.Nat64,
    'small_blind' : IDL.Nat64,
    'min_buy_in' : IDL.Nat64,
    'game' : IDL.Nat64,
    'name' : IDL.Text,
    'game_at_cap' : IDL.Bool,
    'locked' : IDL.Bool,
    'max_seats' : IDL.Nat8,
    'avg_pot' : IDL.Nat64,
    'players' : IDL.Nat8,
    'currency' : Currency,
    'number' : IDL.Nat8,
    'big_blind' : IDL.Nat64,
    'phase' : Phase,
    'room_owner' : IDL.Opt(IDL.Text),
    'max_buy_in' : IDL.Nat64,
  });
  const ModAction = IDL.Variant({
    'Ban' : IDL.Nat32,
    'Warn' : IDL.Null,
    'Dismiss' : IDL.Null,
    'MuteChat' : IDL.Nat32,
  });
  const ReportStatus = IDL.Variant({
    'Open' : IDL.Null,
    'Resolved' : IDL.Record({
      'at' : IDL.Nat64,
      'by' : IDL.Principal,
      'action' : ModAction,
      'note' : IDL.Text,
    }),
  });
  const ReportReason = IDL.Variant({
    'Spam' : IDL.Null,
    'Cheating' : IDL.Null,
    'Collusion' : IDL.Null,
    'Other' : IDL.Null,
    'OffensiveName' : IDL.Null,
    'AbusiveChat' : IDL.Null,
  });
  const Report = IDL.Record({
    'id' : IDL.Nat64,
    'status' : ReportStatus,
    'created_at' : IDL.Nat64,
    'target' : IDL.Principal,
    'evidence' : IDL.Vec(ChatMessage),
    'reporter_name' : IDL.Text,
    'details' : IDL.Text,
    'channel' : IDL.Opt(Channel),
    'reporter' : IDL.Principal,
    'target_name' : IDL.Text,
    'reason' : ReportReason,
  });
  const ModPlayerView = IDL.Record({
    'id' : IDL.Principal,
    'ban' : IDL.Opt(Ban),
    'status' : IDL.Text,
    'chat_muted_until' : IDL.Nat64,
    'username' : IDL.Text,
    'balance' : IDL.Nat64,
    'chips' : IDL.Nat64,
    'is_moderator' : IDL.Bool,
    'created_at' : IDL.Nat64,
    'level' : IDL.Nat32,
    'tables' : IDL.Vec(IDL.Nat64),
    'reports' : IDL.Vec(Report),
    'last_seen' : IDL.Nat64,
    'net_winnings' : IDL.Int64,
    'hands_played' : IDL.Nat64,
  });
  const Result_12 = IDL.Variant({ 'Ok' : ModPlayerView, 'Err' : IDL.Text });
  const ReportPage = IDL.Record({
    'open_total' : IDL.Nat64,
    'reports' : IDL.Vec(Report),
  });
  const Result_13 = IDL.Variant({ 'Ok' : ReportPage, 'Err' : IDL.Text });
  const Result_14 = IDL.Variant({ 'Ok' : IDL.Vec(Report), 'Err' : IDL.Text });
  const Result_15 = IDL.Variant({ 'Ok' : Profile, 'Err' : IDL.Text });
  return IDL.Service({
    'act' : IDL.Func([IDL.Nat64, Action], [Result], []),
    'addon_tournament' : IDL.Func([IDL.Nat64], [Result], []),
    'admin_add_item' : IDL.Func([StoreItem], [Result_1], []),
    'admin_add_tournament' : IDL.Func([TournamentTemplate], [Result_1], []),
    'admin_audit_log' : IDL.Func([IDL.Nat32], [Result_2], ['query']),
    'admin_audit_wallet_total' : IDL.Func([], [Result_3], ['query']),
    'admin_check_solvency' : IDL.Func([], [Result_4], []),
    'admin_create_table' : IDL.Func([TableConfig], [Result_5], []),
    'admin_get_config' : IDL.Func([], [Result_6], ['query']),
    'admin_get_metrics' : IDL.Func([], [Result_7], ['query']),
    'admin_house_stats' : IDL.Func([], [Result_8], ['query']),
    'admin_resolve_op' : IDL.Func(
        [IDL.Nat64, IDL.Bool, IDL.Opt(IDL.Nat64)],
        [Result_9],
        [],
      ),
    'admin_retry_op' : IDL.Func([IDL.Nat64], [Result_9], []),
    'admin_seed_jackpot' : IDL.Func([IDL.Nat64], [Result], []),
    'admin_set_banned_words' : IDL.Func([IDL.Vec(IDL.Text)], [Result], []),
    'admin_set_item_available' : IDL.Func([IDL.Nat32, IDL.Bool], [Result], []),
    'admin_set_moderator' : IDL.Func([IDL.Principal, IDL.Bool], [Result], []),
    'admin_set_pause' : IDL.Func([PauseFlags], [Result], []),
    'admin_set_signup_limit' : IDL.Func([IDL.Nat32], [Result], []),
    'admin_set_signup_pow' : IDL.Func([IDL.Nat8], [Result], []),
    'admin_unresolved_ops' : IDL.Func([], [Result_10], ['query']),
    'admin_withdraw_treasury' : IDL.Func([Account, IDL.Nat64], [Result_5], []),
    'auto_sit' : IDL.Func([IDL.Nat64, IDL.Opt(IDL.Nat64)], [Result_11], []),
    'auto_sit_ping' : IDL.Func([IDL.Nat64], [Result_11], []),
    'block_player' : IDL.Func([IDL.Principal, IDL.Bool], [Result], []),
    'buy_item' : IDL.Func([IDL.Nat32, Currency], [Result], []),
    'claim_daily_chips' : IDL.Func([], [Result_5], []),
    'claim_deposit' : IDL.Func([], [Result_5], []),
    'close_room' : IDL.Func([IDL.Nat64], [Result], []),
    'create_room' : IDL.Func([RoomArgs], [Result_5], []),
    'deposit' : IDL.Func([IDL.Nat64], [Result_5], []),
    'equip' : IDL.Func([IDL.Nat32], [Result], []),
    'find_user' : IDL.Func([IDL.Text], [IDL.Opt(PublicProfile)], ['query']),
    'get_chat' : IDL.Func(
        [Channel, IDL.Nat64],
        [IDL.Vec(ChatMessage)],
        ['query'],
      ),
    'get_deposit_info' : IDL.Func([], [DepositInfo], ['query']),
    'get_dms' : IDL.Func(
        [IDL.Principal, IDL.Nat64],
        [IDL.Vec(ChatMessage)],
        ['query'],
      ),
    'get_friends' : IDL.Func([], [IDL.Vec(FriendInfo)], ['query']),
    'get_hand' : IDL.Func([IDL.Nat64], [IDL.Opt(HandRecord)], ['query']),
    'get_hand_history' : IDL.Func(
        [IDL.Nat32],
        [IDL.Vec(HandSummary)],
        ['query'],
      ),
    'get_jackpots' : IDL.Func([], [JackpotInfo], ['query']),
    'get_leaderboard' : IDL.Func([], [IDL.Vec(LeaderboardEntry)], ['query']),
    'get_my_ledger_ops' : IDL.Func([], [IDL.Vec(LedgerOp)], ['query']),
    'get_my_profile' : IDL.Func([], [IDL.Opt(Profile)], ['query']),
    'get_my_roles' : IDL.Func([], [Roles], ['query']),
    'get_notes' : IDL.Func([], [IDL.Vec(NoteView)], ['query']),
    'get_profile' : IDL.Func(
        [IDL.Principal],
        [IDL.Opt(PublicProfile)],
        ['query'],
      ),
    'get_signup_challenge' : IDL.Func([], [SignupChallenge], ['query']),
    'get_status' : IDL.Func([], [ServiceStatus], ['query']),
    'get_table' : IDL.Func([IDL.Nat64], [IDL.Opt(TableView)], ['query']),
    'get_tables' : IDL.Func(
        [IDL.Vec(IDL.Nat64)],
        [IDL.Vec(TableView)],
        ['query'],
      ),
    'get_tournament' : IDL.Func(
        [IDL.Nat64],
        [IDL.Opt(TournamentDetail)],
        ['query'],
      ),
    'http_request' : IDL.Func([HttpRequest], [HttpResponse], ['query']),
    'join_game' : IDL.Func([IDL.Nat64, IDL.Nat64], [Result_5], []),
    'join_table' : IDL.Func(
        [IDL.Nat64, IDL.Nat8, IDL.Nat64, IDL.Opt(IDL.Text)],
        [Result],
        [],
      ),
    'leave_table' : IDL.Func([IDL.Nat64], [Result_5], []),
    'list_medals' : IDL.Func([], [IDL.Vec(MedalDef)], ['query']),
    'list_store' : IDL.Func([], [IDL.Vec(StoreItem)], ['query']),
    'list_tables' : IDL.Func([], [IDL.Vec(TableSummary)], ['query']),
    'list_tournaments' : IDL.Func([], [IDL.Vec(TournamentSummary)], ['query']),
    'mod_ban' : IDL.Func([IDL.Principal, IDL.Nat32, IDL.Text], [Result], []),
    'mod_get_player' : IDL.Func([IDL.Principal], [Result_12], ['query']),
    'mod_mute' : IDL.Func([IDL.Principal, IDL.Nat32], [Result], []),
    'mod_open_reports' : IDL.Func(
        [IDL.Opt(IDL.Nat64), IDL.Nat32],
        [Result_13],
        ['query'],
      ),
    'mod_recent_reports' : IDL.Func([IDL.Nat32], [Result_14], ['query']),
    'mod_resolve_report' : IDL.Func(
        [IDL.Nat64, ModAction, IDL.Text],
        [Result],
        [],
      ),
    'mod_unban' : IDL.Func([IDL.Principal], [Result], []),
    'rebuy_tournament' : IDL.Func([IDL.Nat64], [Result], []),
    'register' : IDL.Func([IDL.Text, IDL.Opt(IDL.Nat64)], [Result_15], []),
    'register_tournament' : IDL.Func([IDL.Nat64], [Result], []),
    'remove_friend' : IDL.Func([IDL.Principal], [Result], []),
    'report_player' : IDL.Func(
        [IDL.Principal, ReportReason, IDL.Text, IDL.Opt(Channel)],
        [Result_5],
        [],
      ),
    'respond_friend_request' : IDL.Func(
        [IDL.Principal, IDL.Bool],
        [Result],
        [],
      ),
    'send_chat' : IDL.Func([Channel, IDL.Text], [Result], []),
    'send_dm' : IDL.Func([IDL.Principal, IDL.Text], [Result], []),
    'send_friend_request' : IDL.Func([IDL.Principal], [Result], []),
    'send_gift' : IDL.Func(
        [IDL.Nat64, IDL.Principal, IDL.Nat32, Currency],
        [Result],
        [],
      ),
    'set_note' : IDL.Func(
        [IDL.Principal, IDL.Text, IDL.Opt(NoteColor)],
        [Result],
        [],
      ),
    'set_status' : IDL.Func([IDL.Text], [Result], []),
    'show_cards' : IDL.Func([IDL.Nat64], [Result], []),
    'sit_out' : IDL.Func([IDL.Nat64, IDL.Bool], [Result], []),
    'throw_item' : IDL.Func(
        [IDL.Nat64, IDL.Principal, IDL.Nat32, Currency],
        [Result],
        [],
      ),
    'top_up' : IDL.Func([IDL.Nat64, IDL.Nat64], [Result], []),
    'unlock_room' : IDL.Func([IDL.Nat64, IDL.Text], [Result], []),
    'unregister_tournament' : IDL.Func([IDL.Nat64], [Result], []),
    'update_settings' : IDL.Func([Settings], [Result], []),
    'whoami' : IDL.Func([], [IDL.Principal], ['query']),
    'withdraw' : IDL.Func([Account, IDL.Nat64], [Result_5], []),
  });
};
export const init = ({ IDL }) => {
  const InitArgs = IDL.Record({
    'ledger' : IDL.Opt(IDL.Principal),
    'ledger_fee' : IDL.Opt(IDL.Nat64),
  });
  return [IDL.Opt(InitArgs)];
};
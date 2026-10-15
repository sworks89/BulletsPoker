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
    'price' : IDL.Nat64,
  });
  const Result_1 = IDL.Variant({ 'Ok' : IDL.Nat32, 'Err' : IDL.Text });
  const TableConfig = IDL.Record({
    'small_blind' : IDL.Nat64,
    'action_timeout_secs' : IDL.Nat64,
    'min_buy_in' : IDL.Nat64,
    'name' : IDL.Text,
    'rake_bps' : IDL.Nat16,
    'rake_cap' : IDL.Nat64,
    'max_seats' : IDL.Nat8,
    'big_blind' : IDL.Nat64,
    'max_buy_in' : IDL.Nat64,
  });
  const Result_2 = IDL.Variant({ 'Ok' : IDL.Nat64, 'Err' : IDL.Text });
  const HouseStats = IDL.Record({
    'liabilities_total' : IDL.Nat64,
    'chips_on_tables' : IDL.Nat64,
    'wallet_balances' : IDL.Nat64,
    'users' : IDL.Nat64,
    'treasury' : IDL.Nat64,
  });
  const Result_3 = IDL.Variant({ 'Ok' : HouseStats, 'Err' : IDL.Text });
  const Account = IDL.Record({
    'owner' : IDL.Principal,
    'subaccount' : IDL.Opt(IDL.Vec(IDL.Nat8)),
  });
  const Stats = IDL.Record({
    'biggest_pot_won' : IDL.Nat64,
    'hands_won' : IDL.Nat64,
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
    'online' : IDL.Bool,
  });
  const Channel = IDL.Variant({ 'Lobby' : IDL.Null, 'Table' : IDL.Nat64 });
  const MessageKind = IDL.Variant({
    'System' : IDL.Null,
    'Gift' : IDL.Null,
    'Text' : IDL.Null,
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
  const LeaderboardEntry = IDL.Record({
    'id' : IDL.Principal,
    'username' : IDL.Text,
    'level' : IDL.Nat32,
    'stats' : Stats,
    'avatar' : IDL.Text,
  });
  const Profile = IDL.Record({
    'id' : IDL.Principal,
    'xp' : IDL.Nat64,
    'status' : IDL.Text,
    'friend_requests' : IDL.Vec(IDL.Principal),
    'username' : IDL.Text,
    'balance' : IDL.Nat64,
    'inventory' : IDL.Vec(IDL.Nat32),
    'created_at' : IDL.Nat64,
    'level' : IDL.Nat32,
    'stats' : Stats,
    'card_back' : IDL.Nat32,
    'friends' : IDL.Vec(IDL.Principal),
    'avatar' : IDL.Nat32,
  });
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
  });
  const SeatView = IDL.Record({
    'bet' : IDL.Nat64,
    'in_hand' : IDL.Bool,
    'username' : IDL.Text,
    'player' : IDL.Principal,
    'all_in' : IDL.Bool,
    'cards' : IDL.Vec(IDL.Nat8),
    'gift' : IDL.Opt(IDL.Text),
    'has_cards' : IDL.Bool,
    'seat' : IDL.Nat8,
    'last_action' : IDL.Opt(IDL.Text),
    'level' : IDL.Nat32,
    'stack' : IDL.Nat64,
    'card_back' : IDL.Text,
    'sitting_out' : IDL.Bool,
    'folded' : IDL.Bool,
    'avatar' : IDL.Text,
  });
  const Phase = IDL.Variant({
    'Flop' : IDL.Null,
    'Turn' : IDL.Null,
    'River' : IDL.Null,
    'HandOver' : IDL.Null,
    'Waiting' : IDL.Null,
    'PreFlop' : IDL.Null,
  });
  const TableView = IDL.Record({
    'id' : IDL.Nat64,
    'now' : IDL.Nat64,
    'pot' : IDL.Nat64,
    'my_seat' : IDL.Opt(IDL.Nat8),
    'to_call' : IDL.Nat64,
    'hand_no' : IDL.Nat64,
    'last_result' : IDL.Opt(HandResult),
    'min_raise_to' : IDL.Nat64,
    'seats' : IDL.Vec(IDL.Opt(SeatView)),
    'action_deadline' : IDL.Nat64,
    'to_act' : IDL.Opt(IDL.Nat8),
    'current_bet' : IDL.Nat64,
    'board' : IDL.Vec(IDL.Nat8),
    'phase' : Phase,
    'config' : TableConfig,
    'button' : IDL.Nat8,
    'max_raise_to' : IDL.Nat64,
  });
  const TableSummary = IDL.Record({
    'id' : IDL.Nat64,
    'small_blind' : IDL.Nat64,
    'min_buy_in' : IDL.Nat64,
    'name' : IDL.Text,
    'max_seats' : IDL.Nat8,
    'avg_pot' : IDL.Nat64,
    'players' : IDL.Nat8,
    'big_blind' : IDL.Nat64,
    'phase' : Phase,
    'max_buy_in' : IDL.Nat64,
  });
  const Result_4 = IDL.Variant({ 'Ok' : Profile, 'Err' : IDL.Text });
  return IDL.Service({
    'act' : IDL.Func([IDL.Nat64, Action], [Result], []),
    'admin_add_item' : IDL.Func([StoreItem], [Result_1], []),
    'admin_create_table' : IDL.Func([TableConfig], [Result_2], []),
    'admin_house_stats' : IDL.Func([], [Result_3], ['query']),
    'admin_set_item_available' : IDL.Func([IDL.Nat32, IDL.Bool], [Result], []),
    'admin_withdraw_treasury' : IDL.Func([Account, IDL.Nat64], [Result_2], []),
    'buy_item' : IDL.Func([IDL.Nat32], [Result], []),
    'claim_deposit' : IDL.Func([], [Result_2], []),
    'deposit' : IDL.Func([IDL.Nat64], [Result_2], []),
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
    'get_leaderboard' : IDL.Func([], [IDL.Vec(LeaderboardEntry)], ['query']),
    'get_my_profile' : IDL.Func([], [IDL.Opt(Profile)], ['query']),
    'get_profile' : IDL.Func(
        [IDL.Principal],
        [IDL.Opt(PublicProfile)],
        ['query'],
      ),
    'get_table' : IDL.Func([IDL.Nat64], [IDL.Opt(TableView)], ['query']),
    'join_table' : IDL.Func([IDL.Nat64, IDL.Nat8, IDL.Nat64], [Result], []),
    'leave_table' : IDL.Func([IDL.Nat64], [Result_2], []),
    'list_store' : IDL.Func([], [IDL.Vec(StoreItem)], ['query']),
    'list_tables' : IDL.Func([], [IDL.Vec(TableSummary)], ['query']),
    'ping' : IDL.Func([], [], []),
    'register' : IDL.Func([IDL.Text], [Result_4], []),
    'remove_friend' : IDL.Func([IDL.Principal], [Result], []),
    'respond_friend_request' : IDL.Func(
        [IDL.Principal, IDL.Bool],
        [Result],
        [],
      ),
    'send_chat' : IDL.Func([Channel, IDL.Text], [Result], []),
    'send_dm' : IDL.Func([IDL.Principal, IDL.Text], [Result], []),
    'send_friend_request' : IDL.Func([IDL.Principal], [Result], []),
    'send_gift' : IDL.Func([IDL.Nat64, IDL.Principal, IDL.Nat32], [Result], []),
    'set_status' : IDL.Func([IDL.Text], [Result], []),
    'sit_out' : IDL.Func([IDL.Nat64, IDL.Bool], [Result], []),
    'top_up' : IDL.Func([IDL.Nat64, IDL.Nat64], [Result], []),
    'whoami' : IDL.Func([], [IDL.Principal], ['query']),
    'withdraw' : IDL.Func([Account, IDL.Nat64], [Result_2], []),
  });
};
export const init = ({ IDL }) => {
  const InitArgs = IDL.Record({
    'ledger' : IDL.Opt(IDL.Principal),
    'ledger_fee' : IDL.Opt(IDL.Nat64),
  });
  return [IDL.Opt(InitArgs)];
};
import { useEffect, useRef, useState } from 'react';
import { useApp, usePoll } from '../context';
import { unwrap, type ChatMessage, type FriendInfo, type PublicProfile } from '../ic';
import type { Principal } from '@icp-sdk/core/principal';
import { mergeMessages } from './Chat';
import { NotesPanel } from './Notes';
import { BlockedPanel } from './Safety';
import { Avatar } from '../art/Avatar';

export function Friends() {
  const { backend, profile, run, refreshProfile, go } = useApp();
  const [friends, setFriends] = useState<FriendInfo[]>([]);
  const [requests, setRequests] = useState<PublicProfile[]>([]);
  const [search, setSearch] = useState('');
  const [found, setFound] = useState<PublicProfile | null | undefined>(undefined);
  const [chatWith, setChatWith] = useState<FriendInfo | null>(null);

  usePoll(
    async () => {
      setFriends(await backend.get_friends());
    },
    5000,
    [backend],
  );

  useEffect(() => {
    if (!profile) return;
    Promise.all(profile.friend_requests.map((p) => backend.get_profile(p))).then((r) => setRequests(r.flatMap((x): PublicProfile[] => x)));
  }, [backend, profile]);

  if (!profile) return <div className="panel">Sign in to see your friends.</div>;

  const respond = async (p: Principal, accept: boolean) => {
    await run(async () => unwrap(await backend.respond_friend_request(p, accept)), accept ? 'Friend added' : undefined);
    await refreshProfile();
    setFriends(await backend.get_friends());
  };

  return (
    <div className="page-grid">
      <div className="panel">
        <div className="panel-title">Find players</div>
        <form
          className="row gap"
          onSubmit={async (e) => {
            e.preventDefault();
            setFound((await backend.find_user(search.trim()))[0] ?? null);
          }}
        >
          <input value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Username" />
          <button className="btn">Search</button>
        </form>
        {found === null && <p className="muted">No player with that name.</p>}
        {found && (
          <div className="friend-row">
            <Avatar icon={found.avatar} size="sm" />
            <span className="grow">
              {found.username} <span className="lvl">Lv{found.level}</span>
            </span>
            <button className="btn small" onClick={() => run(async () => unwrap(await backend.send_friend_request(found.id)), 'Request sent')}>
              Add friend
            </button>
          </div>
        )}
      </div>

      {requests.length > 0 && (
        <div className="panel">
          <div className="panel-title">Friend requests</div>
          {requests.map((r) => (
            <div key={r.id.toText()} className="friend-row">
              <Avatar icon={r.avatar} size="sm" />
              <span className="grow">{r.username}</span>
              <button className="btn small" onClick={() => respond(r.id, true)}>
                Accept
              </button>
              <button className="btn small ghost" onClick={() => respond(r.id, false)}>
                Decline
              </button>
            </div>
          ))}
        </div>
      )}

      <div className="panel">
        <div className="panel-title">Friends ({friends.length})</div>
        {friends.length === 0 && <p className="muted">Click a player at a table to add them.</p>}
        {friends.map((f) => (
          <div key={f.id.toText()} className="friend-row">
            <span className={`dot ${f.online ? 'on' : ''}`} />
            <Avatar icon={f.avatar} size="sm" />
            <span className="grow">{f.username}</span>
            {f.tables.length > 0 && (
              <button className="btn small" onClick={() => go({ name: 'table', id: f.tables[0] })}>
                Join table
              </button>
            )}
            <button className="btn small ghost" onClick={() => setChatWith(f)}>
              Message
            </button>
            <button className="btn small ghost" onClick={async () => {
              await run(async () => unwrap(await backend.remove_friend(f.id)));
              setFriends(await backend.get_friends());
              await refreshProfile();
            }}>
              ✕
            </button>
          </div>
        ))}
      </div>

      {chatWith && <DirectMessages friend={chatWith} onClose={() => setChatWith(null)} />}

      <NotesPanel />
      <BlockedPanel />
    </div>
  );
}

function DirectMessages({ friend, onClose }: { friend: FriendInfo; onClose: () => void }) {
  const { backend, run } = useApp();
  const [msgs, setMsgs] = useState<ChatMessage[]>([]);
  const [text, setText] = useState('');
  const last = useRef(0n);

  useEffect(() => {
    last.current = 0n;
    setMsgs([]);
  }, [friend.id]);

  usePoll(
    async () => {
      const fresh = await backend.get_dms(friend.id, last.current);
      if (fresh.length) {
        last.current = fresh[fresh.length - 1].id;
        setMsgs((m) => mergeMessages(m, fresh));
      }
    },
    2000,
    [backend, friend.id.toText()],
  );

  return (
    <div className="panel chat">
      <div className="panel-title row">
        <span>Chat with {friend.username}</span>
        <button className="btn small ghost" onClick={onClose}>
          Close
        </button>
      </div>
      <div className="chat-msgs">
        {msgs.map((m) => (
          <div key={String(m.id)} className="msg">
            <b>{m.username}: </b>
            {m.text}
          </div>
        ))}
      </div>
      <form
        className="chat-input"
        onSubmit={async (e) => {
          e.preventDefault();
          const ok = await run(async () => unwrap(await backend.send_dm(friend.id, text)));
          if (ok !== undefined) setText('');
        }}
      >
        <input value={text} onChange={(e) => setText(e.target.value)} maxLength={280} placeholder="Message…" />
        <button className="btn">Send</button>
      </form>
    </div>
  );
}

import { useRef, useState, useEffect } from 'react';
import { useApp, usePoll } from '../context';
import { unwrap, type Channel, type ChatMessage } from '../ic';

/** Appends new messages, ignoring any already shown (overlapping polls). */
export function mergeMessages(old: ChatMessage[], fresh: ChatMessage[]): ChatMessage[] {
  const seen = new Set(old.map((m) => m.id));
  return [...old, ...fresh.filter((m) => !seen.has(m.id))];
}

const QUICK = ['👍', '😂', '🔥', 'nh', 'gg', 'ty', '😭', '🤔'];

export function Chat({ channel, title }: { channel: Channel; title: string }) {
  const { backend, profile, run } = useApp();
  const [msgs, setMsgs] = useState<ChatMessage[]>([]);
  const [text, setText] = useState('');
  const lastId = useRef(0n);
  const box = useRef<HTMLDivElement>(null);
  const key = 'Lobby' in channel ? 'lobby' : `t${channel.Table}`;

  useEffect(() => {
    lastId.current = 0n;
    setMsgs([]);
  }, [key]);

  usePoll(
    async () => {
      const fresh = await backend.get_chat(channel, lastId.current);
      if (fresh.length) {
        lastId.current = fresh[fresh.length - 1].id;
        setMsgs((m) => mergeMessages(m, fresh).slice(-150));
      }
    },
    2000,
    [backend, key],
  );

  useEffect(() => {
    box.current?.scrollTo({ top: box.current.scrollHeight });
  }, [msgs.length]);

  const send = async (t: string) => {
    if (!t.trim()) return;
    const ok = await run(async () => unwrap(await backend.send_chat(channel, t)));
    if (ok !== undefined) setText('');
  };

  return (
    <div className="chat panel">
      <div className="panel-title">{title}</div>
      <div className="chat-msgs" ref={box}>
        {msgs.length === 0 && <div className="muted">No messages yet. Say hi!</div>}
        {msgs.map((m) => (
          <div key={String(m.id)} className={`msg ${Object.keys(m.kind)[0].toLowerCase()}`}>
            {'Text' in m.kind && <b>{m.username}: </b>}
            <span>{m.text}</span>
          </div>
        ))}
      </div>
      {profile ? (
        <>
          <div className="quick">
            {QUICK.map((q) => (
              <button key={q} className="chip" onClick={() => send(q)}>
                {q}
              </button>
            ))}
          </div>
          <form
            className="chat-input"
            onSubmit={(e) => {
              e.preventDefault();
              send(text);
            }}
          >
            <input value={text} maxLength={280} onChange={(e) => setText(e.target.value)} placeholder="Type a message…" />
            <button className="btn">Send</button>
          </form>
        </>
      ) : (
        <div className="muted small">Sign in to chat</div>
      )}
    </div>
  );
}

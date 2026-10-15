import { useEffect, useState } from 'react';
import { useApp, usePoll } from '../context';
import { unwrap, type NoteColor, type NoteView, type PlayerNote } from '../ic';
import type { Principal } from '@icp-sdk/core/principal';
import { timeAgo } from '../format';

type ColorName = 'Red' | 'Orange' | 'Yellow' | 'Green' | 'Blue' | 'Purple';

/** Suggested meanings; players can use the colors however they like. */
export const NOTE_COLORS: { name: ColorName; hint: string }[] = [
  { name: 'Red', hint: 'Maniac / very aggressive' },
  { name: 'Orange', hint: 'Loose' },
  { name: 'Yellow', hint: 'Unknown / watch' },
  { name: 'Green', hint: 'Fish / calling station' },
  { name: 'Blue', hint: 'Tight / nit' },
  { name: 'Purple', hint: 'Strong regular' },
];

const colorName = (c: [] | [NoteColor]): ColorName | null => (c[0] ? (Object.keys(c[0])[0] as ColorName) : null);

/** Private note about another player: free text plus an optional color label. */
export function NoteEditor({ player, note, onSaved }: { player: Principal; note?: PlayerNote; onSaved?: () => void }) {
  const { backend, run } = useApp();
  const [text, setText] = useState(note?.text ?? '');
  const [color, setColor] = useState<ColorName | null>(note ? colorName(note.color) : null);
  const [saved, setSaved] = useState(true);

  useEffect(() => {
    setText(note?.text ?? '');
    setColor(note ? colorName(note.color) : null);
    setSaved(true);
    // reset only when switching players
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [player.toText()]);

  const save = async (nextText = text, nextColor = color) => {
    const r = await run(async () =>
      unwrap(await backend.set_note(player, nextText, nextColor ? [{ [nextColor]: null } as NoteColor] : [])),
    );
    if (r !== undefined) {
      setSaved(true);
      onSaved?.();
    }
  };

  return (
    <div className="note-editor">
      <div className="row">
        <h4>📝 Your notes</h4>
        <span className="muted small">only you can see these</span>
      </div>
      <div className="note-colors">
        {NOTE_COLORS.map((c) => (
          <button
            key={c.name}
            title={c.hint}
            className={`note-swatch note-${c.name.toLowerCase()} ${color === c.name ? 'active' : ''}`}
            onClick={() => {
              const next = color === c.name ? null : c.name;
              setColor(next);
              save(text, next);
            }}
          />
        ))}
        {color && <span className="muted small">{NOTE_COLORS.find((c) => c.name === color)?.hint}</span>}
      </div>
      <textarea
        value={text}
        maxLength={500}
        rows={3}
        placeholder="e.g. 3-bets light from the button, folds to river pressure…"
        onChange={(e) => {
          setText(e.target.value);
          setSaved(false);
        }}
      />
      <div className="row gap">
        <button className="btn small" disabled={saved} onClick={() => save()}>
          Save note
        </button>
        {(text || color) && (
          <button
            className="btn small ghost"
            onClick={() => {
              setText('');
              setColor(null);
              save('', null);
            }}
          >
            Delete
          </button>
        )}
      </div>
    </div>
  );
}

/** Every note you've written, searchable, with inline editing. */
export function NotesPanel() {
  const { backend } = useApp();
  const [notes, setNotes] = useState<NoteView[]>([]);
  const [filter, setFilter] = useState('');
  const [editing, setEditing] = useState<string | null>(null);
  const load = async () => setNotes(await backend.get_notes());
  usePoll(load, 15000, [backend]);

  const q = filter.trim().toLowerCase();
  const shown = notes.filter((n) => !q || n.username.toLowerCase().includes(q) || n.text.toLowerCase().includes(q));

  return (
    <div className="panel">
      <div className="panel-title">Player notes ({notes.length})</div>
      {notes.length === 0 ? (
        <p className="muted small">Click any player at a table to jot down how they play. Notes are private to you.</p>
      ) : (
        <input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Search notes or players" />
      )}
      {shown.map((n) => {
        const key = n.player.toText();
        const c = colorName(n.color);
        return (
          <div key={key} className="note-row">
            <div className="row gap">
              <span className={`avatar sm ${c ? `note-${c.toLowerCase()}` : ''}`}>{n.avatar}</span>
              <b className="grow">{n.username}</b>
              <span className="muted small">{timeAgo(n.updated_at)}</span>
              <button className="chip" onClick={() => setEditing(editing === key ? null : key)}>
                {editing === key ? 'Done' : 'Edit'}
              </button>
            </div>
            {editing === key ? (
              <NoteEditor player={n.player} note={{ text: n.text, color: n.color, updated_at: n.updated_at }} onSaved={load} />
            ) : (
              n.text && <p className="small note-text">{n.text}</p>
            )}
          </div>
        );
      })}
    </div>
  );
}

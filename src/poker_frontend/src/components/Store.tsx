import { useApp } from '../context';
import { unwrap, type StoreItem } from '../ic';
import { icp } from '../format';

const SECTIONS: { kind: 'Avatar' | 'CardBack' | 'Gift'; title: string; note: string }[] = [
  { kind: 'Avatar', title: 'Avatars', note: 'Show off at the table.' },
  { kind: 'CardBack', title: 'Card backs', note: 'What opponents see when you hold cards.' },
  { kind: 'Gift', title: 'Table gifts', note: 'Click a player at a table to send one. Gifts show on their seat for 10 minutes.' },
];

export function Store() {
  const { backend, items, profile, run, refreshProfile } = useApp();

  const owned = (i: StoreItem) => i.price === 0n || (profile?.inventory.includes(i.id) ?? false);
  const equipped = (i: StoreItem) => profile?.avatar === i.id || profile?.card_back === i.id;

  const buy = async (i: StoreItem) => {
    const r = await run(async () => unwrap(await backend.buy_item(i.id)), `Bought ${i.name}!`);
    if (r !== undefined) await refreshProfile();
  };
  const equip = async (i: StoreItem) => {
    const r = await run(async () => unwrap(await backend.equip(i.id)), `${i.name} equipped`);
    if (r !== undefined) await refreshProfile();
  };

  return (
    <div className="store">
      {profile && (
        <div className="panel row">
          <span>
            Balance: <b>{icp(profile.balance)} ICP</b>
          </span>
        </div>
      )}
      {SECTIONS.map((sec) => (
        <section key={sec.kind} className="panel">
          <div className="panel-title">{sec.title}</div>
          <p className="muted small">{sec.note}</p>
          <div className="item-grid">
            {[...items.values()]
              .filter((i) => sec.kind in i.kind)
              .map((i) => (
                <div key={i.id} className={`item ${equipped(i) ? 'equipped' : ''}`}>
                  {sec.kind === 'CardBack' ? <div className="card back" style={{ background: i.icon }} /> : <div className="item-icon">{i.icon}</div>}
                  <div className="item-name">{i.name}</div>
                  <div className="muted small">{i.description}</div>
                  <div className="price">{i.price === 0n ? 'Free' : `${icp(i.price)} ICP`}</div>
                  {profile && sec.kind !== 'Gift' &&
                    (equipped(i) ? (
                      <span className="tag">Equipped</span>
                    ) : owned(i) ? (
                      <button className="btn small ghost" onClick={() => equip(i)}>
                        Equip
                      </button>
                    ) : (
                      <button className="btn small" onClick={() => buy(i)}>
                        Buy
                      </button>
                    ))}
                </div>
              ))}
          </div>
        </section>
      ))}
    </div>
  );
}

import { useApp } from '../context';
import { unwrap, type StoreItem } from '../ic';
import { CHIPS, ICP, icp, type CurrencyV } from '../format';
import { Avatar } from '../art/Avatar';
import { ItemArt } from '../art/ItemArt';

type Kind = 'Avatar' | 'CardBack' | 'Gift' | 'Throwable';
const SECTIONS: { kind: Kind; title: string; note: string }[] = [
  { kind: 'Avatar', title: 'Avatars', note: 'Show off at the table.' },
  { kind: 'CardBack', title: 'Card backs', note: 'What opponents see when you hold cards.' },
  { kind: 'Gift', title: 'Table gifts', note: 'Click a player at a table to send one. Gifts show on their seat for 10 minutes.' },
  { kind: 'Throwable', title: 'Throwables', note: 'Click a player while you are seated to throw one. Everyone at the table sees it fly.' },
];

function Prices({ item }: { item: StoreItem }) {
  if (item.price === 0n && item.chip_price === 0n) return <div className="price">Free</div>;
  return (
    <div className="price">
      {item.price > 0n && <span>{icp(item.price)} ICP</span>}
      {item.price > 0n && item.chip_price > 0n && <span className="muted"> · </span>}
      {item.chip_price > 0n && <span className="chip-price">{item.chip_price.toLocaleString()} chips</span>}
    </div>
  );
}

export function Store() {
  const { backend, items, profile, run, refreshProfile } = useApp();

  const owned = (i: StoreItem) => (i.price === 0n && i.chip_price === 0n) || (profile?.inventory.includes(i.id) ?? false);
  const equipped = (i: StoreItem) => profile?.avatar === i.id || profile?.card_back === i.id;

  const buy = async (i: StoreItem, currency: CurrencyV) => {
    const r = await run(async () => unwrap(await backend.buy_item(i.id, currency)), `Bought ${i.name}!`);
    if (r !== undefined) await refreshProfile();
  };
  const equip = async (i: StoreItem) => {
    const r = await run(async () => unwrap(await backend.equip(i.id)), `${i.name} equipped`);
    if (r !== undefined) await refreshProfile();
  };

  return (
    <div className="store">
      {profile && (
        <div className="panel row gap">
          <span>
            Balance: <b>{icp(profile.balance)} ICP</b>
          </span>
          <span>
            · <b className="chip-price">{profile.chips.toLocaleString()} chips</b>
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
              .map((i) => {
                const usable = sec.kind === 'Gift' || sec.kind === 'Throwable';
                return (
                  <div key={i.id} className={`item ${equipped(i) ? 'equipped' : ''}`}>
                    {sec.kind === 'CardBack' ? (
                      <div className="card back" style={{ background: i.icon }} />
                    ) : sec.kind === 'Avatar' ? (
                      <Avatar icon={i.icon} size="lg" />
                    ) : (
                      <ItemArt icon={i.icon} className="item-icon" />
                    )}
                    <div className="item-name">{i.name}</div>
                    <div className="muted small">{i.description}</div>
                    <Prices item={i} />
                    {profile &&
                      !usable &&
                      (equipped(i) ? (
                        <span className="tag">Equipped</span>
                      ) : owned(i) ? (
                        <button className="btn small ghost" onClick={() => equip(i)}>
                          Equip
                        </button>
                      ) : (
                        <div className="row gap">
                          {i.price > 0n && (
                            <button className="btn small" onClick={() => buy(i, ICP)}>
                              Buy · ICP
                            </button>
                          )}
                          {i.chip_price > 0n && (
                            <button className="btn small chips-btn" onClick={() => buy(i, CHIPS)}>
                              Buy · chips
                            </button>
                          )}
                        </div>
                      ))}
                  </div>
                );
              })}
          </div>
        </section>
      ))}
    </div>
  );
}

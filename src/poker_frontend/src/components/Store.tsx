import { useState } from 'react';
import { useApp } from '../context';
import { unwrap, type StoreItem } from '../ic';
import { CHIPS, ICP, icp, type CurrencyV } from '../format';
import { Avatar } from '../art/Avatar';
import { ItemArt } from '../art/ItemArt';
import { artFor } from '../art/registry';
import { avatarCards, ownsItem } from '../store';

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

/** One store card: an item, or both versions of a paired avatar with a switch between them. */
function ItemCard({
  versions,
  kind,
  onBuy,
  onEquip,
}: {
  versions: StoreItem[];
  kind: Kind;
  onBuy: (i: StoreItem, currency: CurrencyV) => void;
  onEquip: (i: StoreItem) => void;
}) {
  const { profile } = useApp();
  const equipped = (i: StoreItem) => profile?.avatar === i.id || profile?.card_back === i.id;
  const [shown, setShown] = useState(() => Math.max(0, versions.findIndex(equipped)));
  const i = versions[shown] ?? versions[0];
  const usable = kind === 'Gift' || kind === 'Throwable';
  return (
    <div className={`item ${equipped(i) ? 'equipped' : ''}`}>
      {kind === 'CardBack' ? (
        <div className="card back" style={{ background: i.icon }} />
      ) : kind === 'Avatar' ? (
        <Avatar icon={i.icon} size="lg" />
      ) : (
        <ItemArt icon={i.icon} className="item-icon" />
      )}
      {versions.length > 1 && (
        <div className="look-switch" role="group" aria-label="Version">
          {versions.map((v, n) => {
            const look = artFor(v.icon)?.look;
            return (
              <button
                key={v.id}
                className={n === shown ? 'active' : ''}
                aria-pressed={n === shown}
                title={look ? `${look === 'male' ? 'Male' : 'Female'}: ${v.name}` : v.name}
                onClick={() => setShown(n)}
              >
                {look === 'male' ? '♂' : look === 'female' ? '♀' : v.name}
              </button>
            );
          })}
        </div>
      )}
      <div className="item-name">{i.name}</div>
      <div className="muted small">{i.description}</div>
      <Prices item={i} />
      {versions.length > 1 && !(i.price === 0n && i.chip_price === 0n) && <div className="muted small">Buy once, wear either</div>}
      {profile &&
        !usable &&
        (equipped(i) ? (
          <span className="tag">Equipped</span>
        ) : ownsItem(i, profile.inventory) ? (
          <button className="btn small ghost" onClick={() => onEquip(i)}>
            Equip
          </button>
        ) : (
          <div className="buy-row">
            {i.price > 0n && (
              <button className="btn small" onClick={() => onBuy(i, ICP)}>
                Buy · ICP
              </button>
            )}
            {i.chip_price > 0n && (
              <button className="btn small chips-btn" onClick={() => onBuy(i, CHIPS)}>
                Buy · chips
              </button>
            )}
          </div>
        ))}
    </div>
  );
}

export function Store() {
  const { backend, items, profile, run, refreshProfile } = useApp();

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
      {SECTIONS.map((sec) => {
        const all = [...items.values()].filter((i) => sec.kind in i.kind);
        const cards = sec.kind === 'Avatar' ? avatarCards(all) : all.map((i) => [i]);
        return (
          <section key={sec.kind} className="panel">
            <div className="panel-title">{sec.title}</div>
            <p className="muted small">{sec.note}</p>
            <div className="item-grid">
              {cards.map((versions) => (
                <ItemCard key={versions[0].id} versions={versions} kind={sec.kind} onBuy={buy} onEquip={equip} />
              ))}
            </div>
          </section>
        );
      })}
    </div>
  );
}

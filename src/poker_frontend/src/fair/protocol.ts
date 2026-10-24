// BP-FAIR-v1: TypeScript port of src/poker_backend/src/fair.rs.
// Byte-level spec: docs/provably-fair.md. Must match vectors.json exactly.
import { Principal } from '@icp-sdk/core/principal';

export const SHUFFLE_VERSION = 1;

const DECK_SIZE = 52;
const CARD_ENCODING = 'card=rank*4+suit;rank0=2..rank12=A;suit0=c,1=d,2=h,3=s';
const DEAL_ORDER = 'deal=pop-from-end;burn-before-flop-turn-river';
const COMMIT_DOMAIN = 'BulletsPoker/commit/v1';
const SHUFFLE_SALT = 'BulletsPoker/shuffle/v1';

export class UnsupportedVersion extends Error {
  constructor(version: number) {
    super(`unsupported shuffle version ${version}`);
    this.name = 'UnsupportedVersion';
  }
}

const ascii = (s: string) => new TextEncoder().encode(s);

function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

function u64be(v: bigint): Uint8Array {
  const out = new Uint8Array(8);
  new DataView(out.buffer).setBigUint64(0, v, false);
  return out;
}

function checkVersion(version: number): void {
  if (version !== SHUFFLE_VERSION) throw new UnsupportedVersion(version);
}

/** Length prefix: u32be(len(x)) || x. */
export function lp(x: Uint8Array): Uint8Array {
  const out = new Uint8Array(4 + x.length);
  new DataView(out.buffer).setUint32(0, x.length, false);
  out.set(x, 4);
  return out;
}

/** lp(canister principal bytes) || u64be(table_id) || u64be(hand_no). */
export function handIdBytes(canister: string, tableId: bigint, handNo: bigint): Uint8Array {
  return concat(lp(Principal.fromText(canister).toUint8Array()), u64be(tableId), u64be(handNo));
}

/** u16be(version) || u8(52) || lp(card encoding) || lp(deal order). */
export function paramsBytes(version: number): Uint8Array {
  const head = new Uint8Array(3);
  new DataView(head.buffer).setUint16(0, version, false);
  head[2] = DECK_SIZE;
  return concat(head, lp(ascii(CARD_ENCODING)), lp(ascii(DEAL_ORDER)));
}

// Copy into a fresh ArrayBuffer-backed view so WebCrypto's BufferSource typing is satisfied.
const buf = (b: Uint8Array): Uint8Array<ArrayBuffer> => new Uint8Array(b);

/** SHA256(lp(domain) || lp(seed) || lp(hand_id) || lp(params)). */
export async function commitment(seed: Uint8Array, handId: Uint8Array, version: number): Promise<Uint8Array> {
  checkVersion(version);
  const msg = concat(lp(ascii(COMMIT_DOMAIN)), lp(seed), lp(handId), lp(paramsBytes(version)));
  return new Uint8Array(await crypto.subtle.digest('SHA-256', buf(msg)));
}

/** HKDF-SHA256(ikm = seed, salt = raw ASCII salt, info = lp(hand_id) || lp(params), L = 32). */
export async function shuffleKey(seed: Uint8Array, handId: Uint8Array, version: number): Promise<Uint8Array> {
  checkVersion(version);
  const info = concat(lp(handId), lp(paramsBytes(version)));
  const ikm = await crypto.subtle.importKey('raw', buf(seed), 'HKDF', false, ['deriveBits']);
  const bits = await crypto.subtle.deriveBits(
    { name: 'HKDF', hash: 'SHA-256', salt: buf(ascii(SHUFFLE_SALT)), info: buf(info) },
    ikm,
    256,
  );
  return new Uint8Array(bits);
}

/** One rejection-sampling step: the word mod n, or null when it is in the biased zone. */
function acceptWord(n: number, word: number): number | null {
  if (!Number.isInteger(n) || n < 1) throw new Error('n must be >= 1');
  const limit = 2 ** 32 - (2 ** 32 % n);
  return word < limit ? word % n : null;
}

/**
 * Uniform value in 0..n from a fixed word list: limit = 2^32 - (2^32 mod n);
 * take words until one is below limit, return it mod n.
 */
export function uniformFromWords(n: number, words: readonly number[]): { result: number; consumed: number } {
  for (let i = 0; i < words.length; i++) {
    const r = acceptWord(n, words[i]);
    if (r !== null) return { result: r, consumed: i + 1 };
  }
  throw new Error('ran out of words');
}

/** Block i is HMAC-SHA256(key, u64be(i)), read as big-endian u32 words; leftovers carry over. */
class Stream {
  private block = 0n;
  private buf = new Uint8Array(0);
  private pos = 0;
  private constructor(private readonly key: CryptoKey) {}

  static async create(key: Uint8Array): Promise<Stream> {
    const k = await crypto.subtle.importKey('raw', buf(key), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
    return new Stream(k);
  }

  async nextU32(): Promise<number> {
    if (this.pos >= this.buf.length) {
      this.buf = new Uint8Array(await crypto.subtle.sign('HMAC', this.key, buf(u64be(this.block))));
      this.block += 1n;
      this.pos = 0;
    }
    const w = new DataView(this.buf.buffer).getUint32(this.pos, false);
    this.pos += 4;
    return w;
  }

  async uniform(n: number): Promise<number> {
    // Same rule as uniformFromWords, drawing lazily from the stream.
    for (;;) {
      const r = acceptWord(n, await this.nextU32());
      if (r !== null) return r;
    }
  }
}

/**
 * The deck for one hand: start from [0..51]; for i = 51 down to 1 swap deck[i]
 * with deck[uniform(i + 1)]. Cards are dealt by popping from the end.
 */
export async function shuffleDeck(seed: Uint8Array, handId: Uint8Array, version: number): Promise<number[]> {
  checkVersion(version);
  const stream = await Stream.create(await shuffleKey(seed, handId, version));
  const deck = Array.from({ length: DECK_SIZE }, (_, i) => i);
  for (let i = DECK_SIZE - 1; i >= 1; i--) {
    const j = await stream.uniform(i + 1);
    [deck[i], deck[j]] = [deck[j], deck[i]];
  }
  return deck;
}

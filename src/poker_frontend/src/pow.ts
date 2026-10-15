// Sign-up proof-of-work (see the backend's `signup.rs`): find a nonce so that
// sha256("bp-signup" ‖ principal ‖ nonce as u64 little-endian) starts with
// `difficulty` zero bits. The input always fits in one SHA-256 block, so this
// is a small single-block implementation: much faster than crypto.subtle for
// many tiny hashes.

const K = new Uint32Array([
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
  0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
  0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
  0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);
const W = new Uint32Array(64);

/** First 32 bits of sha256(block), where `block` is an already padded 64-byte block. */
function firstWord(block: Uint8Array): number {
  for (let i = 0; i < 16; i++) W[i] = (block[i * 4] << 24) | (block[i * 4 + 1] << 16) | (block[i * 4 + 2] << 8) | block[i * 4 + 3];
  for (let i = 16; i < 64; i++) {
    const a = W[i - 15], b = W[i - 2];
    const s0 = ((a >>> 7) | (a << 25)) ^ ((a >>> 18) | (a << 14)) ^ (a >>> 3);
    const s1 = ((b >>> 17) | (b << 15)) ^ ((b >>> 19) | (b << 13)) ^ (b >>> 10);
    W[i] = (W[i - 16] + s0 + W[i - 7] + s1) | 0;
  }
  let a = 0x6a09e667, b = 0xbb67ae85, c = 0x3c6ef372, d = 0xa54ff53a, e = 0x510e527f, f = 0x9b05688c, g = 0x1f83d9ab, h = 0x5be0cd19;
  for (let i = 0; i < 64; i++) {
    const S1 = ((e >>> 6) | (e << 26)) ^ ((e >>> 11) | (e << 21)) ^ ((e >>> 25) | (e << 7));
    const t1 = (h + S1 + ((e & f) ^ (~e & g)) + K[i] + W[i]) | 0;
    const S0 = ((a >>> 2) | (a << 30)) ^ ((a >>> 13) | (a << 19)) ^ ((a >>> 22) | (a << 10));
    const t2 = (S0 + ((a & b) ^ (a & c) ^ (b & c))) | 0;
    h = g; g = f; f = e; e = (d + t1) | 0; d = c; c = b; b = a; a = (t1 + t2) | 0;
  }
  return (0x6a09e667 + a) >>> 0;
}

/** Leading zero bits of the first 32 bits (enough: difficulty is at most 22). */
function zeroBits(word: number): number {
  return word === 0 ? 32 : Math.clz32(word);
}

/** Builds the padded block for `prefix ‖ nonce`; returns it and the nonce offset. */
function makeBlock(principal: Uint8Array): [Uint8Array, number] {
  const prefix = new Uint8Array([...new TextEncoder().encode('bp-signup'), ...principal]);
  const len = prefix.length + 8;
  if (len > 55) throw new Error('principal too long');
  const block = new Uint8Array(64);
  block.set(prefix);
  block[len] = 0x80;
  const bits = len * 8;
  block[62] = (bits >>> 8) & 0xff;
  block[63] = bits & 0xff;
  return [block, prefix.length];
}

/** The leading zero bits for one nonce (exported for checking against the backend). */
export function powBits(principal: Uint8Array, nonce: number): number {
  const [block, at] = makeBlock(principal);
  writeNonce(block, at, nonce);
  return zeroBits(firstWord(block));
}

function writeNonce(block: Uint8Array, at: number, nonce: number) {
  let n = nonce;
  for (let i = 0; i < 8; i++) {
    block[at + i] = n % 256;
    n = Math.floor(n / 256);
  }
}

/** Finds a nonce, yielding to the browser regularly so the page stays responsive. */
export async function solvePow(principal: Uint8Array, difficulty: number): Promise<bigint> {
  const [block, at] = makeBlock(principal);
  for (let nonce = 0; ; nonce++) {
    writeNonce(block, at, nonce);
    if (zeroBits(firstWord(block)) >= difficulty) return BigInt(nonce);
    if (nonce % 50_000 === 49_999) await new Promise((r) => setTimeout(r, 0));
  }
}

import { describe, expect, it } from 'vitest';
import { UNSIGNED_LINE, canisterNote } from './replay';

const A = 'ryjl3-tyaaa-aaaaa-aaaba-cai';
const B = 'rrkah-fqaaa-aaaaa-aaaaq-cai';

describe('VerifyEvidence copy', () => {
  it('names the canister the file claims and warns when it is not this app', () => {
    expect(canisterNote(A, A)).toEqual({ text: `The file says it comes from canister ${A}, this app's game canister.`, warn: false });
    const other = canisterNote(A, B);
    expect(other.warn).toBe(true);
    expect(other.text).toContain(A);
    expect(other.text).toContain(`not this app's game canister (${B})`);
  });
  it('does not claim a mismatch when the app canister is unknown', () => {
    const n = canisterNote(A, '');
    expect(n.warn).toBe(true);
    expect(n.text).toMatch(/not known here/);
    expect(n.text).not.toMatch(/not this app's game canister/);
  });
  it('says the file is unsigned', () => {
    expect(UNSIGNED_LINE).toBe('This file is not signed: a passing check shows the file is consistent, not that the game produced it.');
  });
});

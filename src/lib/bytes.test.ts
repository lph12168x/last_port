import { describe, it, expect } from 'vitest';
import {
  bytesToHex,
  bytesToAscii,
  hexToBytes,
  asciiToBytes,
  parseInput,
  formatLineTxt,
  buildLogTxt
} from './bytes';

describe('bytesToHex', () => {
  it('formats bytes as uppercase hex with space separator', () => {
    expect(bytesToHex([0x48, 0x65])).toBe('48 65');
    expect(bytesToHex([0x00, 0xff])).toBe('00 FF');
  });
  it('respects custom separator', () => {
    expect(bytesToHex([0x48, 0x65], '-')).toBe('48-65');
  });
  it('accepts Uint8Array', () => {
    expect(bytesToHex(new Uint8Array([0xde, 0xad]))).toBe('DE AD');
  });
});

describe('bytesToAscii', () => {
  it('renders printable ASCII', () => {
    expect(bytesToAscii([0x48, 0x69])).toBe('Hi');
  });
  it('replaces non-printable with dot by default', () => {
    expect(bytesToAscii([0x48, 0x00, 0x65])).toBe('H.e');
  });
  it('respects custom replacement', () => {
    expect(bytesToAscii([0x00], '·')).toBe('·');
  });
});

describe('hexToBytes', () => {
  it('parses space-separated hex', () => {
    const b = hexToBytes('48 65 6C 6C 6F');
    expect(b).not.toBeNull();
    expect(Array.from(b!)).toEqual([0x48, 0x65, 0x6c, 0x6c, 0x6f]);
  });
  it('parses contiguous hex', () => {
    const b = hexToBytes('48656C6C6F');
    expect(Array.from(b!)).toEqual([0x48, 0x65, 0x6c, 0x6c, 0x6f]);
  });
  it('accepts comma, colon, semicolon, hyphen as separators', () => {
    expect(Array.from(hexToBytes('48,65')!)).toEqual([0x48, 0x65]);
    expect(Array.from(hexToBytes('48:65')!)).toEqual([0x48, 0x65]);
    expect(Array.from(hexToBytes('48;65')!)).toEqual([0x48, 0x65]);
    expect(Array.from(hexToBytes('48-65')!)).toEqual([0x48, 0x65]);
  });
  it('strips 0x prefix', () => {
    expect(Array.from(hexToBytes('0x48,0x65')!)).toEqual([0x48, 0x65]);
  });
  it('returns null on odd-length input', () => {
    expect(hexToBytes('48 6')).toBeNull();
  });
  it('returns null on invalid characters', () => {
    expect(hexToBytes('48 ZZ')).toBeNull();
  });
  it('returns empty array on empty/whitespace input', () => {
    expect(Array.from(hexToBytes('')!)).toEqual([]);
    expect(Array.from(hexToBytes('   ')!)).toEqual([]);
  });
  it('is case-insensitive', () => {
    expect(Array.from(hexToBytes('ab CD')!)).toEqual([0xab, 0xcd]);
  });
});

describe('asciiToBytes', () => {
  it('encodes ASCII string', () => {
    expect(Array.from(asciiToBytes('Hi'))).toEqual([0x48, 0x69]);
  });
  it('encodes UTF-8 multibyte', () => {
    // 你 → 0xE4 0xBD 0xA0
    expect(Array.from(asciiToBytes('你'))).toEqual([0xe4, 0xbd, 0xa0]);
  });
});

describe('parseInput', () => {
  it('parses ascii mode', () => {
    const r = parseInput('Hi', 'ascii');
    expect(r.ok).toBe(true);
    if (r.ok) expect(Array.from(r.bytes)).toEqual([0x48, 0x69]);
  });
  it('parses hex mode', () => {
    const r = parseInput('48 69', 'hex');
    expect(r.ok).toBe(true);
    if (r.ok) expect(Array.from(r.bytes)).toEqual([0x48, 0x69]);
  });
  it('returns error on empty input', () => {
    const r = parseInput('', 'ascii');
    expect(r.ok).toBe(false);
  });
  it('returns error on invalid hex', () => {
    const r = parseInput('XX', 'hex');
    expect(r.ok).toBe(false);
  });
});

describe('formatLineTxt', () => {
  it('formats with timestamp and HEX|ASCII', () => {
    // 2025-01-01 12:34:56.789 UTC, hardcoded by setting timezone or
    // constructing via Date(ts) — test with a local time string match
    const ts = Date.UTC(2025, 0, 1, 12, 34, 56, 789);
    const data = new Uint8Array([0x48, 0x69]);
    // 使用本地时间格式断言 HEX|ASCII 部分 (timestamp 由本地时区决定)
    const out = formatLineTxt(ts, data);
    expect(out).toMatch(/\[.+] 48 69 \| Hi/);
  });
  it('respects hex separator', () => {
    const out = formatLineTxt(0, new Uint8Array([0xde, 0xad]), '-');
    expect(out).toContain('DE-AD');
  });
});

describe('buildLogTxt', () => {
  it('joins multiple lines with newlines', () => {
    const lines = [
      { ts_ms: 0, data: new Uint8Array([0x48]) },
      { ts_ms: 0, data: new Uint8Array([0x69]) }
    ];
    const out = buildLogTxt(lines);
    const arr = out.split('\n');
    // 末尾有换行符,实际 split 行数 = 数据行 + 1 空行
    expect(arr.length).toBe(3);
    expect(arr[0]).toContain('48');
    expect(arr[1]).toContain('69');
    expect(arr[2]).toBe('');
  });
});
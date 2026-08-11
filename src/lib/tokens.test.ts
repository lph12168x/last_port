import { describe, it, expect } from 'vitest';
import { tokenizeLine, type LineToken } from './tokens';

describe('tokenizeLine', () => {
  it('returns empty array for empty input', () => {
    expect(tokenizeLine('')).toEqual([]);
  });

  it('returns one plain token for plain text', () => {
    expect(tokenizeLine('hello world')).toEqual([
      { t: 'hello world', k: 'plain' }
    ]);
  });

  it('splits a /usr/bin/foo path segment', () => {
    const r = tokenizeLine('see /usr/bin/foo please');
    expect(r).toContainEqual({ t: '/usr/bin/foo', k: 'path' });
    expect(r[0]).toEqual({ t: 'see ', k: 'plain' });
    expect(r[2]).toEqual({ t: ' please', k: 'plain' });
  });

  it('recognizes https URL', () => {
    expect(tokenizeLine('see https://example.com/foo')).toContainEqual(
      { t: 'https://example.com/foo', k: 'url' }
    );
  });

  it('recognizes an IPv4 address', () => {
    expect(tokenizeLine('PING 192.168.1.1: 64 bytes')).toContainEqual(
      { t: '192.168.1.1', k: 'ipv4' }
    );
  });

  it('recognizes 0x hex sequences', () => {
    expect(tokenizeLine('addr 0xdeadbeef here')).toContainEqual(
      { t: '0xdeadbeef', k: 'hex' }
    );
  });

  it('handles multiple types in one line', () => {
    const r = tokenizeLine('cat /etc/host 0xabcd1234 from 10.0.0.1');
    const kinds = r.map(t => t.k);
    expect(kinds).toContain('path');
    expect(kinds).toContain('hex');
    expect(kinds).toContain('ipv4');
  });

  // 关键回归: user 报"输 pwd 报 RangeError" 根因.
  // 旧 RE 没 g flag, while ((m = RE.exec(text)) !== null) 死循环
  // push 同一段, out 数组增长到超 2^32-1 抛 RangeError: Invalid array length.
  // 修法: 加 g flag 让 RE.exec 推进 lastIndex.
  it('does not infinite-loop on a line with a path (regression for RangeError)', () => {
    // 设一个超时保险: 实际 tokenizeLine 内部也有 > text.length+1 break 防御,
    // 但主要依赖 g flag 正常推进.
    const r = tokenizeLine('pwd /etc/host:192.168.1.1 0xabcd1234');
    // 期望: 多个 tokens, 不是 1 个被无限 push
    expect(r.length).toBeGreaterThan(0);
    expect(r.length).toBeLessThan(100);
    // 必须 tokenize 出 path / ipv4 / hex
    // r 是 tokenizeLine 动态输出, key 集合从 runtime 构造, 用 Set.
    const seen = new Set<string>(r.map(t => t.k));
    expect(seen.has('path')).toBe(true);
    expect(seen.has('ipv4')).toBe(true);
    expect(seen.has('hex')).toBe(true);
  });

  it('returns one plain token for very long lines (over 8KB)', () => {
    const big = 'x'.repeat(9000);
    const r = tokenizeLine(big);
    expect(r).toEqual([{ t: big, k: 'plain' }]);
  });
});

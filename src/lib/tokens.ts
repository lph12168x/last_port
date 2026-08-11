/**
 * 终端输出行的 token 化, 把 path / url / ipv4 / hex 段标出来, 其它是 plain.
 * TerminalPanel.svelte 用它在 mkLine 时计算, 然后 {#each line.tokens as t}
 * 渲染不同 class span. 抽出来到独立文件让 vitest 测覆盖死循环 / g flag 等.
 */
export type LineToken = { t: string; k: 'path' | 'url' | 'ipv4' | 'hex' | 'plain' };

/** 必须带 g flag — 没有 g 时 RegExp.exec 永远从 0 开始, while ((m = RE.exec(text)) !== null)
 *  死循环 push 同一段直到 out array 超 2^32-1, 抛 RangeError: Invalid array length.
 *  这是 user 报告"输 pwd 报 RangeError"的根因, 修在 (this commit). */
const RE = /(\/(?:[^\s\x00-\x1f\<>:"|?*]+\/)*[^\s\x00-\x1f\<>:"|?]*)|((?:https?|ftp):\/\/[^\s]+)|((?:\d{1,3}\.){3}\d{1,3})|(0x[0-9a-fA-F]{4,})/g;

export function tokenizeLine(text: string): LineToken[] {
  const out: LineToken[] = [];
  if (text.length === 0) return out;
  // 行太长 (>8KB) 跳过 tokenize — 避免极端 dmesg / hex dump 让 UI 卡.
  if (text.length > 8192) return [{ t: text, k: 'plain' }];
  const push = (s: string, k: LineToken['k']) => {
    if (s.length > 0) out.push({ t: s, k });
  };
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = RE.exec(text)) !== null) {
    if (m.index > last) push(text.slice(last, m.index), 'plain');
    if (m[1]) push(m[1], 'path');
    else if (m[2]) push(m[2], 'url');
    else if (m[3]) push(m[3], 'ipv4');
    else if (m[4]) push(m[4], 'hex');
    last = m.index + m[0].length;
    // 防御: 死循环保护. 正常输入下每段 ≤ 几十 char, 任何 > text.length + 1 都是
    // bug, 立刻 break.
    if (last > text.length + 1) break;
  }
  if (last < text.length) push(text.slice(last), 'plain');
  return out;
}

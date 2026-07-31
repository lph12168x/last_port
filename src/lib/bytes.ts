/**
 * 字节 ↔ 字符串工具 (纯函数)
 *
 * HEX 模式: 接受 "48 65 6C 6C 6F" 或 "48656C6C6F" 或 "48,65,6C..." (分隔符灵活)
 * ASCII 模式: UTF-8 编码
 */

/** bytes → "48 65 6C 6C 6F" (HEX 字符串,大写,空格分隔) */
export function bytesToHex(data: Uint8Array | number[], sep = ' '): string {
  const arr = data instanceof Uint8Array ? data : Uint8Array.from(data);
  const parts: string[] = new Array(arr.length);
  for (let i = 0; i < arr.length; i++) {
    parts[i] = arr[i].toString(16).padStart(2, '0').toUpperCase();
  }
  return parts.join(sep);
}

/** bytes → 可打印 ASCII (不可打印用 dot) */
export function bytesToAscii(data: Uint8Array | number[], dot = '.'): string {
  const arr = data instanceof Uint8Array ? data : Uint8Array.from(data);
  let s = '';
  for (let i = 0; i < arr.length; i++) {
    const b = arr[i];
    s += b >= 0x20 && b <= 0x7e ? String.fromCharCode(b) : dot;
  }
  return s;
}

/**
 * 解析 HEX 字符串 → bytes
 * 接受分隔符: 空格 / 逗号 / 分号 / 冒号 / 连字符 / 0x 前缀
 * 长度必须为偶数;失败返回 null
 */
export function hexToBytes(text: string): Uint8Array | null {
  const cleaned = text.replace(/0x/gi, '').replace(/[\s,;:\-]+/g, '');
  if (cleaned.length === 0) return new Uint8Array(0);
  if (cleaned.length % 2 !== 0) return null;
  if (!/^[0-9a-fA-F]+$/.test(cleaned)) return null;
  const out = new Uint8Array(cleaned.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(cleaned.substring(i * 2, i * 2 + 2), 16);
  }
  return out;
}

/** ASCII 字符串 → bytes (UTF-8) */
export function asciiToBytes(text: string): Uint8Array {
  return new TextEncoder().encode(text);
}

/** 格式化单行为 txt 记录 (HEX + ASCII 视图) */
export function formatLineTxt(
  ts_ms: number,
  data: Uint8Array,
  hexSep = ' '
): string {
  const d = new Date(ts_ms);
  const hh = String(d.getHours()).padStart(2, '0');
  const mm = String(d.getMinutes()).padStart(2, '0');
  const ss = String(d.getSeconds()).padStart(2, '0');
  const mss = String(d.getMilliseconds()).padStart(3, '0');
  const ts = `${hh}:${mm}:${ss}.${mss}`;
  return `[${ts}] ${bytesToHex(data, hexSep)} | ${bytesToAscii(data)}`;
}

/** 生成完整导出内容 */
export function buildLogTxt(
  lines: { ts_ms: number; data: Uint8Array }[],
  hexSep = ' '
): string {
  return lines.map((l) => formatLineTxt(l.ts_ms, l.data, hexSep)).join('\n') + '\n';
}

/** 解析输入框内容 → bytes (统一入口,mode 决定 hex/ascii) */
export function parseInput(
  text: string,
  mode: 'ascii' | 'hex'
): { ok: true; bytes: Uint8Array } | { ok: false; error: string } {
  if (!text) return { ok: false, error: 'empty' };
  if (mode === 'hex') {
    const bytes = hexToBytes(text);
    if (!bytes) return { ok: false, error: 'invalid hex' };
    return { ok: true, bytes };
  }
  return { ok: true, bytes: asciiToBytes(text) };
}

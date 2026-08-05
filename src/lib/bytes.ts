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

/** 发送附加换行选项 (无 / CR / LF / CRLF) */
export type LineEnding = 'none' | 'cr' | 'lf' | 'crlf';

/** 换行选项对应的原始字节 */
export function lineEndingBytes(ending: LineEnding): number[] {
  switch (ending) {
    case 'cr':
      return [0x0d];
    case 'lf':
      return [0x0a];
    case 'crlf':
      return [0x0d, 0x0a];
    default:
      return [];
  }
}

/**
 * 在输入字节末尾追加换行, 返回新 Uint8Array.
 * ending 为 'none' 时原样返回 (不复制).
 */
export function appendLineEnding(
  bytes: Uint8Array,
  ending: LineEnding
): Uint8Array {
  const suffix = lineEndingBytes(ending);
  if (suffix.length === 0) return bytes;
  const out = new Uint8Array(bytes.length + suffix.length);
  out.set(bytes, 0);
  out.set(suffix, bytes.length);
  return out;
}
/**
 * ANSI / 控制字符清洗, 把字节流归一化为终端显示文本.
 *
 * 处理:
 *  - CSI (`ESC [ ... 终止符`) — SGR(颜色), 私有模式(`?2004h/l` 等), 光标移动
 *    都被丢弃 (我们用 CSS 控制样式)
 *  - OSC (`ESC ] ... BEL|ESC \\`) — 终端标题查询
 *  - DCS / APC / PM (`ESC P|\_|\\] ... BEL|ST`) — 通常私有
 *  - 跳格 `\b`, 报警 `\a`, 响铃 `\x07` — 丢弃
 *  - `\r` 单独出现 → 当作"回到行首"丢弃, `\r\n` 整体当作单个 `\n` 处理
 *  - `\n` 保留作为换行
 *  - `ESC` + 单字符 (例如 `\x1b 7/M` 等) — 丢弃
 *  - 非法 UTF-8 → 用 0xFFFD 替代 (与 TextDecoder 'utf-8' fatal=false 默认一致)
 */
function stripAnsiForDisplay(s: string): string {
  // CSI / DCS / OSC / APC / PM 等以 ESC [ / ESC P / ESC ] / ESC _ / ESC \ 开头,
  // 由 parameter 字节 (0x20-0x3f) + intermediate 字节 (0x20-0x2f) + 最终 字节 (0x40-0x7e) 构成.
  // 最稳妥: 用正则贪婪匹配直到下一个终止字节 (0x40-0x7e).
  let out = s.replace(/\x1b\][\s\S]*?(?:\x07|\x1b\\)/g, ''); // OSC ... BEL / ST
  out = out.replace(/\x1b[PX^_][\s\S]*?(?:\x07|\x1b\\)/g, ''); // DCS / PM / APC
  out = out.replace(/\x1b\[[\x30-\x3f]*[\x20-\x2f]*[\x40-\x7e]/g, ''); // CSI ... final
  out = out.replace(/\x1b[<=>/][\s\S]*?(?:\x07|\x1b\\)/g, ''); // 私有
  out = out.replace(/\x1b[@-Z\\-_]/g, ''); // ESC + 单字符 (7-bit) 控制符

  // 控制字符: 只保留 \t (0x09) 和 \n (0x0a). 其余 (含 \r, \b, \a, \v, \f) 全部丢弃.
  // 重要: 把 \r\n 归一为 \n, 单独 \r 丢弃 (终端 echo `\r` 通常表示"覆盖同一行", 我们不做整行 in-place edit).
  out = out.replace(/\r\n/g, '\n');
  out = out.replace(/\r/g, '');
  out = out.replace(/[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]/g, '');
  return out;
}

/**
 * bytes → 终端文本 (UTF-8 解码 + ANSI 清洗).
 * 保留 \n 让 CSS white-space: pre-wrap 渲染为换行.
 * 非法 UTF-8 序列替换为 U+FFFD, 不丢弃.
 */
export function bytesToTermText(data: Uint8Array | number[]): string {
  const arr = data instanceof Uint8Array ? data : Uint8Array.from(data);
  const raw = new TextDecoder('utf-8', { fatal: false }).decode(arr);
  return stripAnsiForDisplay(raw);
}

/** 仅做 ANSI 清洗, 供历史数据(已是字符串)等场景用 */
export function stripAnsi(s: string): string {
  return stripAnsiForDisplay(s);
}

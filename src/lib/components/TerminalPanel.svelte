<!--
  TerminalPanel — 一体化终端面板 (合并 ReceivePanel + SendPanel 的功能)
  - 顶部: 收发模式 + 换行 + 暂停 / 清空 / 导出 / 字号 + 时间戳开关
  - 中部: 类终端滚动视图 (RX/TX 混排, MAX_LINES 行上限)
  - 底部: 固定输入行, Enter 发送, ↑↓ 回看历史, Ctrl+C 发 SIGINT, Ctrl+L 清屏
  - 字段: 控制字符归一 (\r → 落, \r\n → \n), ANSI CSI 丢弃 (净化的终端显示)
  - 高亮: 行级 severity (err/warn/ok) 给终端内容加色彩辨识
  - 字号: CSS 变量 --term-font-size 驱动, localStorage 持久化, Ctrl+/Ctrl-/Ctrl+0 调节
-->
<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import {
    isTauri,
    onSerialData,
    writeData,
    pickSavePath,
    saveLog
  } from '$lib/api';
  import {
    bytesToAscii,
    bytesToHex,
    bytesToTermText,
    formatLineTxt,
    parseInput,
    type LineEnding
  } from '$lib/bytes';
  import { tokenizeLine, type LineToken } from '$lib/tokens';
  import { appState, uiBusy } from '$lib/state.svelte';
  import type { SerialChunk } from '$lib/types';

  type SendMode = 'ascii' | 'hex';
  type Direction = 'rx' | 'tx';
  type LineSeverity = 'info' | 'ok' | 'warn' | 'err';
  type LineKind = 'plain' | 'prompt' | 'prompt-root' | 'prompt-user' | 'path' | 'error' | 'ok';

  interface TermLine {
    id: number;
    dir: Direction;
    ts_ms: number;
    data: Uint8Array;
    text: string;
    hex: string;
    ascii: string;
    severity: LineSeverity;
    /** 视觉子类: prompt / path / binary / file / dir, 给 term 里不同 token 上色. */
    kind: LineKind;
    /** tokenize() 切分, 给 Svelte {#each} 渲染不同 class span. */
    tokens: LineToken[];
  }

  interface HistoryItem { mode: SendMode; text: string }

  const MAX_LINES = 5000;
  const MAX_HISTORY = 20;

  let nextId = 1;

  let mode = $state<SendMode>('ascii');
  let lineEnding = $state<LineEnding>('lf');
  let showTs = $state(false);
  // raw 模式: 不等 Enter, 每个字符立即发. 默认 false (cooked, 等 Enter),
  // 普通 Linux shell / 终端编辑 走 cooked 模式 (不抢字符). 仅在 uboot /
  // 实时串口监控 / 跟用户协议握手 时切到 raw.
  let rawMode = $state(false);
  let paused = $state(false);
  let text = $state('');
  let error = $state<string | null>(null);
  let log = $state<TermLine[]>([]);
  let pausedBuffer = $state<SerialChunk[]>([]);
  let recvBytes = $state(0);
  let exportMsg = $state<string | null>(null);
  let userScrolledUp = $state(false);
  let history = $state<HistoryItem[]>([]);
  let historyIndex = $state(-1);

  let periodic = $state(false);
  let intervalMs = $state(1000);
  let periodicTimer: number | null = null;

  // 字体大小: 用 CSS 变量 --term-font-size 驱动, 持久化到 localStorage,
  // 范围 9 ~ 22px, 步进 1. 也响应 Ctrl+ / Ctrl- / Ctrl+0 快捷键.
  const FONT_MIN = 9;
  const FONT_MAX = 22;
  const FONT_DEFAULT = 12.5;
  const FONT_STEP = 1;
  function loadFontSize(): number {
    try {
      const v = localStorage.getItem('last_port.term.fontPx');
      if (v === null) return FONT_DEFAULT;
      const n = parseFloat(v);
      if (Number.isFinite(n) && n >= FONT_MIN && n <= FONT_MAX) return n;
    } catch { /* localStorage 可能不可用 */ }
    return FONT_DEFAULT;
  }
  function saveFontSize(v: number) {
    try { localStorage.setItem('last_port.term.fontPx', String(v)); } catch {}
  }
  let fontPx = $state<number>(loadFontSize());
  $effect(() => {
    document.documentElement.style.setProperty('--term-font-size', `${fontPx}px`);
    saveFontSize(fontPx);
  });
  function bumpFont(delta: number) {
    const next = Math.max(FONT_MIN, Math.min(FONT_MAX, fontPx + delta));
    if (next !== fontPx) fontPx = Math.round(next * 10) / 10;
  }

  let unlisten: (() => void) | null = null;
  let windowKeyCleanup: (() => void) | null = null;
  let containerRef: HTMLDivElement | undefined = $state();
  let inputRef: HTMLInputElement | undefined = $state();

  // 渲染节流: 高频数据下逐事件重渲染会拖垮 WebKit. 累积数据按固定间隔批量刷入 DOM.
  const FLUSH_MS = 100;
  const MAX_PENDING = 2000;
  let pendingChunks: SerialChunk[] = [];
  let flushTimer: number | null = null;
  // 跨 flush 累积未成行的字节 — 字符级 echo 字符 (ls, l, s, I 等) 不再各占一行.
  // 切 \n 时把行渲染; 最后一段无 \n 段留到下个 flush 周期.
  let lineBuf: Uint8Array = new Uint8Array(0);
  let staleFlushTimer: number | null = null;
  function decodeUtf8(b: Uint8Array): string {
    return new TextDecoder('utf-8', { fatal: false }).decode(b);
  }

  function flushPending() {
    if (pendingChunks.length === 0) {
      if (flushTimer !== null) { window.clearInterval(flushTimer); flushTimer = null; }
      return;
    }
    const chunks = pendingChunks;
    pendingChunks = [];
    try {
      let bytes = 0;
      // 合并本轮所有 chunk + 上轮残留 lineBuf
      let carried = decodeUtf8(lineBuf);
      const lines: TermLine[] = [];
      for (let i = 0; i < chunks.length; i++) {
        const c = chunks[i];
        const data = new Uint8Array(c.data);
        bytes += data.length;
        carried += bytesToTermText(data);
      }
      recvBytes += bytes;

      // 按 \n 切. 字符级 echo (单字符无 \n) 会和后续字符合到一行.
      // 行内 \r (cursor reset / tput 应用层) 保留, 行尾 \r + 空白 trim 掉.
      const parts = carried.split('\n');
      const tail = parts.pop() ?? '';
      for (const p of parts) {
        const line = p.replace(/[\r\s]+$/, '');
        if (line.length > 0) {
          lines.push(mkLine('rx', Date.now(), new TextEncoder().encode(line)));
        }
      }
      // 末尾无 \n 段留到下次
      lineBuf = new TextEncoder().encode(tail);

      if (lines.length) appendLines(filterSelfEchoes(lines));

      // 长时间 (500ms) 没新数据, 把 lineBuf 强刷 (避免单字符 echo "l" 永远不显示)
      if (lineBuf.length > 0) {
        if (staleFlushTimer !== null) window.clearTimeout(staleFlushTimer);
        staleFlushTimer = window.setTimeout(() => {
          if (lineBuf.length > 0) {
            const txt = decodeUtf8(lineBuf).replace(/[\r\s]+$/, '');
            if (txt.length > 0) {
              appendLines([mkLine('rx', Date.now(), new TextEncoder().encode(txt))]);
            }
            lineBuf = new Uint8Array(0);
          }
          staleFlushTimer = null;
        }, 500);
      }
    } catch (e) {
      error = `数据解析错误: ${e}`;
    }
  }

  /** 折叠串口 shell 的本地命令回显.
   *
   *  Ubuntu getty 给 bash 的 tty 默认 echo ON, bash 把命令字符流回 master.
   *  我们作为 RX 收到一行, 内容 = 最近 TX 行. 折叠它 (不显示), 留 TX 行.
   *
   *  字符级 raw 模式 (e.preventDefault 阻止 input 加字符) 下, TX 是单字符
   *  写, RX 整行 echo 回来 (直到 \n 切行). 所以 fold 用字符级匹配: RX 行
   *  trim 后 = lastTx 拼接的末段 (例如 lastTx="ls", RX="l" 或 "s" 单独, 或
   *  "ls" 整行, 末 1 / 2 / N 字符匹配). */
  let lastTxChars: string[] = [];
  function filterSelfEchoes(lines: TermLine[]): TermLine[] {
    const out: TermLine[] = [];
    for (const ln of lines) {
      if (ln.dir === 'rx') {
        const trimmed = ln.text.replace(/[\r\n\s]+$/, '');
        if (trimmed) {
          const jt = lastTxChars.join('');
          if (trimmed === jt || jt.endsWith(trimmed)) continue;
        }
      }
      out.push(ln);
    }
    return out;
  }
  function rememberLastTxText(text: string) {
    const t = text.replace(/[\r\n\s]+$/, '');
    // 字符级累积, 限 256 防泄漏
    lastTxChars = (lastTxChars.concat(Array.from(t))).slice(-256);
  }

  // 切到 raw 模式时清掉 text 残留 (cooked 阶段已写字符)
  $effect(() => {
    if (rawMode) {
      text = '';
      lastTxChars = [];
    }
  });

  function scheduleFlush() {
    if (flushTimer !== null) return;
    flushTimer = window.setInterval(flushPending, FLUSH_MS);
  }

  function mkLine(dir: Direction, ts_ms: number, data: Uint8Array): TermLine {
    const text = bytesToTermText(data);
    return {
      id: nextId++,
      dir,
      ts_ms,
      data,
      text,
      hex: bytesToHex(data),
      ascii: bytesToAscii(data),
      severity: classifySeverity(text),
      kind: classifyKind(text, dir),
      tokens: tokenizeLine(text),
    };
  }

  /** 把一行文本切分为 token 数组, 给 term 渲染时给不同 span class.
   *  识别:
   *  - 绝对/相对路径: 以 / 或 ./ 或 ../ 开头, 包含路径分隔符
   *  - url: http:// / https:// / ftp://
   *  - ipv4: 四段数字
   *  - hex 字节序列: 0x[0-9a-f]+ 长 >= 4
   *  其它: 'plain' 类型不特殊上色
   *
  /** 行子类识别 — 给 term text 不同片段上色.
   *  - 'prompt': root@host:~# 或 user@host:~$ (含 # / $ 提示符) — 整行用 accent 青
   *  - 'path':   ls/cd 等输出的路径/文件名段 — 浅蓝
   *  - 'error':  bash 错误行 (command not found / No such file / Permission denied) — 红
   *  - 'ok':     成功行 (如 ping 显示 "1 received" / "bytes from ...") — 绿
   *  - 'plain': 其它
   *
   *  注意 prompt / path / error 是 *整行* 着色, 不做 token 切分 (那样太碎).
   *  token 切分交给 CSS (后续可以加 .path .file span). */
  function classifyKind(text: string, dir: Direction): LineKind {
    const trimmed = text.trimEnd();
    if (dir === 'rx') {
      // 整行 prompt: <user>@<host>:<path> [#$]
      // 例: root@ubuntu2204-arm64:~#   /   user@host:/etc$   /   [root@host /]# (chroot)
      if (/^[a-z_][a-z0-9_-]{0,31}@[a-z0-9._-]+(?::[^\s#$:]*)?[#$%]\s*$/.test(trimmed)) {
        // 必须跟主机/路径字符匹配; 单个 #/$ 太宽 (e.g. # comment) — 用
        // user@host 模式, false positive 极少
        if (/@/.test(trimmed) && /[#$:]\s*$/.test(trimmed)) {
        // 末位是 # 通常 root; $ 通常 user. 但也看 user 字段 (root/ubuntu 等常见).
        // 通用策略: # 是 root, $ 是 user. macOS 默认 user prompt 是 host:~ $.
        if (/[#%]\s*$/.test(trimmed)) return 'prompt-root';
        return 'prompt-user';
      }
      }
      // bash 错误
      if (/^[^:]+:\s*(?:command not found|No such file|Permission denied|Read-only|Is a directory|not a directory|invalid|already exists|not found|cannot|unrecognized|operation not permitted)/.test(trimmed)
          || /:\s*command not found/i.test(trimmed)
          || /segmentation fault/.test(trimmed)) {
        return 'error';
      }
      // 成功
      if (/^(?:\d+ bytes from|\d+ packets? received|OK|Success|already up to date|installed|0 errors)/.test(trimmed)) {
        return 'ok';
      }
    }
    return 'plain';
  }

  /** 行级 severity 分类: 让用户能快速看出错误/警告/成功. */
  function classifySeverity(text: string): LineSeverity {
    const lc = text.toLowerCase();
    if (/\berror\b|\berrno\b|\bfailed\b|\bpermission denied\b|\bnot found\b|\bno such\b|\bsegmentation\b|\bcore dump\b|\binvalid\b|\bcannot\b|\bunable to\b|\bunrecognized\b/.test(lc)) return 'err';
    if (/\bwarning\b|\bwarn\b|\bdeprecated\b|\bcaution\b/.test(lc)) return 'warn';
    if (/\bok\b|\bsuccess(?:ful)?\b|\bconnected\b|\bestablished\b|\bready\b|\bdone\b/.test(lc)) return 'ok';
    return 'info';
  }

  function appendLines(lines: TermLine[]) {
    if (lines.length === 0) return;
    // 修剪头部连续空行: 第一次接入串口时 getty/banner/MOTD 等会在
    // 真正的 prompt 之前送多个空行, 这些不影响内容, 占空间也不好看.
    let start = 0;
    if (log.length === 0) {
      // 第一次写入: 跳过开头连续空行
      while (start < lines.length && lines[start].text.trim() === '') start++;
    } else {
      // 后续追加: 跳 1 个空行 (避免已存空行被压成连体)
      if (lines[start].text.trim() === '') {
        const lastLog = log[log.length - 1];
        if (lastLog && lastLog.text.trim() !== '') start++;
      }
    }
    for (let i = start; i < lines.length; i++) log.push(lines[i]);
    if (log.length > MAX_LINES) {
      log.splice(0, log.length - MAX_LINES);
    }
    if (!userScrolledUp && containerRef) {
      // 下一帧布局完成后再设 scrollTop, scrollHeight 此时稳定, 避免读到旧值错位.
      requestAnimationFrame(() => {
        if (containerRef) containerRef.scrollTop = containerRef.scrollHeight;
      });
    }
  }

  onMount(async () => {
    if (!isTauri) return;
    unlisten = await onSerialData((chunks: SerialChunk[]) => {
      if (paused) {
        if (pausedBuffer.length + chunks.length > MAX_LINES) {
          pausedBuffer = pausedBuffer.slice(pausedBuffer.length + chunks.length - MAX_LINES);
        }
        pausedBuffer = pausedBuffer.concat(chunks);
        return;
      }
      pendingChunks = pendingChunks.concat(chunks);
      if (pendingChunks.length > MAX_PENDING) {
        pendingChunks = pendingChunks.slice(pendingChunks.length - MAX_PENDING);
      }
      scheduleFlush();
    });

    // 全局按键 capture: 任意按键自动 focus 输入框, 模拟 uboot 串口 boot menu
    //   "按任意键进入 shell" 行为. WebView 启动后, 不管焦点在哪, 一按
    //   键就进 input. 但 input 自身已经在编辑时不被抢焦点 (input 默认
    //   行为已经把按键送给 input). 排除特定 key 避免破坏浏览器/devtools
    //   默认快捷键 (F12, Ctrl+L, Ctrl+T, Cmd+R, F5).
    const ignoreWhen = (e: KeyboardEvent) => {
      // 修饰键单独 (Shift/Ctrl/Alt/Meta) 不抢焦点
      if (['Shift', 'Control', 'Alt', 'Meta'].includes(e.key)) return true;
      // 浏览器/devtools 保留
      if (e.key === 'F5' || e.key === 'F12') return true;
      if ((e.ctrlKey || e.metaKey) && ['r', 't', 'w', 'l', 'p', 'n', 'j'].includes(e.key.toLowerCase())) return true;
      return false;
    };
    const onWindowKey = (e: KeyboardEvent) => {
      if (!appState.status.opened) return;
      if (ignoreWhen(e)) return;
      // 已经在 input / textarea / contenteditable 里时不动, 让用户输入字符
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) {
        return;
      }
      // 把 focus 抢到 input
      e.preventDefault();
      tick().then(() => inputRef?.focus());
    };
    windowKeyCleanup = () => window.removeEventListener('keydown', onWindowKey, true);
    window.addEventListener('keydown', onWindowKey, true);
  });

  onDestroy(() => {
    unlisten?.();
    stopPeriodic();
    if (flushTimer !== null) window.clearInterval(flushTimer);
    windowKeyCleanup?.();
  });

  $effect(() => {
    if (appState.status.opened) {
      tick().then(() => inputRef?.focus());
    }
  });

  function onScroll() {
    if (!containerRef) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef;
    userScrolledUp = scrollHeight - scrollTop - clientHeight > 20;
  }

  /** body 鼠标事件: 让用户能正常拖选复制终端内容.
   *
   *  设计: 不在 mousedown 时抢焦点. 浏览器 mousedown 默认会启动 selection,
   *  我们不阻止. mouseup 后再判断: 如果 selection 为空 (纯点击) 才把
   *  焦点给 input; 否则 (有选区) 让用户能直接 Cmd+C 复制.
   *  之前用 onmousedown + e.preventDefault() 阻止了 selection 启动, 这就是
   *  Windows 用户反映"鼠标无法选择"的原因. */
  function onBodyMouseDown(_e: MouseEvent) {
    // 故意不做任何事: 让浏览器开始 selection anchor, 后续 mouseup 再处理.
  }
  function onBodyMouseUp(_e: MouseEvent) {
    const sel = window.getSelection();
    if (sel && !sel.isCollapsed) return;
    inputRef?.focus();
  }

  function jumpToBottom() {
    userScrolledUp = false;
    if (containerRef) containerRef.scrollTop = containerRef.scrollHeight;
  }

  function togglePause() {
    paused = !paused;
    if (!paused && pausedBuffer.length > 0) {
      const chunks = pausedBuffer;
      pausedBuffer = [];
      let bytes = 0;
      const lines: TermLine[] = new Array(chunks.length);
      for (let i = 0; i < chunks.length; i++) {
        const c = chunks[i];
        const data = new Uint8Array(c.data);
        bytes += data.length;
        lines[i] = mkLine('rx', c.ts_ms, data);
      }
      recvBytes += bytes;
      appendLines(lines);
    }
  }

  function clearAll() {
    log = [];
    pausedBuffer = [];
    recvBytes = 0;
    userScrolledUp = false;
    error = null;
    exportMsg = null;
  }

  async function send(clearAfter = true) {
    if (!isTauri || !appState.status.opened || uiBusy.value) return;
    if (!text) return;
    const r = parseInput(text, mode);
    if (!r.ok) {
      error = r.error === 'invalid hex' ? 'HEX 格式无效, 例如: 48 65 6C 6C 6F' : r.error;
      return;
    }
    error = null;
    const bytes = appendLineEnding(r.bytes, lineEnding);
    try {
      const n = await writeData(bytes);
      appState.txBytes += n;
      appState.txFrames += 1;
      appendLines([mkLine('tx', Date.now(), bytes)]);
      rememberLastTxText(text);
      pushHistory(text, mode);
      if (clearAfter) {
        text = '';
        historyIndex = -1;
        tick().then(() => inputRef?.focus());
      }
    } catch (e) { error = String(e); }
  }

  function appendLineEnding(bytes: Uint8Array, ending: LineEnding): Uint8Array {
    const suffix = ending === 'crlf' ? [0x0d, 0x0a]
                  : ending === 'lf' ? [0x0a]
                  : ending === 'cr' ? [0x0d]
                  : [];
    if (suffix.length === 0) return bytes;
    const out = new Uint8Array(bytes.length + suffix.length);
    out.set(bytes, 0);
    out.set(suffix, bytes.length);
    return out;
  }

  function pushHistory(t: string, m: SendMode) {
    const last = history[0];
    if (last && last.text === t && last.mode === m) return;
    history = [{ mode: m, text: t }, ...history].slice(0, MAX_HISTORY);
  }

  function applyHistory(idx: number) {
    const h = history[idx];
    if (!h) return;
    mode = h.mode;
    text = h.text;
    error = null;
  }

  function handleInputKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      // raw 模式: text='' (字符已 preventDefault), Enter 只发 lineEnding 提交
      if (rawMode && appState.status.opened && !uiBusy.value) {
        const bytes = appendLineEnding(new Uint8Array(0), lineEnding);
        writeData(bytes).then((n) => {
          appState.txBytes += n;
          appState.txFrames += 1;
          appendLines([mkLine('tx', Date.now(), bytes)]);
        }).catch((err) => { error = String(err); });
        return;
      }
      // cooked 模式: text 字段含完整行, send 走
      send();
      return;
    }
    // 字符级立即发 — 模拟 uboot / BIOS "任意键立即入串口" 模式.
    // input 字符同时进 text (浏览器默认), 我们额外立即 writeData.
    // 排除纯修饰键 + 已 ctrl/cmd 系列 (避免跟现有 Ctrl+C / Ctrl+L 抢),
    // 字符级立即发 — 仅在 raw 模式 (rawMode=true) 启用.
    //   raw 模式 = 不等 Enter, 每个字符立即发到串口. 用于 uboot / 实时协议监控.
    //   默认 cooked (rawMode=false), 字符进入 input text 字段, Enter 触发 send.
    // 也排除 Backspace/Delete/Arrow/Home/End/Tab (编辑键, 不该发).
    if (rawMode && appState.status.opened && !uiBusy.value) {
      const k = e.key;
      const isModifier = k === 'Shift' || k === 'Control' || k === 'Alt' || k === 'Meta' || k === 'CapsLock' || k === 'NumLock' || k === 'ScrollLock';
      const isEditing = k === 'Backspace' || k === 'Delete' || k === 'ArrowUp' || k === 'ArrowDown' || k === 'ArrowLeft' || k === 'ArrowRight' || k === 'Home' || k === 'End' || k === 'Tab' || k === 'PageUp' || k === 'PageDown' || k === 'Insert' || k === 'Escape';
      if (!isModifier && !isEditing && !e.ctrlKey && !e.metaKey && !e.altKey) {
        if (k.length === 1) {
          e.preventDefault();
          writeData(new TextEncoder().encode(k))
            .then((n) => {
              appState.txBytes += n;
              appState.txFrames += 1;
              appendLines([mkLine('tx', Date.now(), new TextEncoder().encode(k))]);
              rememberLastTxText(k);
            })
            .catch((err) => { error = String(err); });
          return;
        }
      }
    }
    // Ctrl+C: 发 SIGINT (0x03)
    if (e.key === 'c' && (e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      if (!appState.status.opened) return;
      const sig = new Uint8Array([0x03]);
      writeData(sig).then(
        (n) => {
          appState.txBytes += n;
          appState.txFrames += 1;
          appendLines([mkLine('tx', Date.now(), sig)]);
          rememberLastTxText(String.fromCharCode(0x03));
          text = '';
          historyIndex = -1;
          inputRef?.focus();
        },
        (err) => { error = String(err); inputRef?.focus(); }
      );
      return;
    }
    // Ctrl+L: 清屏
    if (e.key === 'l' && (e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      clearAll();
      return;
    }
    // Ctrl+ 字体大小: +/-/0
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey) {
      if (e.key === '=' || e.key === '+') { e.preventDefault(); bumpFont(+FONT_STEP); return; }
      if (e.key === '-' || e.key === '_') { e.preventDefault(); bumpFont(-FONT_STEP); return; }
      if (e.key === '0')                   { e.preventDefault(); fontPx = FONT_DEFAULT; return; }
    }
    if (history.length === 0) return;
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      const idx = historyIndex < 0 ? 0 : Math.min(historyIndex + 1, history.length - 1);
      historyIndex = idx;
      applyHistory(idx);
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      const idx = historyIndex - 1;
      if (idx < 0) {
        historyIndex = -1;
        text = '';
      } else {
        historyIndex = idx;
        applyHistory(idx);
      }
    }
  }

  function handleInputChange() {
    if (historyIndex !== -1) historyIndex = -1;
  }

  function startPeriodic() {
    if (periodicTimer !== null) return;
    periodicTimer = window.setInterval(() => send(false), Math.max(10, intervalMs));
  }
  function stopPeriodic() {
    if (periodicTimer !== null) { window.clearInterval(periodicTimer); periodicTimer = null; }
  }
  function togglePeriodic() {
    periodic = !periodic;
    if (periodic) startPeriodic(); else stopPeriodic();
  }
  function handleIntervalChange(e: Event) {
    const v = Math.max(10, Math.min(60000, Number((e.target as HTMLInputElement).value)));
    intervalMs = v;
    if (periodic) { stopPeriodic(); startPeriodic(); }
  }

  function defaultFilename(): string {
    const d = new Date();
    const pad = (n: number) => String(n).padStart(2, '0');
    return `last_port_${d.getFullYear()}${pad(d.getMonth()+1)}${pad(d.getDate())}_${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}.txt`;
  }

  async function exportLog() {
    if (!isTauri || log.length === 0) return;
    exportMsg = null;
    try {
      const path = await pickSavePath({
        defaultPath: defaultFilename(),
        filters: [{ name: 'Text', extensions: ['txt'] }],
        title: '导出终端日志'
      });
      if (!path) return;
      const content =
        log.map((l) => `${l.dir === 'tx' ? '[TX]' : '[RX]'} ${formatLineTxt(l.ts_ms, l.data)}`)
           .join('\n') + '\n';
      await saveLog(path, content);
      exportMsg = `已导出 ${log.length} 行`;
    } catch (e) { exportMsg = `导出失败: ${e}`; }
  }

  function fmtTs(ts: number): string {
    const d = new Date(ts);
    const pad = (n: number) => String(n).padStart(2, '0');
    return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${String(d.getMilliseconds()).padStart(3, '0')}`;
  }

  const endingLabel = $derived(
    lineEnding === 'none' ? '无换行' : lineEnding.toUpperCase()
  );
</script>

<section class="panel">
  <div class="toolbar">
    <span class="title">终端</span>
    <div class="seg" role="tablist" aria-label="收发模式">
      <button role="tab" class:active={mode === 'ascii'}
        onclick={() => { mode = 'ascii'; error = null; inputRef?.focus(); }}>ASCII</button>
      <button role="tab" class:active={mode === 'hex'}
        onclick={() => { mode = 'hex'; error = null; inputRef?.focus(); }}>HEX</button>
    </div>

    <span class="tool-label">换行</span>
    <div class="seg" aria-label="发送附加换行">
      <button class:active={lineEnding === 'none'} onclick={() => { lineEnding = 'none'; inputRef?.focus(); }} title="不追加">无</button>
      <button class:active={lineEnding === 'cr'}    onclick={() => { lineEnding = 'cr';    inputRef?.focus(); }} title="追加 \r">CR</button>
      <button class:active={lineEnding === 'lf'}    onclick={() => { lineEnding = 'lf';    inputRef?.focus(); }} title="追加 \n">LF</button>
      <button class:active={lineEnding === 'crlf'}  onclick={() => { lineEnding = 'crlf';  inputRef?.focus(); }} title="追加 \r\n (登录开发板 shell 推荐)">CRLF</button>
    </div>

    <button class:on={paused} onclick={togglePause} title="暂停/继续接收显示">
      {paused ? '▶ 继续' : '⏸ 暂停'}
    </button>
    <button onclick={clearAll} title="清空输出区">清空</button>
    <button onclick={exportLog} disabled={log.length === 0} title="导出当前日志为 txt 文件">导出</button>

    <div class="font-ctl" title="字号: Ctrl + / Ctrl - 调节, Ctrl + 0 复位">
      <button onclick={() => bumpFont(-FONT_STEP)} title="缩小 Ctrl-">A-</button>
      <span class="font-val">{fontPx}px</span>
      <button onclick={() => bumpFont(+FONT_STEP)} title="放大 Ctrl+">A+</button>
      <button onclick={() => (fontPx = FONT_DEFAULT)} title="默认字号 Ctrl+0">A0</button>
    </div>

    <label class="ts-toggle" title="是否显示每行时间戳">
      <input type="checkbox" checked={showTs} onchange={() => (showTs = !showTs)} />
      时间戳
    </label>
    <label class="ts-toggle" class:on={rawMode} title="Raw 模式: 不等 Enter, 每个字符立即发到串口 (uboot / 协议监控)">
      <input type="checkbox" checked={rawMode} onchange={() => (rawMode = !rawMode)} />
      原始
    </label>
  </div>

  <div class="subbar">
    <label class="periodic" title="按设定间隔重复发送当前输入">
      <input type="checkbox" checked={periodic} onchange={togglePeriodic} disabled={!appState.status.opened} />
      周期
    </label>
    <input type="number" class="interval" value={intervalMs} onchange={handleIntervalChange}
      min="10" max="60000" step="10" disabled={!appState.status.opened} title="发送间隔 (毫秒)" />
    <span class="ms">ms</span>
    <span class="meta">{log.length}/{MAX_LINES} 行 · 收 {recvBytes} B</span>
    {#if userScrolledUp}
      <button class="jump" onclick={jumpToBottom} title="回到最新输出">↓ 底部</button>
    {/if}
    {#if exportMsg}<span class="export-msg">{exportMsg}</span>{/if}
    {#if error}<span class="err">⚠ {error}</span>{/if}
  </div>

  <div class="body" bind:this={containerRef} onscroll={onScroll} onmousedown={onBodyMouseDown} onmouseup={onBodyMouseUp}>
    {#each log as line (line.id)}
      <div class="line {line.dir} sev-{line.severity} kind-{line.kind}">
        <span class="mark">{line.dir === 'tx' ? '▶' : ''}</span>
        {#if showTs}<span class="ts">{fmtTs(line.ts_ms)}</span>{/if}
        {#if mode === 'hex'}
          <span class="hex">{line.hex}</span>
          <span class="ascii">{line.ascii}</span>
        {:else}
          <span class="term">{#each line.tokens as t}<span class="tk-{t.k}">{t.t}</span>{/each}</span>
        {/if}
      </div>
    {/each}
    {#if log.length === 0 && !appState.status.opened}
      <div class="empty">打开串口后开始收发 · 点击下方输入命令</div>
    {/if}
    <!-- Linux 终端风格: input 行作为 .body 内的最后一行, 不是单独的 bar.
         点击 body 任一处 / 按键 自动 focus 到这个 input (由 onWindowKey + mousedown 处理).
         input 是普通 <input>, 视觉上跟 .line 一致, 用户感觉在输出区里敲. -->
    <div class="line line-input">
      <span class="prompt">{appState.status.opened ? (mode === 'hex' ? 'HEX>' : '$') : '—'}</span>
      <input
        bind:this={inputRef}
        bind:value={text}
        oninput={handleInputChange}
        onkeydown={handleInputKeydown}
        class="term-input"
        placeholder={appState.status.opened
          ? mode === 'hex' ? 'HEX 字节, 如 48 65 6C 6C 6F'
                            : `输入命令, 回车发送 (${endingLabel})`
          : '打开串口后可输入发送'}
        disabled={!appState.status.opened}
        spellcheck="false"
        autocomplete="off"
        aria-label="发送输入框"
        title="Enter 发送 · ↑↓ 回看历史 · Ctrl+C 中断 · Ctrl+L 清屏 · Ctrl +/- 调字号"
      />
    </div>
  </div>

</section>

<style>
  .panel {
    display: flex; flex-direction: column; height: 100%; min-width: 0;
    background: var(--bg);
  }
  .toolbar, .subbar {
    display: flex; align-items: center; gap: 8px; flex-wrap: wrap;
    padding: 6px 12px; background: var(--bg-panel);
    border-bottom: 1px solid var(--border); flex-shrink: 0;
  }
  .subbar { padding: 4px 12px; font-size: 11px; }
  .title {
    font-size: 12px; font-weight: 600; text-transform: uppercase;
    letter-spacing: 0.05em; color: var(--text-dim); margin-right: 4px;
  }
  .tool-label { font-size: 11px; color: var(--text-dim); }
  .seg { display: flex; border: 1px solid var(--border); border-radius: 2px; overflow: hidden; }
  .seg button {
    border: none; border-radius: 0; padding: 2px 9px;
    font-size: 11px; background: transparent; color: var(--text-dim);
  }
  .seg button.active { background: var(--accent-dim); color: var(--text); }
  button.on { border-color: var(--warn); color: var(--warn); }
  .ts-toggle, .periodic { display: flex; align-items: center; gap: 4px; font-size: 11px; color: var(--text-dim); }
  .interval { width: 64px; font-family: var(--font-mono); font-size: 11px; padding: 1px 6px; text-align: right; }
  .ms { font-size: 11px; color: var(--text-dim); }
  .meta { font-family: var(--font-mono); font-size: 11px; color: var(--text-dim); margin-left: auto; }
  .jump { padding: 1px 8px; font-size: 11px; }
  .export-msg { color: var(--accent); font-family: var(--font-mono); font-size: 11px; }
  .err { color: var(--danger); font-family: var(--font-mono); font-size: 11px; }
  .font-ctl {
    display: inline-flex; align-items: center; gap: 2px;
    padding: 1px 4px; border: 1px solid var(--border); border-radius: 2px;
    font-size: 11px; background: transparent;
  }
  .font-ctl button { padding: 1px 6px; font-size: 11px; min-width: 26px; }
  .font-val { font-family: var(--font-mono); font-size: 11px; color: var(--text-dim); padding: 0 4px; min-width: 38px; text-align: center; }

  .body {
    flex: 1; overflow-y: auto; overflow-x: hidden; min-width: 0;
    padding: 4px 0;
    font-family: var(--font-mono); font-size: var(--term-font-size, 12.5px); line-height: 1.5;
    /* overflow-wrap: anywhere 词内可断但只在一行真的放不下时;
       word-break: normal 默认, 在词间断字, 不在词内;
       这样 Image 不会被拆成 I+mage, 同时超长 hex 串仍可折. */
    overflow-wrap: anywhere; word-break: normal;
    user-select: text; cursor: text;
  }
  .line {
    display: flex; gap: 10px; padding: 0 14px;
    min-width: 0;
  }
  .line.rx { color: var(--text); }
  .line.tx { color: var(--text-dim); }
  .line.sev-err { color: var(--danger); }
  .line.sev-err .term { color: var(--danger); font-weight: 600; }
  .line.sev-warn { color: #e2aa53; }
  .line.sev-ok { color: #4ec9b0; }
  /* 行子类上色 (kind 字段):
   *  - prompt 整行 (root@host:~#): accent 青 + bold, 像真 terminal 的 prompt
   *  - error 整行 (bash 错误): danger 红 + 半粗, 跟 sev-err 视觉一致
   *  - ok: 跟 sev-ok 一致
   *  sev-err/error 双重 hit 时 err 颜色覆盖 */
  .line.kind-prompt { color: var(--accent); }
  .line.kind-prompt .term { color: var(--accent); font-weight: 600; }
  /* prompt 进一步区分 root (亮 accent) vs 普通 user (暗 accent), 让 root 突出 */
  .line.kind-prompt-root { color: #6ee7ff; }
  .line.kind-prompt-root .term { color: #6ee7ff; font-weight: 700; }
  .line.kind-prompt-user { color: #5fb3d3; }
  .line.kind-prompt-user .term { color: #5fb3d3; font-weight: 500; }
  .line.kind-error { color: var(--danger); }
  .line.kind-error .term { color: var(--danger); font-weight: 600; }
  .line.kind-ok { color: #4ec9b0; }

  /* token-level 上色: path/url/ipv4/hex 局部段. 不影响整行颜色 (kind-* 优先). */
  .term .tk-path { color: #5fb3d3; }      /* 浅蓝青: 路径/文件名 */
  .term .tk-url { color: #6cb6ff; text-decoration: underline; }  /* URL: 蓝色+下划线 */
  .term .tk-ipv4 { color: #d7a3ff; }     /* 紫: IP 地址 */
  .term .tk-hex { color: #d7a3ff; }      /* 紫: 0xABCD hex 串 */
  .mark { flex-shrink: 0; width: 14px; text-align: center; color: var(--accent); }
  .ts { flex-shrink: 0; color: var(--text-dim); width: 84px; }
  .hex { flex-shrink: 0; min-width: 0; color: var(--info); }
  .ascii { flex-shrink: 0; min-width: 0; color: var(--text-dim); margin-left: 12px; }
  .term { flex: 1 1 auto; min-width: 0; overflow-wrap: anywhere; word-break: normal; }
  .empty {
    padding: 32px 16px; text-align: center; color: var(--text-dim);
    font-family: var(--font-sans); font-size: 13px; user-select: none;
  }

  /* 输入行作为 .body 内最后一行, 跟普通 .line 排版一致, 形成 linux terminal 风格.
     输入框透明无边框, 仅用 caret 提示位置. */
  .line-input {
    display: flex; gap: 10px; padding: 0 14px;
    min-width: 0; align-items: baseline;
  }
  .line-input .prompt { color: var(--accent); flex-shrink: 0; font-weight: 600; min-width: 14px; }
  .line-input:not(.off) .prompt { color: var(--accent); }
  .line-input.off .prompt { color: var(--text-dim); }
  .term-input {
    flex: 1 1 auto; min-width: 0; background: transparent; border: none; outline: none;
    color: var(--text); font-family: var(--font-mono); font-size: var(--term-font-size, 12.5px);
    padding: 1px 2px; margin: 0; caret-color: var(--accent); user-select: text;
    background: transparent; appearance: none;
  }
  .term-input:disabled { color: var(--text-dim); cursor: not-allowed; }
  .term-input::placeholder { color: var(--text-dim); opacity: 0.5; }
  .term-input:focus-visible { outline: none; }
</style>

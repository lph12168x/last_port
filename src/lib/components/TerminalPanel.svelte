<!--
  TerminalPanel — 一体化终端面板 (合并 ReceivePanel + SendPanel 的功能)
  - 顶部: 收发模式 + 换行 + 暂停 / 清空 / 导出 / 时间戳开关
  - 中部: 类终端滚动视图 (RX/TX 混排, MAX_LINES 行上限)
  - 底部: 固定输入行, Enter 发送, ↑↓ 回看历史, Ctrl+C 发 SIGINT, Ctrl+L 清屏
  - 字段: 控制字符归一 (\r → 落, \r\n → \n), ANSI CSI 丢弃 (净化的终端显示)
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
  import { appState, uiBusy } from '$lib/state.svelte';
  import type { SerialChunk } from '$lib/types';

  type SendMode = 'ascii' | 'hex';
  type Direction = 'rx' | 'tx';

  interface TermLine {
    id: number;
    dir: Direction;
    ts_ms: number;
    data: Uint8Array;
    text: string;
    hex: string;
    ascii: string;
  }

  interface HistoryItem { mode: SendMode; text: string }

  const MAX_LINES = 5000;
  const MAX_HISTORY = 20;

  let nextId = 1;

  let mode = $state<SendMode>('ascii');
  let lineEnding = $state<LineEnding>('lf');
  let showTs = $state(false);
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

  let unlisten: (() => void) | null = null;
  let containerRef: HTMLDivElement | undefined = $state();
  let inputRef: HTMLInputElement | undefined = $state();

  // 渲染节流: 高频数据下逐事件重渲染会拖垮 WebKit. 累积数据按固定间隔批量刷入 DOM.
  const FLUSH_MS = 100;
  const MAX_PENDING = 2000;
  let pendingChunks: SerialChunk[] = [];
  let flushTimer: number | null = null;

  function flushPending() {
    if (pendingChunks.length === 0) {
      if (flushTimer !== null) { window.clearInterval(flushTimer); flushTimer = null; }
      return;
    }
    const chunks = pendingChunks;
    pendingChunks = [];
    try {
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
    } catch (e) { error = `数据解析错误: ${e}`; }
  }

  function scheduleFlush() {
    if (flushTimer !== null) return;
    flushTimer = window.setInterval(flushPending, FLUSH_MS);
  }

  function mkLine(dir: Direction, ts_ms: number, data: Uint8Array): TermLine {
    return {
      id: nextId++,
      dir,
      ts_ms,
      data,
      text: bytesToTermText(data),
      hex: bytesToHex(data),
      ascii: bytesToAscii(data)
    };
  }

  function appendLines(lines: TermLine[]) {
    // reassign 数组触发 Svelte 5 $state 响应式更新;
    // push/splice 在 Svelte 5 深度代理下虽可工作, 但 reassign 更明确且避免 keyed-each 边界 bug.
    let next = log.concat(lines);
    if (next.length > MAX_LINES) {
      next = next.slice(next.length - MAX_LINES);
    }
    log = next;
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
  });

  onDestroy(() => {
    unlisten?.();
    stopPeriodic();
    if (flushTimer !== null) window.clearInterval(flushTimer);
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
      send();
      return;
    }
    // Ctrl+C: 发 SIGINT (0x03), 当前未发送输入丢弃, 符合 mobaxterm/Linux 习惯.
    if (e.key === 'c' && (e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      if (!appState.status.opened) return;
      const sig = new Uint8Array([0x03]);
      writeData(sig).then(
        (n) => {
          appState.txBytes += n;
          appState.txFrames += 1;
          appendLines([mkLine('tx', Date.now(), sig)]);
          text = '';
          historyIndex = -1;
          inputRef?.focus();
        },
        (err) => { error = String(err); inputRef?.focus(); }
      );
      return;
    }
    // Ctrl+L: 清屏 (类似 bash 的 clear, 但保留 txBytes/RX 计数, 仅清 log).
    if (e.key === 'l' && (e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      clearAll();
      return;
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

    <label class="ts-toggle" title="是否显示每行时间戳">
      <input type="checkbox" checked={showTs} onchange={() => (showTs = !showTs)} />
      时间戳
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

  <div class="body" bind:this={containerRef} onscroll={onScroll} onclick={() => inputRef?.focus()}>
    {#each log as line (line.id)}
      <div class="line {line.dir}">
        <span class="mark">{line.dir === 'tx' ? '▶' : ''}</span>
        {#if showTs}<span class="ts">{fmtTs(line.ts_ms)}</span>{/if}
        {#if mode === 'hex'}
          <span class="hex">{line.hex}</span>
          <span class="ascii">{line.ascii}</span>
        {:else}
          <span class="term">{line.text}</span>
        {/if}
      </div>
    {/each}
    {#if log.length === 0}
      <div class="empty">打开串口后开始收发 · 点击下方输入命令</div>
    {/if}
  </div>

  <div class="inputline" class:off={!appState.status.opened}>
    <span class="prompt">{appState.status.opened ? (mode === 'hex' ? 'HEX>' : '$') : '—'}</span>
    <input
      bind:this={inputRef}
      bind:value={text}
      oninput={handleInputChange}
      onkeydown={handleInputKeydown}
      placeholder={appState.status.opened
        ? mode === 'hex' ? 'HEX 字节, 如 48 65 6C 6C 6F'
                          : `输入命令, 回车发送 (${endingLabel})`
        : '打开串口后可输入发送'}
      disabled={!appState.status.opened}
      spellcheck="false"
      autocomplete="off"
      aria-label="发送输入框"
      title="Enter 发送 · ↑↓ 回看历史 · Ctrl+C 中断 · Ctrl+L 清屏"
    />
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

  .body {
    flex: 1; overflow-y: auto; overflow-x: hidden; min-width: 0;
    padding: 4px 0;
    font-family: var(--font-mono); font-size: 12.5px; line-height: 1.5;
    /* 关键:
       - overflow-wrap: break-word 在词间折行(空格处)
       - word-break: break-all 允许长 hex/UUID/路径无空格串强制折行
       两者的组合保证 dmesg 等带 [hex] 长行的输出不会横向撑开 grid 把 ConfigPanel 挤掉 */
    overflow-wrap: break-word; word-break: break-all;
    user-select: text; cursor: text;
  }
  .line {
    display: flex; gap: 10px; padding: 0 14px;
    min-width: 0; /* 同上:防止 .line 被 .term/.hex 撑大 */
  }
  .line.rx { color: var(--text); }
  .line.tx { color: var(--text-dim); }
  .mark { flex-shrink: 0; width: 14px; text-align: center; color: var(--accent); }
  .ts { flex-shrink: 0; color: var(--text-dim); width: 84px; }
  .hex { flex-shrink: 0; min-width: 0; color: var(--info); }
  .ascii { flex-shrink: 0; min-width: 0; color: var(--text-dim); margin-left: 12px; }
  .term { flex: 1 1 auto; min-width: 0; overflow-wrap: anywhere; white-space: pre-wrap; }
  .empty {
    padding: 32px 16px; text-align: center; color: var(--text-dim);
    font-family: var(--font-sans); font-size: 13px; user-select: none;
  }

  .inputline {
    display: flex; align-items: center; gap: 4px; padding: 4px 14px;
    font-family: var(--font-mono); font-size: 12.5px;
    flex-shrink: 0; border-top: 1px solid var(--border); background: var(--bg);
  }
  .inputline .prompt { color: var(--accent); flex-shrink: 0; font-weight: 600; }
  .inputline.off .prompt { color: var(--text-dim); }
  .inputline input {
    flex: 1; min-width: 0; background: transparent; border: none; outline: none;
    color: var(--text); font-family: var(--font-mono); font-size: 12.5px;
    padding: 1px 2px; margin: 0; caret-color: var(--accent); user-select: text;
  }
  .inputline.off input { color: var(--text-dim); }
  .inputline input::placeholder { color: var(--text-dim); opacity: 0.5; }
  .inputline input:disabled { cursor: not-allowed; }
  .inputline input:focus-visible { outline: none; }
</style>

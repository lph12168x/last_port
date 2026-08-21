<!--
  TerminalPanel — xterm 终端 (VIM/uboot 可交互)
  - 输出区即输入区：无独立输入框，聚焦即敲，Linux 终端语义
  - 接收：串口字节直通 xterm.write，保留 CSI/OSC，不再 stripAnsi
  - 发送：xterm onData 逐字节直通 writeData，Enter/ESC/BS/Tab/方向键/F键 原生 VT 序列
  - 备用屏、清屏、光标移动由 xterm 处理，VIM 可全屏交互
-->
<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { WebLinksAddon } from '@xterm/addon-web-links';
  import '@xterm/xterm/css/xterm.css';
  import {
    isTauri,
    onSerialData,
    writeData,
    pickSavePath,
    saveLog
  } from '$lib/api';
  import { formatLineTxt } from '$lib/bytes';
  import { appState, uiBusy } from '$lib/state.svelte';
  import type { SerialChunk } from '$lib/types';
  import type { LineEnding } from '$lib/bytes';

  type Direction = 'rx' | 'tx';
  interface TermLine {
    id: number;
    dir: Direction;
    ts_ms: number;
    data: Uint8Array;
  }

  const MAX_LINES = 5000;
  const SCROLLBACK = 5000;

  let lineEnding = $state<LineEnding>('lf');
  let paused = $state(false);
  let error = $state<string | null>(null);
  let log = $state<TermLine[]>([]);
  let pausedBuffer = $state<SerialChunk[]>([]);
  let recvBytes = $state(0);
  let exportMsg = $state<string | null>(null);
  let nextId = 1;

  // 字体：CSS 变量驱动，持久化
  const FONT_MIN = 9;
  const FONT_MAX = 22;
  const FONT_DEFAULT = 13;
  const FONT_STEP = 1;
  function loadFontSize(): number {
    try {
      const v = localStorage.getItem('last_port.term.fontPx');
      if (v === null) return FONT_DEFAULT;
      const n = parseFloat(v);
      if (Number.isFinite(n) && n >= FONT_MIN && n <= FONT_MAX) return n;
    } catch {}
    return FONT_DEFAULT;
  }
  function saveFontSize(v: number) {
    try { localStorage.setItem('last_port.term.fontPx', String(v)); } catch {}
  }
  let fontPx = $state<number>(loadFontSize());

  let term: Terminal | undefined;
  let fitAddon: FitAddon | undefined;
  let containerRef: HTMLDivElement | undefined = $state();
  let unlisten: (() => void) | null = null;
  let windowKeyCleanup: (() => void) | null = null;
  let resizeObs: ResizeObserver | undefined;

  $effect(() => {
    saveFontSize(fontPx);
    if (term) term.options.fontSize = fontPx;
    fitAddon?.fit();
  });

  // 端口打开后自动聚焦终端
  $effect(() => {
    if (appState.status.opened) tick().then(() => term?.focus());
  });

  function bumpFont(delta: number) {
    const next = Math.max(FONT_MIN, Math.min(FONT_MAX, fontPx + delta));
    if (next !== fontPx) fontPx = Math.round(next * 10) / 10;
  }

  function createTerminal(): Terminal {
    const t = new Terminal({
      fontFamily: "ui-monospace, 'SF Mono', Consolas, 'Liberation Mono', monospace",
      fontSize: fontPx,
      lineHeight: 1.2,
      theme: {
        background: '#1e1e1e',
        foreground: '#d4d4d4',
        cursor: '#4ec9b0',
        cursorAccent: '#1e1e1e',
        selectionBackground: '#264f78',
        black: '#000000',
        red: '#f44747',
        green: '#4ec9b0',
        yellow: '#dcdcaa',
        blue: '#569cd6',
        magenta: '#c586c0',
        cyan: '#4ec9b0',
        white: '#d4d4d4',
        brightBlack: '#858585',
        brightWhite: '#ffffff'
      },
      cursorBlink: true,
      cursorStyle: 'block',
      scrollback: SCROLLBACK,
      convertEol: true,
      allowProposedApi: true,
      windowsMode: false
    });
    return t;
  }

  function pushLog(dir: Direction, data: Uint8Array) {
    log.push({ id: nextId++, dir, ts_ms: Date.now(), data });
    if (log.length > MAX_LINES) log.splice(0, log.length - MAX_LINES);
  }

  onMount(async () => {
    if (!containerRef) return;

    term = createTerminal();
    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon((_, url) => window.open(url, '_blank')));

    term.open(containerRef);
    await tick();
    fitAddon.fit();

    resizeObs = new ResizeObserver(() => fitAddon?.fit());
    resizeObs.observe(containerRef);

    term.focus();

    // 发送：onData 原生 VT 序列直通串口
    term.onData(async (data: string) => {
      if (!appState.status.opened || uiBusy.value) return;
      error = null;
      // Ctrl+L 本地清屏拦截：发给远端同时本地清屏由 xterm 自己处理 \x0c
      // 字体快捷键由 attachCustomKeyEventHandler 拦截，此处不处理
      let out: Uint8Array;
      if (data === '\r') {
        // 换行配置
        if (lineEnding === 'crlf') out = new Uint8Array([0x0d, 0x0a]);
        else if (lineEnding === 'cr') out = new Uint8Array([0x0d]);
        else if (lineEnding === 'none') out = new Uint8Array([0x0d]);
        else out = new Uint8Array([0x0a]); // lf default
        // 兼容：xterm 在 convertEol=false 时 Enter 为 \r，需按配置转换
        // 已在 if 中处理，lf 选项发 \n
        if (lineEnding === 'lf' && data === '\r') out = new Uint8Array([0x0a]);
      } else {
        out = new TextEncoder().encode(data);
      }
      try {
        const n = await writeData(out);
        appState.txBytes += n;
        appState.txFrames += 1;
        pushLog('tx', out);
      } catch (e) {
        error = String(e);
      }
    });

    term.attachCustomKeyEventHandler((e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey) {
        if (e.key === '=' || e.key === '+') { e.preventDefault(); bumpFont(+FONT_STEP); return false; }
        if (e.key === '-' || e.key === '_') { e.preventDefault(); bumpFont(-FONT_STEP); return false; }
        if (e.key === '0') { e.preventDefault(); fontPx = FONT_DEFAULT; return false; }
      }
      // 其它键放行给 xterm onData
      return true;
    });

    // 接收：串口 -> xterm，直通 CSI
    if (isTauri) {
      try {
        unlisten = await onSerialData((chunks: SerialChunk[]) => {
          if (!term) return;
          if (paused) {
            if (pausedBuffer.length + chunks.length > MAX_LINES) {
              pausedBuffer = pausedBuffer.slice(pausedBuffer.length + chunks.length - MAX_LINES);
            }
            pausedBuffer = pausedBuffer.concat(chunks);
            return;
          }
          for (const c of chunks) {
            const data = new Uint8Array(c.data);
            recvBytes += data.length;
            pushLog('rx', data);
            term.write(data);
          }
        });
      } catch (e) {
        error = String(e);
      }
    }

    // 全局兜底聚焦：任意键聚焦终端
    const onWindowKey = (e: KeyboardEvent) => {
      if (!appState.status.opened || !term) return;
      if (['Shift','Control','Alt','Meta','CapsLock','NumLock','ScrollLock'].includes(e.key)) return;
      if (['F5','F12'].includes(e.key)) return;
      if ((e.ctrlKey || e.metaKey) && ['r','t','w','p','n','j'].includes(e.key.toLowerCase())) return;
      const ae = document.activeElement;
      if (ae && (ae.tagName === 'INPUT' || ae.tagName === 'TEXTAREA' || (ae as HTMLElement).isContentEditable)) return;
      if (document.activeElement !== term.element) term.focus();
    };
    windowKeyCleanup = () => window.removeEventListener('keydown', onWindowKey, true);
    window.addEventListener('keydown', onWindowKey, true);
  });

  onDestroy(() => {
    unlisten?.();
    fitAddon?.dispose();
    term?.dispose();
    resizeObs?.disconnect();
    windowKeyCleanup?.();
  });

  function clearAll() {
    term?.clear();
    log = [];
    pausedBuffer = [];
    recvBytes = 0;
    error = null;
    exportMsg = null;
    term?.focus();
  }

  function togglePause() {
    paused = !paused;
    if (!paused && pausedBuffer.length > 0) {
      const chunks = pausedBuffer;
      pausedBuffer = [];
      for (const c of chunks) {
        const data = new Uint8Array(c.data);
        recvBytes += data.length;
        pushLog('rx', data);
        term?.write(data);
      }
    }
    term?.focus();
  }

  async function exportLog() {
    if (!isTauri || log.length === 0) return;
    exportMsg = null;
    try {
      const path = await pickSavePath({
        defaultPath: `last_port_${new Date().toISOString().slice(0,19).replace(/[:T]/g,'_')}.txt`,
        filters: [{ name: 'Text', extensions: ['txt'] }],
        title: '导出终端日志'
      });
      if (!path) return;
      const content = log.map(l => `${l.dir === 'tx' ? '[TX]' : '[RX]'} ${formatLineTxt(l.ts_ms, l.data)}`).join('\n') + '\n';
      await saveLog(path, content);
      exportMsg = `已导出 ${log.length} 行`;
    } catch (e) { exportMsg = `导出失败: ${e}`; }
  }

  function handleContainerClick() {
    term?.focus();
  }
</script>

<section class="panel">
  <div class="toolbar">
    <span class="title">终端</span>
    <span class="tool-label">换行</span>
    <div class="seg" aria-label="发送附加换行">
      <button class:active={lineEnding === 'none'} onclick={() => lineEnding = 'none'} title="不追加">无</button>
      <button class:active={lineEnding === 'cr'}   onclick={() => lineEnding = 'cr'}   title="追加 \r">CR</button>
      <button class:active={lineEnding === 'lf'}   onclick={() => lineEnding = 'lf'}   title="追加 \n">LF</button>
      <button class:active={lineEnding === 'crlf'} onclick={() => lineEnding = 'crlf'} title="追加 \r\n">CRLF</button>
    </div>
    <button class:on={paused} onclick={togglePause} title="暂停/继续接收显示">
      {paused ? '▶ 继续' : '⏸ 暂停'}
    </button>
    <button onclick={clearAll} title="清空终端（本地）">清空</button>
    <button onclick={exportLog} disabled={log.length === 0} title="导出当前日志为 txt 文件">导出</button>
    <div class="font-ctl" title="字号: Ctrl + / Ctrl - 调节, Ctrl + 0 复位">
      <button onclick={() => bumpFont(-FONT_STEP)} title="缩小 Ctrl-">A-</button>
      <span class="font-val">{fontPx}px</span>
      <button onclick={() => bumpFont(+FONT_STEP)} title="放大 Ctrl+">A+</button>
      <button onclick={() => (fontPx = FONT_DEFAULT)} title="默认字号 Ctrl+0">A0</button>
    </div>
    <span class="meta">{log.length}/{MAX_LINES} 行 · 收 {recvBytes} B</span>
    {#if error}<span class="err">⚠ {error}</span>{/if}
    {#if exportMsg}<span class="export-msg">{exportMsg}</span>{/if}
  </div>

  <div class="xterm-host" bind:this={containerRef} onclick={handleContainerClick} role="application" aria-label="串口终端，按任意键聚焦输入"></div>

  <div class="hint">
    {#if !appState.status.opened}
      打开串口后直接在终端内输入 — 无独立输入框，回车发送由换行设置决定。VIM/uboot 可直接交互。
    {:else}
      已连接 — 直接输入。{lineEnding === 'lf' ? 'LF' : lineEnding === 'cr' ? 'CR' : lineEnding === 'crlf' ? 'CRLF' : '无换行'} · 方向键/ESC 透传 · Ctrl+C 发 0x03
    {/if}
  </div>
</section>

<style>
  .panel {
    display: flex; flex-direction: column; height: 100%; min-width: 0;
    background: var(--bg);
  }
  .toolbar {
    display: flex; align-items: center; gap: 8px; flex-wrap: wrap;
    padding: 6px 12px; background: var(--bg-panel);
    border-bottom: 1px solid var(--border); flex-shrink: 0;
  }
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
  .font-ctl {
    display: inline-flex; align-items: center; gap: 2px;
    padding: 1px 4px; border: 1px solid var(--border); border-radius: 2px;
    font-size: 11px; background: transparent;
  }
  .font-ctl button { padding: 1px 6px; font-size: 11px; min-width: 26px; }
  .font-val { font-family: var(--font-mono); font-size: 11px; color: var(--text-dim); padding: 0 4px; min-width: 38px; text-align: center; }
  .meta { font-family: var(--font-mono); font-size: 11px; color: var(--text-dim); margin-left: auto; }
  .export-msg { color: var(--accent); font-family: var(--font-mono); font-size: 11px; }
  .err { color: var(--danger); font-family: var(--font-mono); font-size: 11px; }
  .xterm-host {
    flex: 1; min-height: 0; min-width: 0;
    padding: 4px;
    background: var(--bg);
  }
  .xterm-host :global(.xterm) { height: 100%; }
  .xterm-host :global(.xterm-viewport) { overflow-y: auto !important; }
  .hint {
    padding: 4px 12px; font-size: 11px; color: var(--text-dim);
    background: var(--bg-panel); border-top: 1px solid var(--border);
    flex-shrink: 0;
  }
</style>

<!--
  ReceivePanel — 接收区 (C5)
  C6+: 加导出按钮
-->
<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { isTauri, onSerialData, pickSavePath, saveLog } from '$lib/api';
  import { bytesToHex, bytesToAscii, buildLogTxt } from '$lib/bytes';
  import type { SerialChunk } from '$lib/types';

  type ViewMode = 'hex' | 'ascii';
  interface RxLine {
    ts_ms: number;
    data: Uint8Array;
  }

  const MAX_LINES = 5000;

  let viewMode = $state<ViewMode>('hex');
  let paused = $state(false);
  let log = $state<RxLine[]>([]);
  let containerRef: HTMLDivElement | undefined = $state();
  let userScrolledUp = $state(false);
  let unlisten: (() => void) | null = null;
  let recvCount = $state(0);
  let exportMsg = $state<string | null>(null);

  onMount(async () => {
    if (!isTauri) return;
    unlisten = await onSerialData((chunks: SerialChunk[]) => {
      if (paused) return;
      for (const c of chunks) {
        const data = new Uint8Array(c.data);
        for (let i = 0; i < data.length; i += 16) {
          log.push({ ts_ms: c.ts_ms, data: data.slice(i, i + 16) });
        }
        recvCount += c.data.length;
      }
      if (log.length > MAX_LINES) {
        log.splice(0, log.length - MAX_LINES);
      }
      if (!userScrolledUp) {
        tick().then(() => {
          if (containerRef) containerRef.scrollTop = containerRef.scrollHeight;
        });
      }
    });
  });

  onDestroy(() => {
    if (unlisten) unlisten();
  });

  function onScroll() {
    if (!containerRef) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef;
    userScrolledUp = scrollHeight - scrollTop - clientHeight > 20;
  }

  function clearLog() {
    log = [];
    userScrolledUp = false;
  }

  function jumpToBottom() {
    userScrolledUp = false;
    if (containerRef) containerRef.scrollTop = containerRef.scrollHeight;
  }

  function defaultFilename(): string {
    const d = new Date();
    const pad = (n: number) => String(n).padStart(2, '0');
    return `last_port_${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}_${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}.txt`;
  }

  async function exportLog() {
    if (!isTauri || log.length === 0) return;
    exportMsg = null;
    try {
      const path = await pickSavePath({
        defaultPath: defaultFilename(),
        filters: [{ name: 'Text', extensions: ['txt'] }],
        title: '导出接收日志'
      });
      if (!path) return;
      const content = buildLogTxt(log);
      await saveLog(path, content);
      exportMsg = `已导出 ${log.length} 行`;
    } catch (e) {
      exportMsg = `导出失败: ${e}`;
    }
  }

  function formatTime(ts_ms: number): string {
    const d = new Date(ts_ms);
    const hh = String(d.getHours()).padStart(2, '0');
    const mm = String(d.getMinutes()).padStart(2, '0');
    const ss = String(d.getSeconds()).padStart(2, '0');
    const ms = String(d.getMilliseconds()).padStart(3, '0');
    return `${hh}:${mm}:${ss}.${ms}`;
  }
</script>

<section class="panel">
  <div class="header">
    <span class="title">接收</span>
    <span class="meta">{log.length}/{MAX_LINES} 行 · {recvCount} 字节</span>
    <div class="actions">
      <div class="mode-toggle" role="tablist" aria-label="视图">
        <button role="tab" class:active={viewMode === 'hex'} onclick={() => (viewMode = 'hex')}>HEX</button>
        <button role="tab" class:active={viewMode === 'ascii'} onclick={() => (viewMode = 'ascii')}>ASCII</button>
      </div>
      <button onclick={() => (paused = !paused)} class:on={paused}>
        {paused ? '▶ 继续' : '⏸ 暂停'}
      </button>
      {#if userScrolledUp}
        <button class="primary" onclick={jumpToBottom}>↓ 跳到底部</button>
      {/if}
      <button onclick={exportLog} disabled={log.length === 0} title="导出当前接收日志到 txt 文件">💾 导出</button>
      <button onclick={clearLog}>清空</button>
    </div>
  </div>

  <div class="body" bind:this={containerRef} onscroll={onScroll}>
    {#each log as line, i (i)}
      <div class="line">
        <span class="ts">{formatTime(line.ts_ms)}</span>
        {#if viewMode === 'hex'}
          <span class="hex">{bytesToHex(line.data)}</span>
          <span class="ascii">{bytesToAscii(line.data)}</span>
        {:else}
          <span class="ascii-only">{bytesToAscii(line.data, '·')}</span>
        {/if}
      </div>
    {/each}
    {#if log.length === 0}
      <div class="empty">暂无数据 · 打开串口后等待接收</div>
    {/if}
  </div>
</section>

<style>
  .panel { display: flex; flex-direction: column; height: 100%; background: var(--bg); min-width: 0; }
  .header {
    display: flex; align-items: center; gap: 12px; padding: 8px 16px;
    background: var(--bg-panel); border-bottom: 1px solid var(--border); flex-shrink: 0; height: 40px;
  }
  .title { font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-dim); }
  .meta { font-family: var(--font-mono); font-size: 11px; color: var(--text-dim); }
  .actions { margin-left: auto; display: flex; align-items: center; gap: 6px; }
  .mode-toggle { display: flex; border: 1px solid var(--border); border-radius: 2px; overflow: hidden; }
  .mode-toggle button { border: none; border-radius: 0; padding: 2px 10px; font-size: 11px; background: transparent; color: var(--text-dim); }
  .mode-toggle button.active { background: var(--accent-dim); color: var(--text); }
  .actions button.on { border-color: var(--warn); color: var(--warn); }
  .body { flex: 1; overflow: auto; padding: 4px 0; font-family: var(--font-mono); font-size: 12px; line-height: 1.45; }
  .line { display: flex; gap: 16px; padding: 1px 16px; white-space: pre; }
  .line:hover { background: var(--bg-panel); }
  .ts { color: var(--text-dim); flex-shrink: 0; width: 90px; }
  .hex { color: var(--info); flex-shrink: 0; width: 320px; }
  .ascii { color: var(--text); flex: 1; min-width: 0; overflow: hidden; }
  .ascii-only { color: var(--text); }
  .empty { padding: 32px; text-align: center; color: var(--text-dim); font-family: var(--font-sans); font-size: 13px; }
  .export-msg {
    padding: 4px 16px;
    background: rgba(78, 201, 176, 0.1);
    border-bottom: 1px solid var(--accent);
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 11px;
    flex-shrink: 0;
  }
</style>


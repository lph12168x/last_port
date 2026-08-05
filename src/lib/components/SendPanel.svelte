<!--
  SendPanel — 发送区 (C5)
  C5: 完整输入/发送/周期/历史
  C6+: 不变
-->
<script lang="ts">
  import { onDestroy } from 'svelte';
  import { isTauri, writeData } from '$lib/api';
  import { parseInput } from '$lib/bytes';
  import { appState, uiBusy } from '$lib/state.svelte';

  type SendMode = 'ascii' | 'hex';

  let mode = $state<SendMode>('ascii');
  let text = $state('');
  let error = $state<string | null>(null);
  let periodic = $state(false);
  let intervalMs = $state(1000);
  let lastBytes = $state(0);

  // svelte-check 4.7.4 runes 模式 + 非 export $props() 类型推断缺失 (源码 bug)
  // 关键: runes 模式必须用 let destructure 才能 reactive — `const props = ...`
  // 然后 `const x = props.x` 是静态绑定, 不会响应 props 变化.
  let { disabled = false } = $props() as any;

  interface HistoryItem {
    mode: SendMode;
    text: string;
    ts_ms: number;
  }
  let history = $state<HistoryItem[]>([]);
  const MAX_HISTORY = 20;

  let periodicTimer: number | null = null;

  async function send() {
    if (!isTauri || !appState.status.opened || uiBusy.value) return;
    const r = parseInput(text, mode);
    if (!r.ok) {
      error = r.error;
      return;
    }
    error = null;
    try {
      const n = await writeData(r.bytes);
      lastBytes = n;
      appState.txBytes += n;
      appState.txFrames += 1;
      const last = history[0];
      if (!last || last.text !== text || last.mode !== mode) {
        history.unshift({ mode, text, ts_ms: Date.now() });
        if (history.length > MAX_HISTORY) history.length = MAX_HISTORY;
        history = history;
      }
    } catch (e) {
      error = String(e);
    }
  }

  function applyHistoryItem(item: HistoryItem) {
    mode = item.mode;
    text = item.text;
  }

  function startPeriodic() {
    if (periodicTimer !== null) return;
    periodicTimer = window.setInterval(() => { send(); }, Math.max(10, intervalMs));
  }
  function stopPeriodic() {
    if (periodicTimer !== null) {
      window.clearInterval(periodicTimer);
      periodicTimer = null;
    }
  }
  function togglePeriodic() {
    periodic = !periodic;
    if (periodic) startPeriodic();
    else stopPeriodic();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.ctrlKey && e.key === 'Enter') { e.preventDefault(); send(); }
  }
  function handleIntervalChange(e: Event) {
    const v = Math.max(10, Math.min(60000, Number((e.target as HTMLInputElement).value)));
    intervalMs = v;
    if (periodic && periodicTimer !== null) {
      stopPeriodic();
      startPeriodic();
    }
  }

  onDestroy(() => stopPeriodic());
</script>

<section class="panel">
  <div class="header">
    <span class="title">发送</span>
    <div class="mode-toggle" role="tablist" aria-label="发送模式">
      <button role="tab" class:active={mode === 'ascii'}
        onclick={() => { mode = 'ascii'; error = null; }} disabled={!appState.status.opened}>ASCII</button>
      <button role="tab" class:active={mode === 'hex'}
        onclick={() => { mode = 'hex'; error = null; }} disabled={!appState.status.opened}>HEX</button>
    </div>
  </div>

  <div class="body">
    <textarea class="input" bind:value={text} onkeydown={handleKeydown}
      placeholder={mode === 'hex' ? 'HEX 字节, 如: 48 65 6C 6C 6F' : '文本, 如: hello'}
      disabled={!appState.status.opened} spellcheck="false" rows="6"></textarea>

    {#if error}<div class="err">⚠ {error}</div>{/if}

    <div class="row">
      <button class="primary send" onclick={send} disabled={!appState.status.opened || !text}>
        发送 <span class="hint">Ctrl+Enter</span>
      </button>
      <span class="meta">上次 {lastBytes} B</span>
    </div>

    <div class="row periodic">
      <label>
        <input type="checkbox" checked={periodic} onchange={togglePeriodic} disabled={!appState.status.opened} />
        周期发送
      </label>
      <input type="number" class="interval" value={intervalMs} onchange={handleIntervalChange}
        min="10" max="60000" step="10" disabled={!appState.status.opened} />
      <span class="suffix">ms</span>
    </div>

    {#if history.length > 0}
      <div class="history">
        <div class="history-title">历史 (最近 {history.length})</div>
        <div class="history-list">
          {#each history as h, i (i)}
            <button type="button" class="history-item" onclick={() => applyHistoryItem(h)}
              disabled={!appState.status.opened} title={`${h.mode.toUpperCase()}: ${h.text}`}>
              <span class="hist-mode">{h.mode}</span>
              <span class="hist-text">{h.text}</span>
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</section>

<style>
  .panel { display: flex; flex-direction: column; height: 100%;
    background: var(--bg-panel); border-left: 1px solid var(--border); min-width: 0; }
  .header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 8px 16px; background: var(--bg-panel); border-bottom: 1px solid var(--border);
    flex-shrink: 0; height: 40px;
  }
  .title { font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-dim); }
  .mode-toggle { display: flex; border: 1px solid var(--border); border-radius: 2px; overflow: hidden; }
  .mode-toggle button { border: none; border-radius: 0; padding: 4px 12px; font-size: 11px; background: transparent; color: var(--text-dim); }
  .mode-toggle button.active { background: var(--accent-dim); color: var(--text); }
  .mode-toggle button:disabled { opacity: 0.5; }
  .body { flex: 1; display: flex; flex-direction: column; gap: 10px; padding: 12px; overflow: auto; }
  .input { width: 100%; flex-shrink: 0; resize: vertical; font-family: var(--font-mono); font-size: 12px; line-height: 1.5; min-height: 100px; }
  .err { color: var(--danger); font-size: 11px; font-family: var(--font-mono); padding: 6px 8px; background: rgba(244, 135, 113, 0.1); border: 1px solid var(--danger); border-radius: 2px; }
  .row { display: flex; align-items: center; gap: 8px; }
  .send { flex: 1; }
  .send .hint { margin-left: 8px; font-family: var(--font-mono); font-size: 10px; color: var(--text-dim); font-weight: normal; }
  .meta { font-family: var(--font-mono); font-size: 11px; color: var(--text-dim); flex-shrink: 0; }
  .periodic label { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text-dim); }
  .periodic .interval { width: 80px; font-family: var(--font-mono); text-align: right; }
  .periodic .suffix { color: var(--text-dim); font-size: 11px; }
  .history { margin-top: auto; border-top: 1px solid var(--border); padding-top: 8px; }
  .history-title { font-size: 11px; color: var(--text-dim); margin-bottom: 6px; }
  .history-list { display: flex; flex-direction: column; gap: 2px; max-height: 200px; overflow-y: auto; }
  .history-item {
    display: flex; align-items: center; gap: 8px; padding: 4px 8px;
    background: transparent; border: 1px solid transparent; text-align: left;
    font-family: var(--font-mono); font-size: 11px; color: var(--text);
  }
  .history-item:hover:not(:disabled) { background: var(--bg); border-color: var(--border); }
  .hist-mode { color: var(--accent); text-transform: uppercase; flex-shrink: 0; width: 36px; }
  .hist-text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; min-width: 0; }
</style>

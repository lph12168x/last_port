<!--
  App — 顶层布局 + 全局状态 + 业务逻辑 (C4)
  C5+ 填充实时收发 UI、保存日志、错误处理等
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import ConfigPanel from '$lib/components/ConfigPanel.svelte';
  import ReceivePanel from '$lib/components/ReceivePanel.svelte';
  import SendPanel from '$lib/components/SendPanel.svelte';
  import {
    listPorts,
    openPort,
    closePort,
    portStatus,
    onSerialData,
    isTauri
  } from '$lib/api';
  import { appState, uiBusy } from '$lib/state.svelte';
  import type { SerialChunk } from '$lib/types';

  const isTauriRuntime = isTauri;

  let unlisten: (() => void) | null = null;

  async function refreshPorts() {
    if (!isTauriRuntime) return;
    uiBusy.value = true;
    try {
      appState.ports = await listPorts();
    } catch (e) {
      appState.error = String(e);
    } finally {
      uiBusy.value = false;
    }
  }

  async function refreshStatus() {
    if (!isTauriRuntime) return;
    try {
      appState.status = await portStatus();
    } catch (e) {
      appState.error = String(e);
    }
  }

  async function handleOpen(name: string) {
    if (uiBusy.value) return;
    uiBusy.value = true;
    appState.error = null;
    try {
      const info = await openPort(name, { ...appState.config });
      appState.status = { opened: true, port: info };
      appState.rxBytes = 0;
      appState.rxFrames = 0;
      appState.txBytes = 0;
      appState.txFrames = 0;
      appState.lastRxTs = null;
    } catch (e) {
      appState.error = String(e);
    } finally {
      uiBusy.value = false;
    }
  }

  async function handleClose() {
    if (uiBusy.value) return;
    uiBusy.value = true;
    appState.error = null;
    try {
      await closePort();
      appState.status = { opened: false, port: null };
    } catch (e) {
      appState.error = String(e);
    } finally {
      uiBusy.value = false;
    }
  }

  onMount(async () => {
    if (!isTauriRuntime) return;
    await refreshPorts();
    await refreshStatus();
    unlisten = await onSerialData((chunks: SerialChunk[]) => {
      let bytes = 0;
      let last = appState.lastRxTs;
      for (const c of chunks) {
        bytes += c.data.length;
        last = c.ts_ms;
      }
      appState.rxBytes += bytes;
      appState.rxFrames += chunks.length;
      appState.lastRxTs = last;
    });
  });

  onDestroy(() => {
    if (unlisten) unlisten();
  });

  function fmtTime(ts: number | null): string {
    if (ts == null) return '—';
    return new Date(ts).toLocaleTimeString();
  }
</script>

<main class="app">
  <header class="topbar">
    <h1>last_port</h1>
    <span class="version">v0.1.0</span>
    <span class="status" class:open={appState.status.opened}>
      {appState.status.opened
        ? `OPEN · ${appState.status.port?.name ?? ''}`
        : 'CLOSED'}
    </span>
  </header>

  {#if !isTauriRuntime}
    <section class="placeholder">
      <div class="card">
        <h2>非 Tauri 环境</h2>
        <p>请用 <code>npm run tauri:dev</code> 启动以查看完整功能。</p>
      </div>
    </section>
  {:else}
    <div class="layout">
      <ConfigPanel
        ports={appState.ports}
        status={appState.status}
        initialConfig={appState.config}
        busy={uiBusy.value}
        onRefresh={refreshPorts}
        onOpen={handleOpen}
        onClose={handleClose}
        onConfigChange={(cfg: typeof appState.config) => (appState.config = cfg)}
      />
      <ReceivePanel
        rxBytes={appState.rxBytes}
        rxFrames={appState.rxFrames}
        lastRxTs={appState.lastRxTs}
      />
      <SendPanel disabled={!appState.status.opened} />
    </div>

    <footer class="statusbar">
      <span class="stat">RX <strong>{appState.rxBytes}</strong>B / {appState.rxFrames} 帧</span>
      <span class="stat">TX <strong>{appState.txBytes}</strong>B / {appState.txFrames} 帧</span>
      <span class="stat dim">最近 {fmtTime(appState.lastRxTs)}</span>
      {#if appState.error}
        <span class="err">⚠ {appState.error}</span>
      {/if}
    </footer>
  {/if}
</main>

<style>
  .app { display: flex; flex-direction: column; height: 100%; }
  .topbar {
    display: flex; align-items: center; gap: 12px; padding: 8px 16px;
    background: var(--bg-panel); border-bottom: 1px solid var(--border);
    flex-shrink: 0; height: 40px;
  }
  h1 { font-size: 14px; font-weight: 600; }
  .version { font-size: 11px; color: var(--text-dim); font-family: var(--font-mono); }
  .status {
    margin-left: auto; font-size: 11px; font-family: var(--font-mono);
    padding: 2px 10px; border: 1px solid var(--warn); color: var(--warn); border-radius: 2px;
  }
  .status.open { border-color: var(--accent); color: var(--accent); }
  .placeholder { flex: 1; display: flex; align-items: center; justify-content: center; }
  .card { background: var(--bg-panel); border: 1px solid var(--border); padding: 24px; border-radius: 4px; }
  .layout {
    flex: 1;
    display: grid;
    grid-template-columns: 280px 1fr 320px;
    min-height: 0;
  }
  .statusbar {
    display: flex; align-items: center; gap: 16px; padding: 4px 16px;
    background: var(--bg-panel); border-top: 1px solid var(--border);
    font-size: 11px; font-family: var(--font-mono); height: 28px; flex-shrink: 0;
  }
  .stat strong { color: var(--accent); margin-right: 2px; }
  .stat.dim { color: var(--text-dim); }
  .err { margin-left: auto; color: var(--danger); }
</style>
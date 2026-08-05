<!--
  App — 顶层布局 + 全局状态 + 业务逻辑
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import ConfigPanel from '$lib/components/ConfigPanel.svelte';
  import TerminalPanel from '$lib/components/TerminalPanel.svelte';
  import {
    listPorts,
    openPort,
    closePort,
    portStatus,
    onSerialData,
    onPortsUpdate,
    onSerialError,
    waitForTauri,
    installFrontendLogBridge,
    isTauri
  } from '$lib/api';
  import { appState, uiBusy } from '$lib/state.svelte';
  import type { SerialChunk } from '$lib/types';

  // 仍保留 isTauri 检测, 仅用于 UI 切换 (浏览器/Tauri 模式).
  // 所有 invoke 调用现在不再被这个 guard 阻塞 — invoke 失败会 throw
  // 由 try/catch 捕获, 没有静默失败.
  const isTauriRuntime = isTauri;

  let unlistenData: (() => void) | null = null;
  let unlistenPorts: (() => void) | null = null;
  let unlistenErr: (() => void) | null = null;

  // 立刻装 console.log 桥, 这样 onMount 之前的所有 console 调用也能被记录
  installFrontendLogBridge();
  console.log('[App] script start, isTauri=' + isTauriRuntime);

  async function refreshPorts() {
    if (!isTauriRuntime) return;
    uiBusy.value = true;
    appState.error = null;
    console.log('[refreshPorts] invoking list_ports...');
    try {
      const ports = await listPorts();
      console.log('[refreshPorts] result:', ports.length, 'port(s)');
      appState.ports = ports;
      if (ports.length === 0) {
        appState.error = '未检测到串口 (Rust 端 list_ports 返回空数组 — 看桌面 last_port.log)';
      }
    } catch (e) {
      console.error('[refreshPorts] FAILED:', e);
      appState.error = `刷新失败: ${String(e)}`;
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
    if (!isTauriRuntime) {
      console.log('[App] not in Tauri runtime, skipping backend subscriptions');
      return;
    }

    // 0. 等待 Tauri IPC bridge 注入 (webview 启动早期 JS 跑得比
    // `__TAURI_INTERNALS__` 注入快, invoke/listen 此时会 throw).
    const ready = await waitForTauri(5000);
    if (!ready) {
      console.error('[App] Tauri IPC bridge did not inject within 5s');
      appState.error = 'Tauri 注入超时 (webview ↔ Rust 通道未建立)';
      return;
    }
    console.log('[App] Tauri IPC bridge ready');

    // 1. 订阅后端 emit 的端口列表 (push 模式)
    try {
      unlistenPorts = await onPortsUpdate((ports) => {
        console.log('[push] serial:ports received, count:', ports.length);
        appState.ports = ports;
      });
      console.log('[App] subscribed to serial:ports push');
    } catch (e) {
      console.error('[App] failed to subscribe serial:ports:', e);
    }

    // 2. 订阅后端 emit 的错误
    try {
      unlistenErr = await onSerialError((msg) => {
        console.error('[push] serial:error:', msg);
        appState.error = msg;
      });
    } catch (e) {
      console.error('[App] failed to subscribe serial:error:', e);
    }

    // 3. 主动拉一次 (确保拿到当前状态, 防止错过 emit)
    try {
      await Promise.all([refreshPorts(), refreshStatus()]);
    } catch (e) {
      console.error('[App] initial refresh failed:', e);
    }

    // 4. 订阅串口数据
    try {
      unlistenData = await onSerialData((chunks: SerialChunk[]) => {
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
      console.log('[App] subscribed to serial:data push');
    } catch (e) {
      console.error('[App] failed to subscribe serial:data:', e);
    }
  });

  onDestroy(() => {
    unlistenData?.();
    unlistenPorts?.();
    unlistenErr?.();
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
      <TerminalPanel />
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
    grid-template-columns: 280px minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    min-height: 0;
    min-width: 0; /* Grid item 容易被滚出视野的长内容撑出 */
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
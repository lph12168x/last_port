<!--
  ConfigPanel — 左侧配置面板 (C4)
  C5+: 引入 bytes 工具、HEX/ASCII 双视图、实时收发
-->
<script lang="ts">
  import {
    BAUD_RATES,
    type DataBits,
    type FlowControl,
    type Parity,
    type PortConfig,
    type PortInfo,
    type PortStatus,
    type StopBits
  } from '$lib/types';

  // svelte-check 4.7.4 不完整支持 $props() 类型推断 (src-tauri/src/serial/session.rs bug),
  // 用 any 跳过,运行时由 Svelte 编译器保证正确
  const props = $props() as any;
  const ports: PortInfo[] = props.ports ?? [];
  const status: PortStatus = props.status ?? { opened: false, port: null };
  const busy: boolean = props.busy ?? false;
  const onRefresh: () => void = props.onRefresh ?? (() => {});
  const onOpen: (name: string) => void = props.onOpen ?? (() => {});
  const onClose: () => void = props.onClose ?? (() => {});
  const onConfigChange: (cfg: PortConfig) => void = props.onConfigChange ?? (() => {});

  // 本地维护 config (C4 用单向数据流: 内部修改时通过 onConfigChange 上抛)
  let config = $state<PortConfig>({ ...(props.initialConfig as PortConfig) });
  let selectedName = $state('');

  // 外部 initialConfig 变化时同步
  $effect(() => {
    const ic = props.initialConfig as PortConfig;
    if (
      ic.baud_rate !== config.baud_rate ||
      ic.data_bits !== config.data_bits ||
      ic.stop_bits !== config.stop_bits ||
      ic.parity !== config.parity ||
      ic.flow_control !== config.flow_control
    ) {
      config = { ...ic };
    }
  });

  // 选中端口变化: 优先当前打开的端口,否则第一个
  $effect(() => {
    if (status.opened && status.port) {
      selectedName = status.port.name;
    } else if (!selectedName && ports.length > 0) {
      selectedName = ports[0].name;
    } else if (!ports.find((p: PortInfo) => p.name === selectedName)) {
      selectedName = ports[0]?.name ?? '';
    }
  });

  function notify() {
    onConfigChange({ ...config });
  }

  function handleBaudChange(e: Event) {
    config = { ...config, baud_rate: Number((e.target as HTMLSelectElement).value) };
    notify();
  }
  function handleDataBits(e: Event) {
    config = { ...config, data_bits: Number((e.target as HTMLSelectElement).value) as DataBits };
    notify();
  }
  function handleStopBits(e: Event) {
    config = { ...config, stop_bits: Number((e.target as HTMLSelectElement).value) as StopBits };
    notify();
  }
  function handleParity(e: Event) {
    config = { ...config, parity: (e.target as HTMLSelectElement).value as Parity };
    notify();
  }
  function handleFlow(e: Event) {
    config = { ...config, flow_control: (e.target as HTMLSelectElement).value as FlowControl };
    notify();
  }

  function handleOpenClick() {
    if (selectedName) onOpen(selectedName);
  }
</script>

<aside class="panel">
  <div class="section">
    <div class="section-title">端口</div>
    <div class="row">
      <select bind:value={selectedName} disabled={status.opened || busy} aria-label="选择端口">
        {#if ports.length === 0}
          <option value="">(未发现串口)</option>
        {:else}
          {#each ports as p (p.name)}
            <option value={p.name}>{p.name} — {p.label}</option>
          {/each}
        {/if}
      </select>
      <button onclick={onRefresh} disabled={busy || status.opened} title="刷新列表">↻</button>
    </div>
  </div>

  <div class="section">
    <div class="section-title">波特率</div>
    <select value={config.baud_rate} onchange={handleBaudChange} disabled={status.opened || busy} aria-label="波特率">
      {#each BAUD_RATES as b (b)}
        <option value={b}>{b}</option>
      {/each}
    </select>
  </div>

  <div class="section two-col">
    <div>
      <div class="section-title">数据位</div>
      <select value={config.data_bits} onchange={handleDataBits} disabled={status.opened || busy} aria-label="数据位">
        <option value={5}>5</option>
        <option value={6}>6</option>
        <option value={7}>7</option>
        <option value={8}>8</option>
      </select>
    </div>
    <div>
      <div class="section-title">停止位</div>
      <select value={config.stop_bits} onchange={handleStopBits} disabled={status.opened || busy} aria-label="停止位">
        <option value={1}>1</option>
        <option value={2}>2</option>
      </select>
    </div>
  </div>

  <div class="section">
    <div class="section-title">校验位</div>
    <select value={config.parity} onchange={handleParity} disabled={status.opened || busy} aria-label="校验位">
      <option value="none">None</option>
      <option value="odd">Odd</option>
      <option value="even">Even</option>
    </select>
  </div>

  <div class="section">
    <div class="section-title">流控</div>
    <select value={config.flow_control} onchange={handleFlow} disabled={status.opened || busy} aria-label="流控">
      <option value="none">None</option>
      <option value="software">XON/XOFF</option>
      <option value="hardware">RTS/CTS</option>
    </select>
  </div>

  <div class="section actions">
    {#if status.opened}
      <button class="danger block" onclick={onClose} disabled={busy}>关闭</button>
    {:else}
      <button class="primary block" onclick={handleOpenClick} disabled={busy || !selectedName}>打开</button>
    {/if}
  </div>
</aside>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px;
    background: var(--bg-panel);
    border-right: 1px solid var(--border);
    overflow-y: auto;
  }
  .section { display: flex; flex-direction: column; gap: 6px; }
  .section.two-col { flex-direction: row; gap: 8px; }
  .section.two-col > div { flex: 1; display: flex; flex-direction: column; gap: 6px; }
  .section-title {
    font-size: 11px; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.05em;
  }
  .row { display: flex; gap: 6px; }
  .row select { flex: 1; }
  select { width: 100%; }
  .actions { margin-top: 8px; }
  .block { width: 100%; padding: 8px 12px; font-weight: 500; }
</style>
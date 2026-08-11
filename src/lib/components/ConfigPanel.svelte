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

  // runes 模式必须用 *let destructure* 才能让 props 反应式更新 —
  // `const props = ...` 然后 `const ports = props.ports` 是静态绑定,
  // 不会重新求值 (这就是 C4 之后端口列表始终为空的根因).
  interface Props {
    ports: PortInfo[];
    status: PortStatus;
    busy: boolean;
    onRefresh: () => void;
    onOpen: (name: string) => void;
    onClose: () => void;
    onConfigChange: (cfg: PortConfig) => void;
    initialConfig: PortConfig;
  }
  let {
    ports = [] as PortInfo[],
    status = { opened: false, port: null } as PortStatus,
    busy = false,
    onRefresh = () => {},
    onOpen = (_name: string) => {},
    onClose = () => {},
    onConfigChange = (_cfg: PortConfig) => {},
    initialConfig = { baud_rate: 9600, data_bits: 8, stop_bits: 1, parity: 'none', flow_control: 'none', read_timeout_ms: 50 } as PortConfig
  }: Props = $props();

  // 本地维护 config (C4 用单向数据流: 内部修改时通过 onConfigChange 上抛)
  let config = $state<PortConfig>({ ...initialConfig });
  let selectedName = $state('');

  // 外部 initialConfig 变化时同步
  $effect(() => {
    if (
      initialConfig.baud_rate !== config.baud_rate ||
      initialConfig.data_bits !== config.data_bits ||
      initialConfig.stop_bits !== config.stop_bits ||
      initialConfig.parity !== config.parity ||
      initialConfig.flow_control !== config.flow_control
    ) {
      config = { ...initialConfig };
    }
  });

  // 选中端口变化: 优先当前打开的端口 -> ttyUSB2 (Ubuntu 串口 console 假设) -> 任意 ttyUSB* -> 第一个
  $effect(() => {
    if (status.opened && status.port) {
      selectedName = status.port.name;
      return;
    }
    const list = ports;
    if (list.length === 0) {
      selectedName = '';
      return;
    }
    const find = (re: RegExp) => list.find((p: PortInfo) => re.test(p.name))?.name;
    const preferred =
      find(/^ttyUSB2$/) ??
      find(/^ttyACM0$/) ??
      find(/^ttyUSB\d+$/) ??
      find(/^ttyACM\d+$/) ??
      list[0].name;
    if (!selectedName || !list.find((p: PortInfo) => p.name === selectedName)) {
      selectedName = preferred;
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
      <button onclick={onRefresh} disabled={busy} title="刷新端口列表" class="refresh">
        {#if busy}…{:else}↻{/if}
      </button>
    </div>
    {#if ports.length === 0 && !busy}
      <div class="hint hint-warn">
        <strong>未检测到串口设备</strong><br />
        <ol>
          <li>确认 USB-Serial 已插入</li>
          <li>Windows: 打开 <strong>设备管理器</strong> → <strong>端口 (COM &amp; LPT)</strong>,查看是否列出 COM* 端口
            <ul>
              <li>看到 "未知设备" → 装驱动 (CH340/CH341/CP210x/FTDI/PL2303)</li>
              <li>看到 "USB Serial" 但无 COM 编号 → 重装驱动</li>
              <li>列表中无任何串口设备 → 检查线缆/插口</li>
            </ul>
          </li>
          <li>Linux: 终端跑 <code>ls /dev/ttyUSB* /dev/ttyACM*</code> + 检查 <code>dialout</code> 组</li>
          <li>macOS: 系统设置 → 隐私与安全 → 允许 last_port 访问 USB</li>
        </ol>
        点击 <strong>↻</strong> 重新扫描。
      </div>
    {/if}
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
  .refresh {
    min-width: 32px;
    font-size: 14px;
  }
  .hint {
    font-size: 11px;
    line-height: 1.5;
    color: var(--text-dim);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 2px;
    padding: 6px 8px;
  }
  .hint code {
    color: var(--info);
    font-size: 10px;
  }
  .hint-warn {
    border-color: var(--warn);
    color: var(--text);
  }
  .hint-warn strong {
    color: var(--warn);
    display: block;
    margin-bottom: 4px;
  }
  .hint-warn ol {
    margin: 4px 0;
    padding-left: 18px;
  }
  .hint-warn ol li {
    margin: 2px 0;
  }
  .hint-warn ol ul {
    margin: 2px 0;
    padding-left: 16px;
    list-style: disc;
  }
</style>
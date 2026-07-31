/**
 * 全局应用状态 (Svelte 5 runes module, C4)
 *
 * .svelte.ts 后缀让 Svelte 编译器在编译期处理 $state 等 runes,
 * 避免 svelte-check 把 $state 误识别为 svelte/store 的 store prefix。
 */

import type { PortConfig, PortInfo, PortStatus } from './types';

export interface AppState {
  ports: PortInfo[];
  status: PortStatus;
  config: PortConfig;
  error: string | null;
  rxBytes: number;
  rxFrames: number;
  txBytes: number;
  txFrames: number;
  lastRxTs: number | null;
}

/** 全局响应式状态 (deep proxy),App.svelte 持有并通过 props + 回调共享 */
export const appState: AppState = $state({
  ports: [],
  status: { opened: false, port: null },
  config: {
    baud_rate: 9600,
    data_bits: 8,
    stop_bits: 1,
    parity: 'none',
    flow_control: 'none',
    read_timeout_ms: 50
  },
  error: null,
  rxBytes: 0,
  rxFrames: 0,
  txBytes: 0,
  txFrames: 0,
  lastRxTs: null
});

/** UI 锁状态 (避免重复点击触发 race) */
export const uiBusy: { value: boolean } = $state({ value: false });
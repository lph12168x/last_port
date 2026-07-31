/**
 * Tauri 命令 / 事件桥接封装
 *
 * 命令走 invoke (同步等待返回),数据流入走 listen (事件流)。
 * 所有命令的错误以字符串形式抛到前端 (Rust 端 SerialError 自定义 Serialize)。
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';
import type {
  PortConfig,
  PortInfo,
  PortStatus,
  SerialChunk
} from './types';

export async function listPorts(): Promise<PortInfo[]> {
  return invoke('list_ports');
}

export async function openPort(name: string, config: PortConfig): Promise<PortInfo> {
  return invoke('open_port', { name, config });
}

export async function closePort(): Promise<void> {
  return invoke('close_port');
}

export async function writeData(data: Uint8Array | number[]): Promise<number> {
  return invoke('write_data', { data: Array.from(data) });
}

export async function portStatus(): Promise<PortStatus> {
  return invoke('port_status');
}

/**
 * 监听串口接收数据,handler 收到的参数是 SerialChunk 数组
 * (Rust 端 pump 线程每 5ms 批量 emit 一次)
 */
export async function onSerialData(
  handler: (chunks: SerialChunk[]) => void
): Promise<UnlistenFn> {
  return listen<SerialChunk[]>('serial:data', (event) => {
    handler(event.payload);
  });
}

/** 检测是否在 Tauri 运行时 (非浏览器) */
export const isTauri =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** 写入文本到文件 (Rust 端 std::fs::write) */
export async function saveLog(path: string, content: string): Promise<void> {
  return invoke('save_log', { path, content });
}

/** 弹出保存对话框,返回用户选择的路径或 null (取消) */
export async function pickSavePath(opts: {
  defaultPath?: string;
  filters?: { name: string; extensions: string[] }[];
  title?: string;
}): Promise<string | null> {
  return saveDialog({
    defaultPath: opts.defaultPath,
    filters: opts.filters,
    title: opts.title
  });
}
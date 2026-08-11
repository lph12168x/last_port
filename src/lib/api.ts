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

/**
 * 等待 Tauri IPC bridge 注入完成.
 * 在 webview 启动早期 JS 跑得比 Tauri 注入 `__TAURI_INTERNALS__` 快,
 * 此时 invoke/listen 会 throw. 此函数用 polling 等到位.
 *
 * 注意: 只检查 `__TAURI_INTERNALS__`, 不检查 `window.__TAURI__`.
 * `__TAURI__` 全局变量仅在 tauri.conf.json 设置 `withGlobalTauri: true` 时才存在,
 * 而 IPC 实际依赖的是 `__TAURI_INTERNALS__`; 检查 `__TAURI__` 会导致 waitForTauri 永远超时.
 *
 * @param timeoutMs 最长等待时间 (ms). 0 = 无限等待.
 * @returns true = 注入完成, false = 超时
 */
export async function waitForTauri(timeoutMs = 5000): Promise<boolean> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (
      typeof window !== 'undefined' &&
      '__TAURI_INTERNALS__' in window
    ) {
      return true;
    }
    await new Promise((r) => setTimeout(r, 50));
  }
  return false;
}

/**
 * 把前端 console 输出转发到 backend 日志文件.
 * 安装一次即可, 后续所有 console.log / console.warn / console.error
 * 都会出现在 ~/Desktop/last_port.log 中.
 */
export function installFrontendLogBridge(): void {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const w = window as any;
  if (w.__FRONTEND_LOG_BRIDGE_INSTALLED__) return;
  w.__FRONTEND_LOG_BRIDGE_INSTALLED__ = true;

  for (const level of ['log', 'info', 'warn', 'error', 'debug'] as const) {
    const orig = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      try {
        const msg = args
          .map((a) => {
            if (typeof a === 'string') return a;
            try {
              return JSON.stringify(a);
            } catch {
              return String(a);
            }
          })
          .join(' ');
        // fire-and-forget, 不 await
        invoke('frontend_log', { level, msg }).catch(() => {});
      } catch {
        // ignore
      }
      orig(...args);
    };
  }
}

export async function listPorts(): Promise<PortInfo[]> {
  return invoke<PortInfo[]>('list_ports');
}

/**
 * 订阅后端推送的端口列表 (push 模式).
 * 后端启动时 / 端口变化时会 emit "serial:ports" 事件.
 * 返回反订阅函数.
 */
export async function onPortsUpdate(
  handler: (ports: PortInfo[]) => void
): Promise<UnlistenFn> {
  return listen<PortInfo[]>('serial:ports', (event) => {
    handler(event.payload);
  });
}

/**
 * 订阅后端推送的串口错误 (push 模式).
 */
export async function onSerialError(
  handler: (message: string) => void
): Promise<UnlistenFn> {
  return listen<string>('serial:error', (event) => {
    handler(event.payload);
  });
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
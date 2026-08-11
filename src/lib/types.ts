/**
 * last_port 类型定义 (与 Rust serial 模块 serde 形态对齐, C3)
 */

export interface PortInfo {
  name: string;
  label: string;
  /** "usb" | "bluetooth" | "pci" | "unknown" */
  kind: string;
}

export type Parity = 'none' | 'odd' | 'even';
export type FlowControl = 'none' | 'software' | 'hardware';
export type DataBits = 5 | 6 | 7 | 8;
export type StopBits = 1 | 2;

export interface PortConfig {
  baud_rate: number;
  data_bits: DataBits;
  stop_bits: StopBits;
  parity: Parity;
  flow_control: FlowControl;
  read_timeout_ms: number;
}

export const DEFAULT_PORT_CONFIG: PortConfig = {
  baud_rate: 9600,
  data_bits: 8,
  stop_bits: 1,
  parity: 'none',
  flow_control: 'none',
  read_timeout_ms: 50
};

export interface PortStatus {
  opened: boolean;
  port: PortInfo | null;
}

export interface SerialChunk {
  data: number[];
  /** UNIX 毫秒时间戳 */
  ts_ms: number;
}

/** 常用波特率白名单 (与 Rust 端一致) */
export const BAUD_RATES = [
  110, 300, 600, 1200, 2400, 4800, 9600, 14400, 19200, 38400,
  57600, 115200, 128000, 230400, 256000, 460800, 500000, 576000,
  921600, 1000000, 1152000, 1500000, 2000000, 2500000, 3000000,
  3500000, 4000000
] as const;
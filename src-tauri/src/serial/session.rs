//! 单个打开的串口会话 + 后台读线程
//!
//! 读线程持有独立的 `Box<dyn SerialPort>` (通过 `try_clone`),
//! 主线程通过 `Mutex<Box<dyn SerialPort>>` 处理写入。
//! 线程循环以 `read_timeout_ms` 节奏轮询,`stop` 原子标志决定退出。

use super::{PortConfig, Result, SerialError};
use serde::Serialize;
use serialport::{SerialPort, SerialPortInfo, SerialPortType};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// 前端展示用的端口信息
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PortInfo {
    pub name: String,
    pub label: String,
    /// "usb" | "bluetooth" | "pci" | "unknown"
    pub kind: String,
}

impl PortInfo {
    pub fn from_spinfo(info: &SerialPortInfo) -> Self {
        let kind = match &info.port_type {
            SerialPortType::UsbPort(_) => "usb",
            SerialPortType::BluetoothPort => "bluetooth",
            SerialPortType::PciPort => "pci",
            SerialPortType::Unknown => "unknown",
        };
        let label = match &info.port_type {
            SerialPortType::UsbPort(u) => u
                .product
                .clone()
                .unwrap_or_else(|| format!("USB Serial ({})", info.port_name)),
            _ => info.port_name.clone(),
        };
        Self {
            name: info.port_name.clone(),
            label,
            kind: kind.to_string(),
        }
    }
}

/// 列出系统中所有可见串口
pub fn list_ports() -> Result<Vec<PortInfo>> {
    let ports = serialport::available_ports().map_err(SerialError::Serial)?;
    Ok(ports.iter().map(PortInfo::from_spinfo).collect())
}

pub struct Session {
    port: Mutex<Box<dyn SerialPort>>,
    rx: Receiver<Vec<u8>>,
    info: PortInfo,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

// 仅 Debug 用于单元测试断言,Box<dyn SerialPort> 自身没 Debug
// 用一个空实现,只展示 None
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("info", &self.info)
            .field("handle_alive", &self.handle.is_some())
            .finish()
    }
}

impl Session {
    pub fn open(name: &str, cfg: PortConfig) -> Result<Self> {
        let port: Box<dyn SerialPort> = serialport::new(name, cfg.baud_rate)
            .data_bits(cfg.data_bits.into())
            .stop_bits(cfg.stop_bits.into())
            .parity(cfg.parity.into())
            .flow_control(cfg.flow_control.into())
            .timeout(cfg.timeout())
            .open()
            .map_err(|e| match e.kind() {
                serialport::ErrorKind::Io(io::ErrorKind::NotFound) => {
                    SerialError::PortNotFound(name.to_string())
                }
                serialport::ErrorKind::NoDevice => SerialError::PortBusy(name.to_string()),
                _ => SerialError::Serial(e),
            })?;

        // 给读线程一份独立的 port 副本
        let port_clone = port.try_clone().map_err(SerialError::Serial)?;

        let info = PortInfo {
            name: name.to_string(),
            label: name.to_string(),
            kind: "unknown".to_string(),
        };

        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let mut reader = port_clone;

        let handle = thread::Builder::new()
            .name(format!("serial-reader-{}", name))
            .spawn(move || {
                let mut buf = [0u8; 1024];
                while !stop_thread.load(Ordering::Relaxed) {
                    match reader.read(&mut buf) {
                        Ok(0) => {
                            // read_timeout 触发,无数据 — 继续轮询
                        }
                        Ok(n) => {
                            // 发送失败意味着 Session 已被 drop,退出线程
                            if tx.send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                        Err(e) if e.kind() == io::ErrorKind::TimedOut => {
                            // 正常的 timeout,继续
                        }
                        Err(_) => {
                            // 其他 IO 错误,通常意味着 fd 已关闭,退出
                            break;
                        }
                    }
                }
            })
            .map_err(|e| SerialError::Poisoned(format!("spawn thread: {}", e)))?;

        Ok(Self {
            port: Mutex::new(port),
            rx,
            info,
            stop,
            handle: Some(handle),
        })
    }

    pub fn write(&self, data: &[u8]) -> Result<usize> {
        let mut guard = self.port.lock().map_err(|e| SerialError::Poisoned(e.to_string()))?;
        guard.write(data).map_err(SerialError::Io)
    }

    pub fn try_recv(&self) -> Option<Vec<u8>> {
        match self.rx.try_recv() {
            Ok(data) => Some(data),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }

    pub fn info(&self) -> &PortInfo {
        &self.info
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // 1. 通知读线程退出
        self.stop.store(true, Ordering::Relaxed);
        // 2. 拿走 lock 释放 port,关闭底层 fd,read 立即失败
        if let Ok(guard) = self.port.lock() {
            drop(guard);
        }
        // 3. 等读线程退出 (最多 read_timeout_ms)
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::UsbPortInfo;

    #[test]
    fn port_info_from_usb() {
        let info = SerialPortInfo {
            port_name: "/dev/ttyUSB0".into(),
            port_type: SerialPortType::UsbPort(UsbPortInfo {
                vid: 0x1234,
                pid: 0x5678,
                serial_number: None,
                product: Some("FTDI FT232".into()),
                manufacturer: Some("FTDI".into()),
            }),
        };
        let p = PortInfo::from_spinfo(&info);
        assert_eq!(p.name, "/dev/ttyUSB0");
        assert_eq!(p.kind, "usb");
        assert_eq!(p.label, "FTDI FT232");
    }

    #[test]
    fn port_info_from_unknown() {
        let info = SerialPortInfo {
            port_name: "/dev/ttyS0".into(),
            port_type: SerialPortType::Unknown,
        };
        let p = PortInfo::from_spinfo(&info);
        assert_eq!(p.kind, "unknown");
    }

    /// 真实硬件测试需要 USB-Serial 适配器,默认跳过。
    /// 跑法: `cargo test -- --ignored`
    #[test]
    #[ignore]
    fn list_ports_does_not_panic() {
        // 即使没有任何串口,也不应 panic
        let _ = list_ports();
    }
}
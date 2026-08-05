//! 集成测试: 用 PTY pair 模拟 115200 串口端到端链路.
//!
//! - 端 A: PTY slave, serialport-rs 作为 last_port 视角打开 (模拟 /dev/ttyUSB3)
//! - 端 B: PTY master, 单独 thread 读 RX (模拟 ubuntu 串口终端)
//!
//! 用真 /dev/ttyUSB3 跑, 需要 last_port_lib 走 serialport-rs (root caller
//! 或 dialout 组成员). chmod 666 临时也行.

use last_port_lib::serial::{DataBits, FlowControl, Manager, Parity, PortConfig, StopBits};
use nix::pty::{openpty, OpenptyResult};
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, OwnedFd};
use std::time::Duration;

const TEST_BAUD: u32 = 115200;
const TEST_PAYLOAD: &[u8] = b"ls\n";

fn main() {
    // 1. 尝试真 /dev/ttyUSB3
    let use_real = std::env::var("LAST_PORT_E2E_USE_PTY").map(|v| v != "1").unwrap_or(true);
    let device = if use_real { "/dev/ttyUSB3" } else { "" };

    if use_real {
        run_real(device);
    } else {
        run_pty();
    }
}

fn run_real(device: &str) {
    eprintln!("[diag] opening REAL device {}", device);
    let cfg = PortConfig {
        baud_rate: TEST_BAUD,
        data_bits: DataBits::Eight,
        stop_bits: StopBits::One,
        parity: Parity::None,
        flow_control: FlowControl::None,
        read_timeout_ms: 50,
    };
    let m = Manager::new();
    if let Err(e) = m.open(device, cfg) {
        eprintln!("[diag] REAL OPEN failed: {}", e);
        eprintln!("[diag] (设备可能没权限 — 跑在 dialout 组, 或 chmod 666 /dev/ttyUSB3)");
        std::process::exit(2);
    }
    eprintln!("[diag] opened REAL");

    let n = m.write(TEST_PAYLOAD).expect("write");
    eprintln!("[diag] wrote {} bytes: {:?}", n, TEST_PAYLOAD);

    // 等 + 收
    std::thread::sleep(Duration::from_millis(500));
    let mut total_rx = 0;
    let mut rx_dump = Vec::new();
    for _ in 0..40 {
        if let Some(chunk) = m.try_recv() {
            total_rx += chunk.len();
            rx_dump.extend_from_slice(&chunk);
            eprintln!("[diag] RX chunk ({} bytes): {:?}", chunk.len(), chunk);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    eprintln!("[diag] total RX: {} bytes", total_rx);
    eprintln!("[diag] RX full: {:?}", rx_dump);

    let _ = m.close();
    eprintln!("[diag] done");
}

fn run_pty() {
    let OpenptyResult { master, slave } = openpty(None, None).expect("openpty");
    let slave_path = format!("/proc/self/fd/{}", slave.as_raw_fd());
    eprintln!("[diag] PTY slave: {}", slave_path);

    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    let reader = std::thread::spawn(move || {
        let mut f = File::from(master);
        let mut buf = [0u8; 256];
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            match f.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
    });

    let cfg = PortConfig {
        baud_rate: TEST_BAUD,
        data_bits: DataBits::Eight,
        stop_bits: StopBits::One,
        parity: Parity::None,
        flow_control: FlowControl::None,
        read_timeout_ms: 50,
    };
    let m = Manager::new();
    if let Err(e) = m.open(&slave_path, cfg) {
        eprintln!("[diag] PTY OPEN failed: {}", e);
        std::process::exit(2);
    }
    eprintln!("[diag] opened PTY");

    let n = m.write(TEST_PAYLOAD).expect("write");
    eprintln!("[diag] wrote {} bytes: {:?}", n, TEST_PAYLOAD);

    let mut total_rx = 0;
    let mut rx_dump = Vec::new();
    let read_deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < read_deadline {
        if let Ok(chunk) = rx.try_recv() {
            total_rx += chunk.len();
            rx_dump.extend_from_slice(&chunk);
            eprintln!("[diag] RX chunk ({} bytes): {:?}", chunk.len(), chunk);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    eprintln!("[diag] total RX: {} bytes", total_rx);
    eprintln!("[diag] RX full: {:?}", rx_dump);

    let _ = m.close();
    drop(slave);
    let _ = reader.join();
    eprintln!("[diag] done");
}
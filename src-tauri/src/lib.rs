//! last_port 后端库 (Tauri 入口 + 串口抽象)
//!
//! - C1: 仅注册 dialog 插件
//! - C2: 暴露 `serial` 模块 (纯 Rust,不依赖 Tauri)
//! - C3: 注册 Tauri commands + 后台 pump 线程

pub mod commands;
pub mod log;
pub mod serial;

use std::sync::Arc;
use tauri::Emitter; // 提供 AppHandle::emit
use tauri::Manager; // 提供 AppHandle::try_state

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 必须在 tauri Builder 之前 — 把 stderr 镜像到日志文件
    log::init();

    log!("[startup] last_port v{}", env!("CARGO_PKG_VERSION"));
    log!("[startup] WAYLAND_DISPLAY={:?} DISPLAY={:?}",
        std::env::var("WAYLAND_DISPLAY").ok(),
        std::env::var("DISPLAY").ok());
    let manager = Arc::new(serial::Manager::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(manager.clone())
        .setup(move |app| {
            let handle = app.handle().clone();
            commands::spawn_pump(handle.clone(), manager.clone());

            // 启动探测: 不依赖 webview, 直接在 setup 阶段调用 list_ports.
            // 两阶段 emit 解决 webview 启动后再 subscribe 错过早 emit 的问题:
            //  - 立刻 emit 一次 (webview 启动前)
            //  - 定时器每 2s emit 一次 (webview 启动后能收到)
            // 上限 600 次 (=20 分钟) — 短时 USB 热插拔事件都会捕获,
            // 之后降级到 60s 一次的最低频扫描,避免日志填满磁盘.
            let emit_handle = handle.clone();
            std::thread::spawn(move || {
                let mut counter = 0u64;
                loop {
                    counter += 1;
                    match serial::list_ports() {
                        Ok(ports) => {
                            if counter <= 3 || counter % 30 == 0 {
                                log!(
                                    "[startup-probe #{}] {} port(s) visible",
                                    counter,
                                    ports.len()
                                );
                            }
                            if let Err(e) = emit_handle.emit("serial:ports", &ports) {
                                log!("[startup-probe] emit failed: {}", e);
                            }
                        }
                        Err(e) => {
                            log!("[startup-probe] serial ERR: {}", e);
                            let _ = emit_handle.emit("serial:error", e.to_string());
                        }
                    }
                    let backoff = if counter < 30 {
                        std::time::Duration::from_secs(2)
                    } else {
                        std::time::Duration::from_secs(60)
                    };
                    std::thread::sleep(backoff);
                }
            });

            // Auto-test: 如果环境变量 LAST_PORT_AUTORUN 设了, 自动 open + write + close.
            // 让 headless 验证器直接看到 backend 链路.
            if let Ok(spec) = std::env::var("LAST_PORT_AUTORUN") {
                // spec 格式: "device:baud" 例如 "/dev/ttyUSB3:115200"
                let parts: Vec<&str> = spec.split(':').collect();
                if parts.len() == 2 {
                    let dev = parts[0];
                    let baud: u32 = parts[1].parse().unwrap_or(115200);
                    log!("[autorun] opening {} @ {} baud", dev, baud);
                    let cfg = serial::PortConfig {
                        baud_rate: baud,
                        data_bits: serial::DataBits::Eight,
                        stop_bits: serial::StopBits::One,
                        parity: serial::Parity::None,
                        flow_control: serial::FlowControl::None,
                        read_timeout_ms: 50,
                    };
                    let m = serial::Manager::new();
                    match m.open(dev, cfg) {
                        Ok(_) => {
                            log!("[autorun] opened");
                            match m.write(b"ls\n") {
                                Ok(n) => log!("[autorun] wrote {} bytes", n),
                                Err(e) => log!("[autorun] write failed: {}", e),
                            }
                            // 等一会儿看 RX
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            let mut total_rx = 0;
                            for _ in 0..40 {
                                if let Some(chunk) = m.try_recv() {
                                    total_rx += chunk.len();
                                    log!("[autorun] RX chunk ({} bytes): {:?}", chunk.len(), chunk);
                                }
                                std::thread::sleep(std::time::Duration::from_millis(25));
                            }
                            log!("[autorun] total RX: {} bytes", total_rx);
                            let _ = m.close();
                            log!("[autorun] closed");
                        }
                        Err(e) => {
                            log!("[autorun] open {} failed: {}", dev, e);
                        }
                    }
                } else {
                    log!("[autorun] bad spec (expected device:baud): {}", spec);
                }
            }

            // GUI flood test: 通过 Tauri state 打开端口(走 TerminalPanel 渲染路径)
            // 验证节流修复在 GUI 下有效
            if let Ok(spec) = std::env::var("LAST_PORT_GUI_FLOOD") {
                let parts: Vec<&str> = spec.split(':').collect();
                if parts.len() == 2 {
                    let dev = parts[0];
                    let baud: u32 = parts[1].parse().unwrap_or(115200);
                    // 等 2s 让 GUI 完全挂载
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    log!("[gui-flood] opening {} @ {} via state", dev, baud);
                    let cfg = serial::PortConfig {
                        baud_rate: baud,
                        data_bits: serial::DataBits::Eight,
                        stop_bits: serial::StopBits::One,
                        parity: serial::Parity::None,
                        flow_control: serial::FlowControl::None,
                        read_timeout_ms: 50,
                    };
                    if let Some(state) = app.handle().try_state::<Arc<serial::Manager>>() {
                        match state.open(dev, cfg) {
                            Ok(_) => {
                                log!("[gui-flood] opened, will hold for 10s");
                                std::thread::sleep(std::time::Duration::from_secs(10));
                                let _ = state.close();
                                log!("[gui-flood] closed");
                            }
                            Err(e) => log!("[gui-flood] open failed: {}", e),
                        }
                    } else {
                        log!("[gui-flood] no state Manager");
                    }
                }
            }

            // Closed-loop smoke test: open the port and send commands so we can
            // visually verify the "Enter -> send -> shell echoes" round-trip
            // inside the running GUI. Usage: LAST_PORT_CLOSED_LOOP=dev:baud
            if let Ok(spec) = std::env::var("LAST_PORT_CLOSED_LOOP") {
                let parts: Vec<&str> = spec.split(':').collect();
                if parts.len() == 2 {
                    let dev = parts[0].to_string();
                    let baud: u32 = parts[1].parse().unwrap_or(115200);
                    let app_handle = handle.clone();
                    std::thread::spawn(move || {
                        // 等 webview 挂载
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        log!("[closed-loop] opening {} @ {} via Tauri Manager", dev, baud);
                        let cfg = serial::PortConfig {
                            baud_rate: baud,
                            data_bits: serial::DataBits::Eight,
                            stop_bits: serial::StopBits::One,
                            parity: serial::Parity::None,
                            flow_control: serial::FlowControl::None,
                            read_timeout_ms: 50,
                        };
                        let state = match app_handle.try_state::<Arc<serial::Manager>>() {
                            Some(s) => s,
                            None => {
                                log!("[closed-loop] no Manager state");
                                return;
                            }
                        };
                        if let Err(e) = state.open(&dev, cfg) {
                            log!("[closed-loop] open failed: {}", e);
                            return;
                        }
                        log!("[closed-loop] opened");
                        // 真实 dmesg 行 长 200-500 chars, 单 hex dump 行偶尔 >1000 chars.
                        // long_line 模拟后者: 1100+ chars, 中间一大段无空格的 hex+ascii.
                        let payload_body: String = std::iter::repeat('x').take(1024).collect();
                        let mut long_line = String::from("echo '[12345.678901] HEXDUMP: payload=");
                        long_line.push_str(&payload_body);
                        long_line.push_str(" and_more_stuff\n");
                        let long_line: &[u8] = long_line.as_bytes();
                        // 复现 user 报: 输 ls, Enter, 输 pwd, Enter -> 数据解析错误.
                        // 直接经 Manager.write (跟 GUI send 走同样代码路径) 字符级.
                        let mut lspwd_cmds: Vec<Vec<u8>> = vec![];
                        // 1) cooked 模式: 'ls\n' 整行
                        lspwd_cmds.push(b"ls\n".to_vec());
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        // 2) cooked 模式: 'pwd\n' 整行
                        lspwd_cmds.push(b"pwd\n".to_vec());
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        // 3) raw 模式字符级: 'l' 's' 单 char, 然后 '\n'
                        lspwd_cmds.push(b"l".to_vec());
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        lspwd_cmds.push(b"s".to_vec());
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        lspwd_cmds.push(b"\n".to_vec());
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        // 4) raw 模式字符级: 'p' 'w' 'd' 单 char, 然后 '\n'
                        lspwd_cmds.push(b"p".to_vec());
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        lspwd_cmds.push(b"w".to_vec());
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        lspwd_cmds.push(b"d".to_vec());
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        lspwd_cmds.push(b"\n".to_vec());
                        for c in lspwd_cmds {
                            std::thread::sleep(std::time::Duration::from_millis(50));
                            if let Err(e) = state.write(&c) {
                                log!("[closed-loop] write failed: {}", e);
                            } else {
                                log!("[closed-loop] sent {} bytes", c.len());
                            }
                        }
                        // 给 shell echo + uname 输出流回 GUI 6s
                        std::thread::sleep(std::time::Duration::from_secs(6));
                        log!("[closed-loop] test window complete");
                        // 不关闭,留 GUI 操作员观察
                    });
                }
            }

            // 滚动压测: 灌入 N 行,验证满屏后 scroll-anchor 是否准确停底
            // LAST_PORT_GUI_FLOOD_LINES=2000
            if let Ok(n_str) = std::env::var("LAST_PORT_GUI_FLOOD_LINES") {
                let n: usize = n_str.parse().unwrap_or(2000);
                let app_handle = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(4));
                    let state = match app_handle.try_state::<Arc<serial::Manager>>() {
                        Some(s) => s,
                        None => {
                            log!("[gui-flood-lines] no Manager state");
                            return;
                        }
                    };
                    let dev = "/dev/ttyUSB2";
                    let cfg = serial::PortConfig {
                        baud_rate: 115200,
                        data_bits: serial::DataBits::Eight,
                        stop_bits: serial::StopBits::One,
                        parity: serial::Parity::None,
                        flow_control: serial::FlowControl::None,
                        read_timeout_ms: 50,
                    };
                    if state.open(dev, cfg).is_err() {
                        log!("[gui-flood-lines] open failed");
                        return;
                    }
                    log!("[gui-flood-lines] opened, will send {} lines", n);
                    let mut buf = Vec::with_capacity(64);
                    for i in 0..n {
                        buf.clear();
                        let s = format!("echo LP_LINE_{:06}\n", i);
                        buf.extend_from_slice(s.as_bytes());
                        if state.write(&buf).is_err() {
                            log!("[gui-flood-lines] write failed at {}", i);
                            break;
                        }
                        // 50us 间隔让前端有时间 buffer; 太密集会把 WebKit 拖垮
                        if i % 50 == 0 {
                            std::thread::sleep(std::time::Duration::from_millis(2));
                        }
                    }
                    log!("[gui-flood-lines] sent {}, holding port open", n);
                    // 不关, 留 GUI 在 30s 内观察
                    std::thread::sleep(std::time::Duration::from_secs(20));
                });
            }

            // 前端注入式压测: 不依赖底层串口, 直接 emit serial:data 事件
            // 模拟 Ubuntu bash 在 115200 上的实时回声(bracketed-paste 切换 + prompt).
            // LAST_PORT_GUI_FAKE_FLOOD=6000
            if let Ok(n_str) = std::env::var("LAST_PORT_GUI_FAKE_FLOOD") {
                let n: usize = n_str.parse().unwrap_or(6000);
                let app_handle = handle.clone();
                std::thread::spawn(move || {
                    use tauri::Emitter;
                    // 等 webview 完全挂载
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    log!("[fake-flood] emitting {} lines via serial:data", n);
                    // 先发一个真实 echo 的启动序列, 证明 ANSI strip 工作
                    let primer: Vec<u8> = b"\x1b[?2004l\rroot@ubuntu2204-arm64:~#".to_vec();
                    let _ = app_handle.emit(
                        "serial:data",
                        vec![commands::SerialChunk {
                            data: primer,
                            ts_ms: commands::now_ms(),
                        }],
                    );
                    // 然后每行一行 echo + prompt sequence, 模拟真实 bash
                    let mut emitted = 0usize;
                    let mut counter: u64 = 0;
                    let mut last_print = std::time::Instant::now();
                    for i in 0..n {
                        let chunks: Vec<u8> = format!(
                            "\x1b[?2004l\recho LP_FAKE_{:06}\r\nLP_FAKE_{:06}\r\n\x1b[?2004hroot@ubuntu2204-arm64:~# ",
                            i, i
                        ).into_bytes();
                        let chunk = commands::SerialChunk {
                            data: chunks,
                            ts_ms: commands::now_ms(),
                        };
                        if app_handle.emit("serial:data", vec![chunk]).is_err() {
                            log!("[fake-flood] emit failed at {}", i);
                            break;
                        }
                        emitted += 1;
                        counter += 1;
                        if counter % 500 == 0 {
                            let elapsed = last_print.elapsed();
                            log!(
                                "[fake-flood] emitted {}/{}  rate={:.0}/s",
                                emitted,
                                n,
                                if elapsed.as_secs_f64() > 0.0 { 500.0 / elapsed.as_secs_f64() } else { 0.0 }
                            );
                            last_print = std::time::Instant::now();
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }
                    }
                    log!("[fake-flood] DONE emitted {}", emitted);
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_ports,
            commands::open_port,
            commands::close_port,
            commands::write_data,
            commands::port_status,
            commands::save_log,
            commands::frontend_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
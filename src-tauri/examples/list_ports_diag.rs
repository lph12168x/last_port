//! 独立诊断工具:直接调用 last_port_lib::serial::list_ports,
//! 不需要 webview,绕过前端验证 serialport-rs 在本机是否能看到设备。
//!
//! 用法: cargo run --example list_ports_diag --release
//! 输出: 打印 list_ports 调用结果 (stderr + stdout)
//! 退出码: 0 找到设备 / 1 找不到 / 2 调用错误

use last_port_lib::serial;

fn main() {
    eprintln!("[diag] serialport crate version: {}", env!("CARGO_PKG_VERSION"));
    eprintln!("[diag] calling serial::list_ports()...");
    match serial::list_ports() {
        Ok(ports) => {
            eprintln!("[diag] OK, {} port(s) found", ports.len());
            for p in &ports {
                println!("  {} ({}) kind={}", p.name, p.label, p.kind);
            }
            if ports.is_empty() {
                eprintln!("[diag] WARN: zero ports");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("[diag] ERR: {} ({:?})", e, e);
            std::process::exit(2);
        }
    }
}
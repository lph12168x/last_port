//! Tauri commands 桥接层
//!
//! 把 [`crate::serial::Manager`] 的方法暴露给前端。
//! 事件:`serial:data` (后端 -> 前端,批量推)
//!
//! 设计要点:
//! - Manager 用 `tauri::State<Arc<Manager>>` 注入 (单例)
//! - 数据通过后台 `pump` 线程批量 emit (~5ms 节流 + 256 chunk 上限)
//! - 时间戳用 UNIX 毫秒

use crate::serial::{Manager, PortConfig, PortInfo, SerialError};
use crate::log;
use serde::Serialize;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};

/// 单帧数据,前端 listen("serial:data") 收到的数组元素
#[derive(Debug, Clone, Serialize)]
pub struct SerialChunk {
    pub data: Vec<u8>,
    pub ts_ms: i64,
}

/// 当前会话状态
#[derive(Debug, Clone, Serialize)]
pub struct PortStatus {
    pub opened: bool,
    pub port: Option<PortInfo>,
}
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub fn list_ports() -> Result<Vec<PortInfo>, SerialError> {
    crate::serial::list_ports()
}

#[tauri::command]
pub fn open_port(
    manager: State<'_, Arc<Manager>>,
    name: String,
    config: PortConfig,
) -> Result<PortInfo, SerialError> {
    log!("[open_port] invoked name={} baud={}", name, config.baud_rate);
    let r = manager.open(&name, config);
    match &r {
        Ok(info) => log!("[open_port] OK: {:?}", info),
        Err(e) => log!("[open_port] ERR: {}", e),
    }
    r
}

#[tauri::command]
pub fn close_port(manager: State<'_, Arc<Manager>>) -> Result<(), SerialError> {
    manager.close()
}

#[tauri::command]
pub fn write_data(
    manager: State<'_, Arc<Manager>>,
    data: Vec<u8>,
) -> Result<usize, SerialError> {
    log!("[write_data] {} bytes: {:?}", data.len(), data);
    let r = manager.write(&data);
    match &r {
        Ok(n) => log!("[write_data] wrote {} bytes", n),
        Err(e) => log!("[write_data] ERR: {}", e),
    }
    r
}

#[tauri::command]
pub fn port_status(manager: State<'_, Arc<Manager>>) -> PortStatus {
    PortStatus {
        opened: manager.is_open(),
        port: manager.current_port(),
    }
}

/// 把内容写入文件 (供前端导出日志使用)
#[tauri::command]
pub fn save_log(path: String, content: String) -> Result<(), SerialError> {
    std::fs::write(&path, content.as_bytes())?;
    Ok(())
}

/// 接收前端 console.log / console.error 等, 写入后台日志,
/// 便于 headless / 自动诊断前端运行时. 前端任何时候可以 invoke.
#[tauri::command]
pub fn frontend_log(level: String, msg: String) {
    log!("[frontend] [{}] {}", level, msg);
}

/// 启动后台 pump 线程,把 Manager 的 try_recv 缓冲批量 emit 到前端。
///
/// 节流: 5ms tick + 单 batch 上限 256 chunk,避免高频小包事件洪水。
pub fn spawn_pump(app: AppHandle, manager: Arc<Manager>) {
    thread::Builder::new()
        .name("serial-pump".into())
        .spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(5));
                let mut batch: Vec<SerialChunk> = Vec::new();
                while let Some(data) = manager.try_recv() {
                    batch.push(SerialChunk {
                        data,
                        ts_ms: now_ms(),
                    });
                    if batch.len() >= 256 {
                        break;
                    }
                }
                if !batch.is_empty() {
                    if let Err(e) = app.emit("serial:data", &batch) {
                        eprintln!("serial:data emit failed: {}", e);
                    }
                }
            }
        })
        .expect("spawn serial-pump thread");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_chunk_serializes() {
        let chunk = SerialChunk {
            data: vec![0x48, 0x65, 0x6c, 0x6c, 0x6f],
            ts_ms: 1_700_000_000_000,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        // 验证字段名正确 (前端依赖)
        assert!(json.contains("\"data\":[72,101,108,108,111]"));
        assert!(json.contains("\"ts_ms\":1700000000000"));
    }

    #[test]
    fn port_status_serializes_closed() {
        let s = PortStatus {
            opened: false,
            port: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "{\"opened\":false,\"port\":null}");
    }

    #[test]
    fn now_ms_is_monotonic() {
        let a = now_ms();
        std::thread::sleep(Duration::from_millis(5));
        let b = now_ms();
        assert!(b >= a);
    }

    #[test]
    fn save_log_writes_content_to_file() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("last_port_test_{}.txt", std::process::id()));
        let content = "hello world\nline2\n".to_string();
        save_log(path.to_string_lossy().to_string(), content.clone()).unwrap();
        let read = std::fs::read_to_string(&path).unwrap();
        assert_eq!(read, content);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn save_log_rejects_invalid_path() {
        // Windows 不允许 ':' 在路径中段,Linux 用 NUL 触发错误
        let result = save_log("/dev/null/foo/bar".into(), "x".into());
        assert!(result.is_err());
    }
}
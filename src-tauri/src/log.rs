//! 简易日志:stderr + 文件双写
//!
//! 日志直接写到桌面,方便用户查看:
//! - Windows: `%USERPROFILE%\Desktop\last_port.log`
//! - macOS:   `$HOME/Desktop/last_port.log`
//! - Linux:   `$HOME/Desktop/last_port.log`
//!
//! 启动时在 stderr 打印一次路径。

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static LOG_FILE: Mutex<Option<File>> = Mutex::new(None);
static LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// 在 setup 阶段调用一次。失败不 panic,仅 stderr 输出警告。
pub fn init() {
    let path = match log_path() {
        Some(p) => p,
        None => {
            eprintln!("[log] WARN: could not determine desktop path");
            return;
        }
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("[log] WARN: mkdir {} failed: {}", parent.display(), e);
            return;
        }
    }
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(f) => {
            eprintln!("[log] writing to {}", path.display());
            *LOG_PATH.lock().unwrap() = Some(path.clone());
            *LOG_FILE.lock().unwrap() = Some(f);
        }
        Err(e) => {
            eprintln!("[log] WARN: open {} failed: {}", path.display(), e);
        }
    }
}

/// 当前日志路径 (用于 UI 显示)
pub fn path() -> Option<PathBuf> {
    LOG_PATH.lock().unwrap().clone()
}

/// 写一行日志到 stderr + 文件
pub fn write(msg: &str) {
    eprintln!("{}", msg);
    if let Ok(mut guard) = LOG_FILE.lock() {
        if let Some(f) = guard.as_mut() {
            let _ = writeln!(f, "{}", msg);
            let _ = f.flush();
        }
    }
}

fn log_path() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        // Windows 桌面通常在 %USERPROFILE%\Desktop
        std::env::var_os("USERPROFILE")
            .map(|p| PathBuf::from(p).join("Desktop").join("last_port.log"))
    } else {
        // macOS / Linux: ~/Desktop
        std::env::var_os("HOME")
            .map(|p| PathBuf::from(p).join("Desktop").join("last_port.log"))
    }
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::log::write(&msg);
    }};
}
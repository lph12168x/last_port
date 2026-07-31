//! last_port 后端库 (Tauri 入口 + 串口抽象)
//!
//! - C1: 仅注册 dialog 插件
//! - C2: 暴露 `serial` 模块 (纯 Rust,不依赖 Tauri)
//! - C3: 注册 Tauri commands + 后台 pump 线程

pub mod commands;
pub mod serial;

use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let manager = Arc::new(serial::Manager::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(manager.clone())
        .setup(move |app| {
            let handle = app.handle().clone();
            commands::spawn_pump(handle, manager.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_ports,
            commands::open_port,
            commands::close_port,
            commands::write_data,
            commands::port_status,
            commands::save_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
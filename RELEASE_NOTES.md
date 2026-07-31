# Release Notes

## v0.1.0 (2025-07-31)

首次发布 (MVP)。

### 新增

- 串口枚举 (USB / 蓝牙 / PCI / 未知)
- 串口配置: 波特率 (27 种白名单)、数据位、停止位、校验、流控
- 实时接收: HEX/ASCII 双视图、毫秒时间戳、自动滚动、暂停、清空
- 发送: HEX/ASCII 双模式、Ctrl+Enter 快捷键、周期发送、最近 20 条历史
- 日志导出: 一键导出当前接收缓冲到 txt (格式 `[HH:MM:SS.mmm] HEX | ASCII`)
- 三端打包: Windows / macOS / Linux

### 技术指标

- 安装包大小: ~10MB (Tauri 优势)
- 前端包大小: ~21 KB JS (gzipped) + ~2 KB CSS
- 启动时间: <500ms (估算,基于 Webview 复用)
- 内存占用: 30-60MB (运行时,估算)

### 测试覆盖

- Rust 单元测试: 18 个
- Rust 集成测试: 2 个 (PTY 回环,需要 `--ignored`)
- 前端单元测试: 23 个 (vitest)

### 已知问题

- `svelte-check` 4.7.4 与 Svelte 5 runes 类型推断不兼容 (工具 bug,源码已确认) — `npm run check` 报错不影响实际编译运行
- 占位应用图标 (纯色),正式发布前需要替换为设计稿
- 应用未签名,Windows SmartScreen / macOS Gatekeeper 会警告
- Linux 串口需要用户属于 `dialout` 组

### 致谢

- [Tauri](https://tauri.app) — 跨平台原生应用框架
- [serialport-rs](https://gitlab.com/susurrus/serialport-rs) — Rust 串口库
- [Svelte](https://svelte.dev) — 前端框架

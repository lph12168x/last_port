# last_port

> 跨平台现代串口工具 — Tauri 2 + Rust + Svelte 5

![status](https://img.shields.io/badge/status-MVP-orange)
![platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-blue)
![license](https://img.shields.io/badge/license-MIT-green)

`last_port` 是一款轻量、跨平台的串口调试助手。打包仅 ~10MB,启动快,内存占用低,三端体验一致。

## 功能特性 (v0.1.0)

- ✅ **串口枚举**: 自动列出系统 USB / 蓝牙 / PCI / 未知类型串口
- ✅ **可配置**: 波特率 (110 ~ 4 Mbps 白名单)、数据位 (5/6/7/8)、停止位 (1/2)、校验 (None/Odd/Even)、流控 (None/XON-XOFF/RTS-CTS)
- ✅ **实时收发**: HEX/ASCII 双视图、时间戳 (毫秒精度)、自动滚动 (向上滚暂停)、暂停/继续、清空
- ✅ **发送**: HEX/ASCII 双模式输入、Ctrl+Enter 快捷键、周期发送 (10ms ~ 60s 可调)、最近 20 条历史一键重发
- ✅ **日志导出**: 一键导出当前接收缓冲到 txt 文件
- ✅ **跨平台**: Windows / macOS / Linux 三端原生打包

## 截图占位

```
┌──────────────────────────────────────────────────────────────────────┐
│ last_port   v0.1.0                                       OPEN · COM3 │
├──────────┬─────────────────────────────────────┬─────────────────────┤
│ 端口     │ 接收                                │ 发送              │
│ [COM3 ▾] │ [HEX/ASCII] [⏸ 暂停] [💾 导出] [清空]│ [ASCII/HEX]        │
│ ↻        │ ─────────────────────────           │ ┌──────────────┐   │
│          │ 12:34:56.789 48 65 6C 6C 6F  |Hello │ │ hello        │   │
│ 波特率   │ 12:34:56.790 0D 0A           |..    │ └──────────────┘   │
│ [9600 ▾] │ 12:34:57.001 41 42 43        |ABC   │ [发送 Ctrl+Enter] │
│          │                                     │ ☐ 周期 [1000]ms   │
│ 数据位   │                                     │ 历史: hello, A1 B2 │
│ [8 ▾]    │                                     │                     │
│          │                                     │                     │
│ [打开]   │                                     │                     │
├──────────┴─────────────────────────────────────┴─────────────────────┤
│ RX 18B / 3 帧    TX 5B / 1 帧     最近 12:34:57.001                  │
└──────────────────────────────────────────────────────────────────────┘
```

## 技术栈

| 层 | 选型 |
|---|---|
| 应用框架 | Tauri 2 (Rust + Webview) |
| 串口库 | `serialport` 4.x + `libudev` |
| 前端 | Svelte 5 (runes) + TypeScript + Vite 6 |
| 样式 | 原生 CSS + CSS 变量 (无重型 UI 库) |
| 桥接 | `tauri::Emitter` + `@tauri-apps/api/core` invoke |
| 存储 | 文件系统 (`std::fs::write`,via `tauri-plugin-dialog`) |
| 测试 | Rust 单元/集成测试 + Vitest |

## 开发环境要求

| 工具 | 版本 |
|---|---|
| Node.js | 18+ (推荐 20+) |
| Rust | 1.77+ (推荐 stable) |
| 系统依赖 | 见下方"平台系统依赖" |

### 平台系统依赖

**Linux (Ubuntu/Debian 22.04+)**:

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  build-essential curl wget file \
  libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  pkg-config libudev-dev
```

**macOS**:

```bash
xcode-select --install   # Command Line Tools
```

**Windows**:

- WebView2 Runtime (Win11 自带,Win10 需 [手动安装](https://developer.microsoft.com/microsoft-edge/webview2/))
- MSVC Build Tools (Visual Studio Installer 选 "C++ build tools")

## 常用命令

```bash
# 安装前端依赖
npm install

# 开发模式 (带窗口)
npm run tauri:dev

# 单元测试
cargo test --manifest-path src-tauri/Cargo.toml --lib   # Rust
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored  # 集成测试 (PTY 回环)
npm test                                               # 前端

# 类型检查
npm run check    # ⚠ svelte-check 4.7.4 与 Svelte 5 runes 类型推断已知 bug,
                 # 错误均为工具误报,实际 Vite/Svelte 编译器正确处理。
                 # 详见 docs/dev_notes.md。

# 构建生产前端
npm run build

# 三端打包
npm run tauri:build
```

### `tauri:build` 产物路径

- **Linux**: `src-tauri/target/release/bundle/{deb,appimage,rpm}/`
- **macOS**: `src-tauri/target/release/bundle/{dmg,macos}/`
- **Windows**: `src-tauri/target/release/bundle/{msi,nsis}/`

## 项目结构

```
last_port/
├── src/                          # 前端 (Svelte 5 + TS)
│   ├── App.svelte                # 顶层布局 + 全局状态
│   ├── main.ts
│   ├── app.css
│   └── lib/
│       ├── api.ts                # Tauri invoke/listen 封装
│       ├── bytes.ts              # 字节 ↔ HEX/ASCII 工具
│       ├── bytes.test.ts         # vitest 单测
│       ├── state.svelte.ts       # runes module (全局响应式状态)
│       ├── types.ts              # TS 类型
│       └── components/
│           ├── ConfigPanel.svelte
│           ├── ReceivePanel.svelte
│           └── SendPanel.svelte
├── src-tauri/                    # 后端 (Rust)
│   ├── src/
│   │   ├── main.rs               # 桌面入口
│   │   ├── lib.rs                # Tauri Builder
│   │   ├── commands.rs           # Tauri commands + 事件 pump
│   │   └── serial/{mod,config,error,session}.rs
│   ├── tests/integration.rs      # PTY 回环测试 (--ignored)
│   ├── capabilities/default.json
│   ├── icons/                    # 占位图标 (C8 可优化)
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/dev_notes.md             # svelte-check bug 文档
├── scripts/gen_placeholder_icons.py
├── README.md
├── RELEASE_NOTES.md
└── LICENSE (MIT)
```

## 使用指南

1. 启动后从左上角下拉框选择串口
2. 设置波特率/数据位/停止位/校验/流控
3. 点击"打开",状态指示变绿
4. 中间面板实时显示接收数据,右侧输入并发送
5. 数据会按 `≤16 字节/行` 拆分显示,每行带毫秒时间戳
6. 点击 `💾 导出` 把当前缓冲保存为 txt
7. 点击 `清空` 清空接收面板

### 串口权限

- **Linux**: 用户需要属于 `dialout` 组才能打开 `/dev/ttyUSB0` 等:`sudo usermod -aG dialout $USER`,然后重新登录
- **macOS**: USB-Serial 通常无需额外配置,某些设备需在系统设置授权
- **Windows**: 一般无需额外配置

## 开发路线

- [x] **C1** 项目骨架 (Tauri 2 + Svelte 5 + TS)
- [x] **C2** Rust 串口抽象 (list/open/close/read/write/configure + 单测 + PTY 集成测试)
- [x] **C3** Tauri 命令桥接 (commands + serial:data 事件流 + 错误映射)
- [x] **C4** 前端骨架 (三栏布局 + 端口选择 + 配置面板 + 开关)
- [x] **C5** 实时收发 UI (HEX/ASCII + 时间戳 + 自动滚动 + 暂停 + 清空)
- [x] **C6** 日志与导出 (环形缓冲 5000 行 + save_log + save dialog + txt 导出)
- [x] **C7** 打包完善 (元数据 + README + release notes)
- [ ] **C8** 验证与发布 (三平台实机回环 + 修复 + release)

## 已知问题与限制

- **svelte-check 4.7.4 与 Svelte 5 runes 类型推断不兼容** (工具 bug,源码已确认): `npm run check` 会报错,但实际 Svelte 编译器 (`npm run build`) 正确处理。已在 dev notes 标注。
- **macOS 串口权限**: USB-Serial 设备首次插入需在系统设置授权。
- **图标**: 当前是占位绿色色块,正式发布前需要替换为设计稿。
- **代码签名**: MVP 不签名,Windows SmartScreen / macOS Gatekeeper 会警告,首次需手动允许。

## 贡献

提交 PR 前:
- `cargo test --lib` 通过
- `cargo test -- --ignored` 通过 (需要 PTY 权限)
- `npm test` 通过
- `npm run build` 成功

## License

MIT — 详见 LICENSE 文件